use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn wai(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("syn").unwrap();
    cmd.current_dir(dir.path());
    cmd
}

async fn init_kb(dir: &TempDir) {
    wai(dir).arg("init").assert().success();
}

fn anthropic_sse_response(text: &str, edits: &[serde_json::Value]) -> String {
    let mut events = String::new();

    // message_start
    events.push_str("event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_01\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-sonnet-4-6\",\"stop_reason\":null,\"usage\":{\"input_tokens\":100,\"output_tokens\":0}}}\n\n");

    // text block
    events.push_str("event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n");
    events.push_str(&format!(
        "event: content_block_delta\ndata: {{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{{\"type\":\"text_delta\",\"text\":{}}}}}\n\n",
        serde_json::to_string(text).unwrap()
    ));
    events.push_str("event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n");

    // tool use blocks
    for (i, edit) in edits.iter().enumerate() {
        let idx = i + 1;
        events.push_str(&format!(
            "event: content_block_start\ndata: {{\"type\":\"content_block_start\",\"index\":{idx},\"content_block\":{{\"type\":\"tool_use\",\"id\":\"tu_{idx}\",\"name\":\"wiki_edit\",\"input\":{{}}}}}}\n\n"
        ));
        let json_str = serde_json::to_string(edit).unwrap();
        events.push_str(&format!(
            "event: content_block_delta\ndata: {{\"type\":\"content_block_delta\",\"index\":{idx},\"delta\":{{\"type\":\"input_json_delta\",\"partial_json\":{}}}}}\n\n",
            serde_json::to_string(&json_str).unwrap()
        ));
        events.push_str(&format!(
            "event: content_block_stop\ndata: {{\"type\":\"content_block_stop\",\"index\":{idx}}}\n\n"
        ));
    }

    // message_delta + stop
    events.push_str("event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":50}}\n\n");
    events.push_str("event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n");

    events
}

#[tokio::test]
async fn ingest_dry_run_shows_edits_without_writing() {
    let server = MockServer::start().await;
    let dir = TempDir::new().unwrap();
    init_kb(&dir).await;

    let edits = vec![serde_json::json!({
        "op": "create",
        "path": "wiki/sources/test-article.md",
        "content": "# Test Article\n\nSummary."
    })];

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "test-key"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(anthropic_sse_response("Ingesting the article.", &edits))
                .append_header("content-type", "text/event-stream"),
        )
        .mount(&server)
        .await;

    // Write a test source
    let source_path = dir.path().join("my-article.md");
    std::fs::write(&source_path, "# Test Article\n\nSome content.").unwrap();

    // Patch the API URL via env — we override by testing the ingest logic directly
    // For integration test, use --dry-run to avoid needing real API
    wai(&dir)
        .arg("ingest")
        .arg(source_path.to_str().unwrap())
        .arg("--dry-run")
        .env("ANTHROPIC_API_KEY", "test-key")
        .env("ANTHROPIC_BASE_URL", server.uri()) // future hook
        .assert()
        // dry-run exits 0 and prints a "no changes" message or proceeds to stderr
        .success();

    // Wiki file should NOT be created in dry-run
    assert!(!dir.path().join("wiki/sources/test-article.md").exists());
}

#[tokio::test]
async fn ingest_fails_without_api_key() {
    let dir = TempDir::new().unwrap();
    init_kb(&dir).await;

    let source_path = dir.path().join("article.md");
    std::fs::write(&source_path, "# Article\n\nContent.").unwrap();

    wai(&dir)
        .arg("ingest")
        .arg(source_path.to_str().unwrap())
        .env_remove("ANTHROPIC_API_KEY")
        .assert()
        .failure()
        .stderr(predicate::str::contains("ANTHROPIC_API_KEY"));
}

