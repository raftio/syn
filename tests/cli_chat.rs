mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tempfile::TempDir;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn syn_cmd(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("syn").unwrap();
    cmd.current_dir(dir.path());
    cmd
}

fn init_with_pages(dir: &TempDir) {
    syn_cmd(dir).arg("init").assert().success();
    std::fs::create_dir_all(dir.path().join("wiki/concepts")).unwrap();
    std::fs::write(
        dir.path().join("wiki/concepts/rust.md"),
        "---\ntitle: \"Rust Programming\"\n---\n\nRust is a systems language focused on memory safety.",
    )
    .unwrap();
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

/// stdin must end with `exit` or `quit` so the REPL exits; use `write_stdin` so the child
/// stdin closes only after the bytes are written (avoids hanging on read_line).
#[tokio::test]
async fn chat_two_turns_streams_both_answers() {
    let server = MockServer::start().await;
    let dir = TempDir::new().unwrap();
    init_with_pages(&dir);

    let call = Arc::new(AtomicUsize::new(0));
    let call_m = call.clone();
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "test-key"))
        .respond_with(move |_req: &wiremock::Request| {
            let n = call_m.fetch_add(1, Ordering::SeqCst);
            let text = if n == 0 {
                "First turn about Rust."
            } else {
                "Second turn about memory safety."
            };
            ResponseTemplate::new(200)
                .set_body_string(anthropic_text_response(text))
                .append_header("content-type", "text/event-stream")
        })
        .expect(2)
        .mount(&server)
        .await;

    syn_cmd(&dir)
        .arg("chat")
        .write_stdin(
            "What is Rust?\n\
             Tell me about safety.\n\
             exit\n",
        )
        .env("ANTHROPIC_API_KEY", "test-key")
        .env("ANTHROPIC_BASE_URL", server.uri())
        .assert()
        .success()
        .stdout(predicate::str::contains("First turn about Rust."))
        .stdout(predicate::str::contains("Second turn about memory safety."));

    assert_eq!(call.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn chat_fails_outside_kb() {
    let dir = TempDir::new().unwrap();
    syn_cmd(&dir)
        .env("SYN_GLOBAL_CONFIG", common::empty_global_config_path(&dir))
        .arg("chat")
        .env("ANTHROPIC_API_KEY", "test-key")
        .assert()
        .failure()
        .stderr(predicate::str::contains("knowledge base"));
}
