use anyhow::{anyhow, Context, Result};

use super::{Cli, ConfigAction};
use crate::config::paths::resolve_kb_root;

pub fn run(action: &ConfigAction, cli: &Cli) -> Result<()> {
    let kb_root = resolve_kb_root(&cli.kb_resolve_opts())?;
    let config_path = kb_root.join(".syn").join("config.toml");

    match action {
        ConfigAction::Show => {
            let content = std::fs::read_to_string(&config_path)
                .context("reading config.toml")?;
            print!("{content}");
        }
        ConfigAction::Get { key } => {
            let content = std::fs::read_to_string(&config_path)
                .context("reading config.toml")?;
            let doc: toml::Value = toml::from_str(&content).context("parsing config.toml")?;
            let value = get_dotted(&doc, key)
                .ok_or_else(|| anyhow!("key not found: {key}"))?;
            println!("{}", toml_value_display(value));
        }
        ConfigAction::Set { key, value } => {
            let content = std::fs::read_to_string(&config_path)
                .context("reading config.toml")?;
            let mut doc: toml::Value = toml::from_str(&content).context("parsing config.toml")?;
            set_dotted(&mut doc, key, value)?;
            let new_content = toml::to_string_pretty(&doc).context("serialising config")?;
            std::fs::write(&config_path, new_content)
                .context("writing config.toml")?;
            eprintln!("Set {key} = {value}");

            // Helpful hints when switching providers
            if key == "llm.provider" {
                match value.as_str() {
                    "openai" => eprintln!(
                        "\nHint — also run:\n  syn config set llm.model gpt-4o\n  syn config set llm.api_key_env OPENAI_API_KEY"
                    ),
                    "anthropic" => eprintln!(
                        "\nHint — also run:\n  syn config set llm.model claude-sonnet-4-6\n  syn config set llm.api_key_env ANTHROPIC_API_KEY"
                    ),
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

/// Navigate a dot-path in a `toml::Value` and return the leaf.
fn get_dotted<'a>(doc: &'a toml::Value, path: &str) -> Option<&'a toml::Value> {
    let mut current = doc;
    for part in path.split('.') {
        current = current.get(part)?;
    }
    Some(current)
}

/// Navigate a dot-path and set the leaf value, creating tables as needed.
fn set_dotted(doc: &mut toml::Value, path: &str, value: &str) -> Result<()> {
    let parts: Vec<&str> = path.split('.').collect();
    let (parent_parts, last) = parts.split_at(parts.len() - 1);
    let last = last[0];

    let mut current = doc;
    for part in parent_parts {
        if current.get(part).is_none() {
            if let Some(table) = current.as_table_mut() {
                table.insert(part.to_string(), toml::Value::Table(toml::map::Map::new()));
            }
        }
        current = current.get_mut(part)
            .ok_or_else(|| anyhow!("key not a table: {part}"))?;
    }

    let typed = infer_type(value);
    current
        .as_table_mut()
        .ok_or_else(|| anyhow!("parent path is not a table: {}", &parts[..parts.len()-1].join(".")))?
        .insert(last.to_string(), typed);

    Ok(())
}

/// Infer TOML type from a string value.
fn infer_type(s: &str) -> toml::Value {
    if s == "true" {
        return toml::Value::Boolean(true);
    }
    if s == "false" {
        return toml::Value::Boolean(false);
    }
    if let Ok(n) = s.parse::<i64>() {
        return toml::Value::Integer(n);
    }
    toml::Value::String(s.to_string())
}

fn toml_value_display(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => s.clone(),
        toml::Value::Integer(n) => n.to_string(),
        toml::Value::Boolean(b) => b.to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> toml::Value {
        toml::from_str(r#"
[llm]
provider = "anthropic"
model = "claude-sonnet-4-6"
max_tokens = 8192
"#).unwrap()
    }

    #[test]
    fn get_nested_key() {
        let d = doc();
        assert_eq!(
            get_dotted(&d, "llm.model").and_then(|v| v.as_str()),
            Some("claude-sonnet-4-6")
        );
    }

    #[test]
    fn set_nested_string() {
        let mut d = doc();
        set_dotted(&mut d, "llm.model", "gpt-4o").unwrap();
        assert_eq!(
            get_dotted(&d, "llm.model").and_then(|v| v.as_str()),
            Some("gpt-4o")
        );
    }

    #[test]
    fn set_nested_integer() {
        let mut d = doc();
        set_dotted(&mut d, "llm.max_tokens", "4096").unwrap();
        assert_eq!(
            get_dotted(&d, "llm.max_tokens").and_then(|v| v.as_integer()),
            Some(4096)
        );
    }

    #[test]
    fn get_missing_key_returns_none() {
        let d = doc();
        assert!(get_dotted(&d, "llm.nonexistent").is_none());
    }
}
