use anyhow::Result;

use crate::llm::anthropic::AnthropicClient;
use crate::llm::messages::{MessageRequest, ToolUseResult};
use crate::llm::openai::OpenAiClient;

pub enum LlmProvider {
    Anthropic(AnthropicClient),
    OpenAi(OpenAiClient),
}

impl LlmProvider {
    pub fn from_config(cfg: &crate::config::LlmConfig) -> Result<Self> {
        match cfg.provider.as_str() {
            "openai" => Ok(LlmProvider::OpenAi(OpenAiClient::from_config(cfg)?)),
            _ => Ok(LlmProvider::Anthropic(AnthropicClient::from_config(cfg)?)),
        }
    }

    pub async fn stream_message(
        &self,
        req: &MessageRequest,
        on_text: impl FnMut(&str),
    ) -> Result<Vec<ToolUseResult>> {
        match self {
            LlmProvider::Anthropic(c) => c.stream_message(req, on_text).await,
            LlmProvider::OpenAi(c) => c.stream_message(req, on_text).await,
        }
    }
}