#[tokio::test]
async fn ingest_fails_with_missing_source_file() {
    let dir = TempDir::new().unwrap();
    init_kb(&dir).await;

    wai(&dir)
        .arg("ingest")
        .arg("nonexistent-file.md")
        .env("ANTHROPIC_API_KEY", "test-key")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[tokio::test]
async fn ingest_fails_outside_kb() {
    let dir = TempDir::new().unwrap();
    // No wai init — no .wai/config.toml

    let source_path = dir.path().join("article.md");
    std::fs::write(&source_path, "# Article\n").unwrap();

    wai(&dir)
        .arg("ingest")
        .arg(source_path.to_str().unwrap())
        .env("ANTHROPIC_API_KEY", "test-key")
        .assert()
        .failure()
        .stderr(predicate::str::contains("knowledge base"));
}

#[tokio::test]
async fn ingest_multiple_files_dry_run() {
    let server = MockServer::start().await;
    let dir = TempDir::new().unwrap();
    init_kb(&dir).await;

    let edits_a = vec![serde_json::json!({
        "op": "create",
        "path": "wiki/sources/article-a.md",
        "content": "# A\n\nSummary."
    })];
    let edits_b = vec![serde_json::json!({
        "op": "create",
        "path": "wiki/sources/article-b.md",
        "content": "# B\n\nSummary."
    })];

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "test-key"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(anthropic_sse_response("Ingesting.", &edits_a))
                .append_header("content-type", "text/event-stream"),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "test-key"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(anthropic_sse_response("Ingesting.", &edits_b))
                .append_header("content-type", "text/event-stream"),
        )
        .mount(&server)
        .await;

    let a = dir.path().join("article-a.md");
    let b = dir.path().join("article-b.md");
    std::fs::write(&a, "# Article A\n\nContent.").unwrap();
    std::fs::write(&b, "# Article B\n\nContent.").unwrap();

    wai(&dir)
        .arg("ingest")
        .arg(a.to_str().unwrap())
        .arg(b.to_str().unwrap())
        .arg("--dry-run")
        .env("ANTHROPIC_API_KEY", "test-key")
        .env("ANTHROPIC_BASE_URL", server.uri())
        .assert()
        .success()
        .stderr(predicate::str::contains("dry-run"));

    // Nothing written in dry-run
    assert!(!dir.path().join("wiki/sources/article-a.md").exists());
    assert!(!dir.path().join("wiki/sources/article-b.md").exists());
}

#[tokio::test]
async fn ingest_skip_existing_skips_already_ingested() {
    let dir = TempDir::new().unwrap();
    init_kb(&dir).await;

    // Pre-create the wiki/sources page to simulate already-ingested
    let sources_dir = dir.path().join("wiki").join("sources");
    std::fs::create_dir_all(&sources_dir).unwrap();
    std::fs::write(sources_dir.join("my-note.md"), "# My Note\n\nAlready here.").unwrap();

    let source_path = dir.path().join("my-note.md");
    std::fs::write(&source_path, "# My Note\n\nContent.").unwrap();

    // No API mock — should not reach LLM at all
    wai(&dir)
        .arg("ingest")
        .arg(source_path.to_str().unwrap())
        .arg("--skip-existing")
        .env("ANTHROPIC_API_KEY", "test-key")
        .assert()
        .success()
        .stderr(predicate::str::contains("already ingested").or(predicate::str::contains("All sources")));
}

#[tokio::test]
async fn ingest_ext_filter_excludes_non_matching() {
    let dir = TempDir::new().unwrap();
    init_kb(&dir).await;

    // A .txt file with the default --ext md filter → no files pass → "No matching files found"
    let txt_path = dir.path().join("notes.txt");
    std::fs::write(&txt_path, "Some notes.").unwrap();

    wai(&dir)
        .arg("ingest")
        .arg(txt_path.to_str().unwrap())
        .env("ANTHROPIC_API_KEY", "test-key")
        .assert()
        .success()
        .stderr(predicate::str::contains("No matching files"));
}

#[tokio::test]
async fn ingest_directory_walk_dry_run() {
    let server = MockServer::start().await;
    let dir = TempDir::new().unwrap();
    init_kb(&dir).await;

    // Create a subdirectory with two markdown files
    let notes_dir = dir.path().join("notes");
    std::fs::create_dir_all(&notes_dir).unwrap();
    std::fs::write(notes_dir.join("alpha.md"), "# Alpha\n\nContent.").unwrap();
    std::fs::write(notes_dir.join("beta.md"), "# Beta\n\nContent.").unwrap();
    std::fs::write(notes_dir.join("ignore.txt"), "Not markdown.").unwrap();

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "test-key"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(anthropic_sse_response("OK", &[]))
                .append_header("content-type", "text/event-stream"),
        )
        .mount(&server)
        .await;

    wai(&dir)
        .arg("ingest")
        .arg(notes_dir.to_str().unwrap())
        .arg("--dry-run")
        .env("ANTHROPIC_API_KEY", "test-key")
        .env("ANTHROPIC_BASE_URL", server.uri())
        .assert()
        .success();
}
