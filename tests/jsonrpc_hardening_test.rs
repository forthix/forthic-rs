//! JSON-RPC server hardening tests
//!
//! Port of forthic-ts `tests/unit/jsonrpc/server_hardening.test.ts`:
//! loopback default, bearer auth (including its ordering relative to body
//! reads), the body cap via both Content-Length and streaming/chunked
//! enforcement, and error-detail sanitization.

mod common;

use common::{any_valid_envelope, TestServer};
use forthic::jsonrpc::ServeOptions;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

fn with_token(token: &str) -> ServeOptions {
    ServeOptions {
        token: Some(token.to_string()),
        ..ServeOptions::default()
    }
}

// ===== Bind defaults =====

#[tokio::test]
async fn test_binds_loopback_by_default() {
    let server = TestServer::start(ServeOptions::default()).await;
    assert!(server.addr().ip().is_loopback(), "bound {}", server.addr());
    server.stop().await;
}

// ===== Bearer auth =====

#[tokio::test]
async fn test_auth_required_when_token_set() {
    let server = TestServer::start(with_token("sekret")).await;

    // No Authorization header
    let response = server.request().body(any_valid_envelope()).send().await.unwrap();
    assert_eq!(response.status(), 401);
    assert_eq!(
        response.headers().get("www-authenticate").and_then(|v| v.to_str().ok()),
        Some("Bearer")
    );
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], -32600);
    assert_eq!(body["error"]["message"], "Unauthorized");

    // Wrong token
    let response = server
        .request()
        .header("Authorization", "Bearer wrong")
        .body(any_valid_envelope())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);

    // Same secret but not a Bearer scheme
    let response = server
        .request()
        .header("Authorization", "Basic sekret")
        .body(any_valid_envelope())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);

    // Correct token
    let response = server
        .request()
        .header("Authorization", "Bearer sekret")
        .body(any_valid_envelope())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert!(body.get("result").is_some());

    server.stop().await;
}

#[tokio::test]
async fn test_no_token_means_open_on_loopback() {
    let server = TestServer::start(ServeOptions::default()).await;
    let response = server.request().body(any_valid_envelope()).send().await.unwrap();
    assert_eq!(response.status(), 200);
    server.stop().await;
}

#[tokio::test]
async fn test_auth_checked_before_body_is_read() {
    // An unauthorized request with a huge declared body must get 401, not
    // 413 — proving the auth check precedes any body handling
    let server = TestServer::start(with_token("sekret")).await;
    let mut stream = TcpStream::connect(server.addr()).await.unwrap();
    let request = "POST /rpc HTTP/1.1\r\n\
                   Host: test\r\n\
                   Content-Type: application/json\r\n\
                   Content-Length: 999999999\r\n\
                   Connection: close\r\n\
                   \r\n";
    stream.write_all(request.as_bytes()).await.unwrap();
    // Don't send the body; the server should answer from headers alone
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    assert!(
        response.starts_with("HTTP/1.1 401"),
        "expected 401 before body read, got: {}",
        response.lines().next().unwrap_or("")
    );
    server.stop().await;
}

// ===== Body cap =====

fn small_cap() -> ServeOptions {
    ServeOptions {
        max_body_bytes: Some(256),
        ..ServeOptions::default()
    }
}

#[tokio::test]
async fn test_content_length_precheck_413() {
    let server = TestServer::start(small_cap()).await;
    let big = "x".repeat(1000);
    let (status, text) = server.post_raw(big, "application/json").await;
    assert_eq!(status, 413);
    let body: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(body["error"]["code"], -32600);
    assert_eq!(body["error"]["message"], "Payload too large");
    server.stop().await;
}

#[tokio::test]
async fn test_chunked_body_enforced_while_streaming() {
    // Chunked transfer has no Content-Length, so only the streaming cap
    // can catch it
    let server = TestServer::start(small_cap()).await;
    let mut stream = TcpStream::connect(server.addr()).await.unwrap();
    let head = "POST /rpc HTTP/1.1\r\n\
                Host: test\r\n\
                Content-Type: application/json\r\n\
                Transfer-Encoding: chunked\r\n\
                Connection: close\r\n\
                \r\n";
    stream.write_all(head.as_bytes()).await.unwrap();
    // Send 4 chunks of 100 bytes (400 > 256 cap), then terminate
    let chunk_data = "y".repeat(100);
    for _ in 0..4 {
        let chunk = format!("64\r\n{chunk_data}\r\n");
        if stream.write_all(chunk.as_bytes()).await.is_err() {
            break; // server may have already rejected and closed
        }
    }
    let _ = stream.write_all(b"0\r\n\r\n").await;
    let mut response = String::new();
    let _ = stream.read_to_string(&mut response).await;
    assert!(
        response.starts_with("HTTP/1.1 413"),
        "expected 413 from streaming cap, got: {}",
        response.lines().next().unwrap_or("<no response>")
    );
    server.stop().await;
}

#[tokio::test]
async fn test_body_at_cap_still_works() {
    // The cap is a limit, not a target — a request exactly at it succeeds
    let envelope = any_valid_envelope();
    let server = TestServer::start(ServeOptions {
        max_body_bytes: Some(envelope.len()),
        ..ServeOptions::default()
    })
    .await;
    let (status, body) = server.post_json(envelope, &server.endpoint.clone()).await;
    assert_eq!(status, 200);
    assert!(body.get("result").is_some());
    server.stop().await;
}

// ===== Error-detail sanitization =====

#[tokio::test]
async fn test_error_details_stripped_by_default() {
    let server = TestServer::start(ServeOptions::default()).await;
    // Unterminated string: a tokenizer error, which carries a location
    let (_, body) = server
        .rpc("executeWord", json!({ "word_name": "'unterminated", "stack": [] }))
        .await;
    let data = &body["error"]["data"];
    assert_eq!(data["error_type"], "UnterminatedString");
    assert!(data.get("word_location").is_none(), "got: {data}");
    assert!(data.get("stack_trace").is_none());
    server.stop().await;
}

#[tokio::test]
async fn test_error_details_exposed_when_enabled() {
    let server = TestServer::start(ServeOptions {
        expose_error_details: true,
        ..ServeOptions::default()
    })
    .await;
    let (_, body) = server
        .rpc("executeWord", json!({ "word_name": "'unterminated", "stack": [] }))
        .await;
    let data = &body["error"]["data"];
    assert!(
        data.get("word_location").and_then(Value::as_str).is_some(),
        "expected word_location in {data}"
    );
    server.stop().await;
}
