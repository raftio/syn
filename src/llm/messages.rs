use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct MessageRequest {
    pub system: Option<String>,
    pub messages: Vec<Message>,
    pub tools: Vec<Tool>,
    pub model: String,
    pub max_tokens: u32,
}

/// A completed tool call returned after streaming finishes.
#[derive(Debug, Clone)]
pub struct ToolUseResult {
    pub name: String,
    pub input: serde_json::Value,
}

pub fn wiki_edit_tool() -> Tool {
    Tool {
        name: "wiki_edit".to_string(),
        description: "Create, update, append to, or delete a file in the wiki.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "op": {
                    "type": "string",
                    "enum": ["create", "update", "append", "delete"],
                    "description": "File operation"
                },
                "path": {
                    "type": "string",
                    "description": "Relative path from KB root (e.g. wiki/sources/article.md)"
                },
                "content": {
                    "type": "string",
                    "description": "File content (omit for delete)"
                }
            },
            "required": ["op", "path"]
        }),
    }
}
