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

fn init_with_pages(dir: &TempDir) {
    wai(dir).arg("init").assert().success();
    std::fs::create_dir_all(dir.path().join("wiki/concepts")).unwrap();
    std::fs::write(
        dir.path().join("wiki/concepts/rust.md"),
        "---\ntitle: \"Rust Programming\"\n---\n\nRust is a systems language focused on memory safety.",
    ).unwrap();
}

fn anthropic_text_response(text: &str) -> String {
    format!(
        "event: message_start\ndata: {{\"type\":\"message_start\",\"message\":{{\"id\":\"msg_01\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-sonnet-4-6\",\"stop_reason\":null,\"usage\":{{\"input_tokens\":10,\"output_tokens\":0}}}}}}\n\n\
         event: content_block_start\ndata: {{\"type\":\"content_block_start\",\"index\":0,\"content_block\":{{\"type\":\"text\",\"text\":\"\"}}}}\n\n\
         event: content_block_delta\ndata: {{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{{\"type\":\"text_delta\",\"text\":{}}}}}\n\n\
         event: content_block_stop\ndata: {{\"type\":\"content_block_stop\",\"index\":0}}\n\n\
         event: message_delta\ndata: {{\"type\":\"message_delta\",\"delta\":{{\"stop_reason\":\"end_turn\"}},\"usage\":{{\"output_tokens\":20}}}}\n\n\
         event: message_stop\ndata: {{\"type\":\"message_stop\"}}\n\n",
        serde_json::to_string(text).unwrap()
    )
}

#[tokio::test]
async fn query_streams_answer() {
    let server = MockServer::start().await;
    let dir = TempDir::new().unwrap();
    init_with_pages(&dir);

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "test-key"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(anthropic_text_response("Rust is great for systems programming."))
                .append_header("content-type", "text/event-stream"),
        )
        .mount(&server)
        .await;

    wai(&dir)
        .args(["query", "What is Rust?"])
        .env("ANTHROPIC_API_KEY", "test-key")
        .env("ANTHROPIC_BASE_URL", server.uri())
        .assert()
        .success()
        .stdout(predicate::str::contains("Rust"));
}

#[tokio::test]
async fn query_save_creates_synthesis_page() {
    let server = MockServer::start().await;
    let dir = TempDir::new().unwrap();
    init_with_pages(&dir);

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(anthropic_text_response("Rust provides memory safety."))
                .append_header("content-type", "text/event-stream"),
        )
        .mount(&server)
        .await;

    wai(&dir)
        .args(["query", "What is Rust?", "--save", "rust-overview"])
        .env("ANTHROPIC_API_KEY", "test-key")
        .env("ANTHROPIC_BASE_URL", server.uri())
        .assert()
        .success();

    assert!(dir.path().join("wiki/synthesis/rust-overview.md").exists());

    let content = std::fs::read_to_string(
        dir.path().join("wiki/synthesis/rust-overview.md")
    ).unwrap();
    assert!(content.contains("Rust provides memory safety."));
    assert!(content.contains("What is Rust?"));
}

#[tokio::test]
async fn query_fails_outside_kb() {
    let dir = TempDir::new().unwrap();
    wai(&dir)
        .args(["query", "anything"])
        .env("ANTHROPIC_API_KEY", "test-key")
        .assert()
        .failure()
        .stderr(predicate::str::contains("knowledge base"));
}
