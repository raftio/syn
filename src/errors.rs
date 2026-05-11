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

    #[error(
        "not inside a syn knowledge base — run from the vault tree, or set SYN_KB / SYN_VAULT, use --kb-root / -w NAME, `syn vault default`, or register exactly one vault"
    )]
    NotAKnowledgeBase,
}
