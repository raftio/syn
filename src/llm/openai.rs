use std::collections::HashMap;

use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use serde::Deserialize;
use tracing::debug;

use crate::llm::messages::{MessageRequest, ToolUseResult};
use crate::llm::stream::SseParser;

const DEFAULT_API_BASE: &str = "https://api.openai.com";

pub struct OpenAiClient {
    http: reqwest::Client,
    api_key: String,
    model: String,
    max_tokens: u32,
    base_url: String,
}

impl OpenAiClient {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>, max_tokens: u32) -> Self {
        let base_url = std::env::var("OPENAI_BASE_URL")
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
        format!("{}/v1/chat/completions", self.base_url.trim_end_matches('/'))
    }

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
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await
                .context("sending request to OpenAI")?;

            let status = resp.status();

            if status.as_u16() == 429 {
                // Extract suggested delay from Retry-After header, then from body.
                let retry_after = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<f64>().ok());
                let err_body = resp.text().await.unwrap_or_default();

                if attempt >= MAX_RETRIES {
                    bail!("OpenAI API error {status}: {err_body}");
                }

                let delay = retry_after
                    .or_else(|| parse_retry_secs(&err_body))
                    .unwrap_or_else(|| 2_f64.powi(attempt as i32 + 1))
                    .max(1.0);

                let source = if retry_after.is_some() {
                    "Retry-After header"
                } else if parse_retry_secs(&err_body).is_some() {
                    "response body"
                } else {
                    "exponential backoff"
                };
                tracing::warn!(
                    attempt = attempt + 1,
                    max = MAX_RETRIES,
                    delay_secs = delay,
                    delay_source = source,
                    body = %err_body,
                    "OpenAI rate limit (429) — retrying"
                );
                eprint!(
                    "\r\x1b[2K\x1b[90m[rate limit] waiting {delay:.1}s then retrying ({}/{MAX_RETRIES})…\x1b[0m",
                    attempt + 1
                );
                tokio::time::sleep(std::time::Duration::from_secs_f64(delay)).await;
                continue;
            }

            if !status.is_success() {
                let err_body = resp.text().await.unwrap_or_default();
                bail!("OpenAI API error {status}: {err_body}");
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
                        let chunk: OpenAiChunk = match serde_json::from_str(&data) {
                            Ok(c) => c,
                            Err(e) => {
                                debug!("skipping unparseable SSE: {e} — {data}");
                                continue;
                            }
                        };
                        process_chunk(chunk, &mut state, &mut on_text);
                    }
                }
            }

            return Ok(state.finalize());
        }

        bail!("OpenAI API rate limit exceeded after {MAX_RETRIES} retries")
    }
}

/// Parse "Please try again in 1.834s." from an OpenAI 429 body.
fn parse_retry_secs(body: &str) -> Option<f64> {
    let start = body.find("Please try again in ")? + "Please try again in ".len();
    let rest = &body[start..];
    let end = rest.find('s')?;
    rest[..end].trim().parse::<f64>().ok()
}

fn build_request_body(req: &MessageRequest, model: &str, max_tokens: u32) -> serde_json::Value {
    // OpenAI: system prompt is a message with role "system"
    let mut messages: Vec<serde_json::Value> = Vec::new();
    if let Some(system) = &req.system {
        messages.push(serde_json::json!({"role": "system", "content": system}));
    }
    for m in &req.messages {
        messages.push(serde_json::json!({
            "role": format!("{:?}", m.role).to_lowercase(),
            "content": m.content,
        }));
    }

    // OpenAI uses "parameters" not "input_schema"
    let tools: Vec<serde_json::Value> = req
        .tools
        .iter()
        .map(|t| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.input_schema,
                }
            })
        })
        .collect();

    let mut body = serde_json::json!({
        "model": model,
        "max_completion_tokens": max_tokens,
        "messages": messages,
        "stream": true,
    });

    if !tools.is_empty() {
        body["tools"] = serde_json::Value::Array(tools);
    }

    body
}

// ── SSE types ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct OpenAiChunk {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    delta: Delta,
}

#[derive(Debug, Deserialize, Default)]
struct Delta {
    content: Option<String>,
    tool_calls: Option<Vec<ToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
struct ToolCallDelta {
    index: usize,
    id: Option<String>,
    function: Option<FunctionDelta>,
}

#[derive(Debug, Deserialize)]
struct FunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}

// ── Streaming state ───────────────────────────────────────────────────────────

#[derive(Default)]
struct StreamState {
    tool_blocks: HashMap<usize, ToolBlockAccum>,
}

struct ToolBlockAccum {
    id: String,
    name: String,
    args_buf: String,
}

impl StreamState {
    fn finalize(self) -> Vec<ToolUseResult> {
        let mut indices: Vec<usize> = self.tool_blocks.keys().copied().collect();
        indices.sort();
        indices
            .into_iter()
            .map(|i| {
                let b = &self.tool_blocks[&i];
                let input =
                    serde_json::from_str(&b.args_buf).unwrap_or(serde_json::Value::Null);
                ToolUseResult { name: b.name.clone(), input }
            })
            .collect()
    }
}

fn process_chunk(chunk: OpenAiChunk, state: &mut StreamState, on_text: &mut impl FnMut(&str)) {
    for choice in chunk.choices {
        let delta = choice.delta;

        if let Some(text) = delta.content {
            if !text.is_empty() {
                on_text(&text);
            }
        }

        if let Some(tool_calls) = delta.tool_calls {
            for tc in tool_calls {
                let entry = state.tool_blocks.entry(tc.index).or_insert_with(|| ToolBlockAccum {
                    id: String::new(),
                    name: String::new(),
                    args_buf: String::new(),
                });
                if let Some(id) = tc.id {
                    entry.id = id;
                }
                if let Some(func) = tc.function {
                    if let Some(name) = func.name {
                        entry.name = name;
                    }
                    if let Some(args) = func.arguments {
                        entry.args_buf.push_str(&args);
                    }
                }
            }
        }
    }
}
