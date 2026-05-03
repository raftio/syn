use std::collections::HashMap;

use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use serde::Deserialize;
use tracing::debug;

use crate::llm::messages::{MessageRequest, ToolUseResult};
use crate::llm::stream::SseParser;

const DEFAULT_API_BASE: &str = "https://api.anthropic.com";
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AnthropicClient {
    http: reqwest::Client,
    api_key: String,
    model: String,
    max_tokens: u32,
    /// Base URL — overridable via `ANTHROPIC_BASE_URL` for testing.
    base_url: String,
}

impl AnthropicClient {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>, max_tokens: u32) -> Self {
        let base_url = std::env::var("ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| DEFAULT_API_BASE.to_string());
        Self {
            http: reqwest::Client::new(),
            api_key: api_key.into(),
            model: model.into(),
            max_tokens,
            base_url,
        }
    }

    pub fn from_config(cfg: &crate::config::LlmConfig) -> Result<Self> {
        let api_key = std::env::var(&cfg.api_key_env)
            .with_context(|| format!("environment variable {} not set", cfg.api_key_env))?;
        Ok(Self::new(&api_key, &cfg.model, cfg.max_tokens))
    }

    fn api_url(&self) -> String {
        format!("{}/v1/messages", self.base_url.trim_end_matches('/'))
    }

    /// Stream a message from Claude. Calls `on_text` for each text delta (printed live).
    /// Returns all tool-use results after the stream ends.
    pub async fn stream_message(
        &self,
        req: &MessageRequest,
        mut on_text: impl FnMut(&str),
    ) -> Result<Vec<ToolUseResult>> {
        let body = build_request_body(req, &self.model, self.max_tokens);
        let url = self.api_url();

        const MAX_RETRIES: u32 = 8;

        for attempt in 0..=MAX_RETRIES {
            debug!("POST {url}");

            let resp = self
                .http
                .post(&url)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await
                .context("sending request to Anthropic")?;

            let status = resp.status();

            if status.as_u16() == 429 {
                let retry_after = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<f64>().ok());
                let err_body = resp.text().await.unwrap_or_default();

                if attempt >= MAX_RETRIES {
                    bail!("Anthropic API error {status}: {err_body}");
                }

                let delay = retry_after
                    .unwrap_or_else(|| 2_f64.powi(attempt as i32 + 1))
                    .max(1.0);

                eprint!(
                    "\r\x1b[2K\x1b[90m[rate limit] waiting {delay:.1}s then retrying ({}/{MAX_RETRIES})…\x1b[0m",
                    attempt + 1
                );
                tokio::time::sleep(std::time::Duration::from_secs_f64(delay)).await;
                continue;
            }

            if !status.is_success() {
                let err_body = resp.text().await.unwrap_or_default();
                bail!("Anthropic API error {status}: {err_body}");
            }

            let mut parser = SseParser::new();
            let mut state = StreamState::default();
            let mut bytes_stream = resp.bytes_stream();

            while let Some(chunk) = bytes_stream.next().await {
                let bytes = chunk.context("reading response stream")?;
                for sse in parser.feed(&bytes) {
                    if let Some(data) = sse.data {
                        if data == "[DONE]" {
                            break;
                        }
                        let event: AnthropicEvent = match serde_json::from_str(&data) {
                            Ok(e) => e,
                            Err(e) => {
                                debug!("skipping unparseable SSE data: {e} — {data}");
                                continue;
                            }
                        };
                        process_event(event, &mut state, &mut on_text)?;
                    }
                }
            }

            return Ok(state.finalize());
        }

        bail!("Anthropic API rate limit exceeded after {MAX_RETRIES} retries")
    }
}

fn build_request_body(
    req: &MessageRequest,
    model: &str,
    max_tokens: u32,
) -> serde_json::Value {
    let messages: Vec<serde_json::Value> = req
        .messages
        .iter()
        .map(|m| {
            serde_json::json!({
                "role": format!("{:?}", m.role).to_lowercase(),
                "content": m.content,
            })
        })
        .collect();

    let tools: Vec<serde_json::Value> = req
        .tools
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.input_schema,
            })
        })
        .collect();

    let mut body = serde_json::json!({
        "model": model,
        "max_tokens": max_tokens,
        "messages": messages,
        "stream": true,
    });

    if let Some(system) = &req.system {
        body["system"] = serde_json::Value::String(system.clone());
    }

    if !tools.is_empty() {
        body["tools"] = serde_json::Value::Array(tools);
    }

    body
}

// ── SSE event types ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicEvent {
    MessageStart {
        #[allow(dead_code)]
        message: serde_json::Value,
    },
    ContentBlockStart {
        index: usize,
        content_block: ContentBlock,
    },
    ContentBlockDelta {
        index: usize,
        delta: Delta,
    },
    ContentBlockStop {
        #[allow(dead_code)]
        index: usize,
    },
    MessageDelta {
        #[allow(dead_code)]
        delta: serde_json::Value,
    },
    MessageStop,
    Ping,
    Error {
        error: ApiError,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentBlock {
    Text {
        #[allow(dead_code)]
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Delta {
    TextDelta { text: String },
    InputJsonDelta { partial_json: String },
}

#[derive(Debug, Deserialize)]
struct ApiError {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}

// ── Streaming state ───────────────────────────────────────────────────────────

#[derive(Default)]
struct StreamState {
    tool_blocks: HashMap<usize, ToolBlockAccum>,
}

struct ToolBlockAccum {
    name: String,
    #[allow(dead_code)]
    id: String,
    json_buf: String,
}

impl StreamState {
    fn finalize(self) -> Vec<ToolUseResult> {
        let mut results = Vec::new();
        let mut indices: Vec<usize> = self.tool_blocks.keys().copied().collect();
        indices.sort();
        for idx in indices {
            let block = self.tool_blocks[&idx].to_owned_values();
            results.push(block);
        }
        results
    }
}

impl ToolBlockAccum {
    fn to_owned_values(&self) -> ToolUseResult {
        let input = serde_json::from_str(&self.json_buf).unwrap_or(serde_json::Value::Null);
        ToolUseResult {
            name: self.name.clone(),
            input,
        }
    }
}

fn process_event(
    event: AnthropicEvent,
    state: &mut StreamState,
    on_text: &mut impl FnMut(&str),
) -> Result<()> {
    match event {
        AnthropicEvent::ContentBlockStart {
            index,
            content_block: ContentBlock::ToolUse { id, name },
        } => {
            state.tool_blocks.insert(
                index,
                ToolBlockAccum {
                    name,
                    id,
                    json_buf: String::new(),
                },
            );
        }
        AnthropicEvent::ContentBlockDelta {
            delta: Delta::TextDelta { text },
            ..
        } => {
            on_text(&text);
        }
        AnthropicEvent::ContentBlockDelta {
            index,
            delta: Delta::InputJsonDelta { partial_json },
        } => {
            if let Some(block) = state.tool_blocks.get_mut(&index) {
                block.json_buf.push_str(&partial_json);
            }
        }
        AnthropicEvent::Error { error } => {
            bail!("Anthropic stream error [{}]: {}", error.error_type, error.message);
        }
        _ => {}
    }
    Ok(())
}

