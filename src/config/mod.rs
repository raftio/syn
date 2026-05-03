pub mod paths;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum WikilinkStyle {
    #[default]
    Syn,
    Obsidian,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfig {
    pub obsidian: bool,
    pub wiki_dir: String,
    pub raw_dir: String,
    pub wikilink_style: WikilinkStyle,
}

impl Default for VaultConfig {
    fn default() -> Self {
        VaultConfig {
            obsidian: true,
            wiki_dir: "syn".to_string(),
            raw_dir: "syn-sources".to_string(),
            wikilink_style: WikilinkStyle::Obsidian,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub llm: LlmConfig,
    pub paths: PathsConfig,
    pub search: SearchConfig,
    pub ingest: IngestConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vault: Option<VaultConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: String,
    pub model: String,
    pub max_tokens: u32,
    pub api_key_env: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathsConfig {
    pub raw: String,
    pub wiki: String,
    pub schema: String,
    pub index: String,
    pub log: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    pub backend: String,
    pub top_k: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestConfig {
    pub auto_commit: bool,
    pub include_schema_in_prompt: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            llm: LlmConfig {
                provider: "anthropic".to_string(),
                model: "claude-sonnet-4-6".to_string(),
                max_tokens: 8192,
                api_key_env: "ANTHROPIC_API_KEY".to_string(),
            },
            paths: PathsConfig {
                raw: "raw".to_string(),
                wiki: "wiki".to_string(),
                schema: "CLAUDE.md".to_string(),
                index: "index.md".to_string(),
                log: "log.md".to_string(),
            },
            search: SearchConfig {
                backend: "bm25".to_string(),
                top_k: 8,
            },
            ingest: IngestConfig {
                auto_commit: false,
                include_schema_in_prompt: true,
            },
            vault: None,
        }
    }
}

impl Config {
    pub fn load(kb_root: &Path) -> Result<Self> {
        let config_path = kb_root.join(".syn").join("config.toml");
        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("reading {}", config_path.display()))?;
        toml::from_str(&content).with_context(|| "parsing .syn/config.toml")
    }

    pub fn save(&self, kb_root: &Path) -> Result<()> {
        let config_path = kb_root.join(".syn").join("config.toml");
        let content = toml::to_string_pretty(self).context("serializing config")?;
        std::fs::write(&config_path, content)
            .with_context(|| format!("writing {}", config_path.display()))
    }

    pub fn wiki_dir_name(&self) -> &str {
        self.vault.as_ref()
            .map(|v| v.wiki_dir.as_str())
            .unwrap_or(&self.paths.wiki)
    }

    #[allow(dead_code)]
    pub fn raw_dir(&self, kb_root: &Path) -> PathBuf {
        let name = self.vault.as_ref()
            .map(|v| v.raw_dir.as_str())
            .unwrap_or(&self.paths.raw);
        kb_root.join(name)
    }

    pub fn wiki_dir(&self, kb_root: &Path) -> PathBuf {
        kb_root.join(self.wiki_dir_name())
    }

    pub fn schema_path(&self, kb_root: &Path) -> PathBuf {
        kb_root.join(&self.paths.schema)
    }

    pub fn index_path(&self, kb_root: &Path) -> PathBuf {
        kb_root.join(&self.paths.index)
    }

    pub fn log_path(&self, kb_root: &Path) -> PathBuf {
        kb_root.join(&self.paths.log)
    }

    pub fn wikilink_style(&self) -> WikilinkStyle {
        self.vault.as_ref()
            .map(|v| v.wikilink_style.clone())
            .unwrap_or_default()
    }
}
