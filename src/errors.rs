use thiserror::Error;

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum WaiError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("LLM error: {0}")]
    Llm(String),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("invalid edit: {0}")]
    InvalidEdit(String),

    #[error("source load error: {0}")]
    SourceLoad(String),

    #[error("not inside a syn knowledge base (no .syn/config.toml found)")]
    NotAKnowledgeBase,
}
