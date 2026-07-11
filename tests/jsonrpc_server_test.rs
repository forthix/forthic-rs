//! JSON-RPC server tests over real HTTP
//!
//! Port of forthic-ts `tests/unit/jsonrpc/server.test.ts`: exercises the
//! wire format end-to-end — envelopes, the four methods, protocol errors,
//! and transport-level rejections. Dispatch-level behavior (validation
//! messages, ErrorInfo shape) is covered in jsonrpc_dispatch_test.rs; here
//! the focus is that HTTP delivers the same contract.

mod common;

use common::{any_valid_envelope, TestServer};
use forthic::jsonrpc::ServeOptions;
use serde_json::json;

#[tokio::test]
async fn test_list_modules_over_http() {
    let server = TestServer::start(ServeOptions::default()).await;
    let (status, body) = server.rpc("listModules", json!({})).await;
    assert_eq!(status, 200);
    assert_eq!(body["jsonrpc"], "2.0");
    assert_eq!(body["result"]["modules"], json!([]));
    server.stop().await;
}

#[tokio::test]
async fn test_execute_word_round_trips_stack() {
    let server = TestServer::start(ServeOptions::default()).await;
    let (status, body) = server
        .rpc(
            "executeWord",
            json!({
                "word_name": "SWAP",
                "stack": [ { "int_value": 1 }, { "int_value": 2 } ]
            }),
        )
        .await;
    assert_eq!(status, 200);
    assert_eq!(
        body["result"]["result_stack"],
        json!([ { "int_value": 2 }, { "int_value": 1 } ])
    );
    server.stop().await;
}

#[tokio::test]
async fn test_execute_sequence_over_http() {
    let server = TestServer::start(ServeOptions::default()).await;
    let (status, body) = server
        .rpc(
            "executeSequence",
            json!({ "word_names": ["DUP", "+"], "stack": [ { "int_value": 21 } ] }),
        )
        .await;
    assert_eq!(status, 200);
    assert_eq!(
        body["result"]["result_stack"],
        json!([ { "int_value": 42 } ])
    );
    server.stop().await;
}

#[tokio::test]
async fn test_rich_types_over_http() {
    let server = TestServer::start(ServeOptions::default()).await;
    let zoned = json!({
        "zoned_datetime_value": {
            "iso8601": "2020-06-05T10:15:00-07:00[America/Los_Angeles]",
            "timezone": "America/Los_Angeles"
        }
    });
    let time = json!({ "plain_time_value": { "iso8601_time": "09:30:00" } });
    let (status, body) = server
        .rpc(
            "executeWord",
            json!({ "word_name": "SWAP", "stack": [zoned, time] }),
        )
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["result"]["result_stack"], json!([time, zoned]));
    server.stop().await;
}

#[tokio::test]
async fn test_unknown_word_runtime_error_shape() {
    let server = TestServer::start(ServeOptions::default()).await;
    let (status, body) = server
        .rpc(
            "executeWord",
            json!({ "word_name": "NO-SUCH-WORD", "stack": [] }),
        )
        .await;
    // Protocol-level errors ride HTTP 200; the error lives in the envelope
    assert_eq!(status, 200);
    let error = &body["error"];
    assert_eq!(error["code"], -32000);
    assert_eq!(error["data"]["runtime"], "rust");
    assert_eq!(error["data"]["error_type"], "UnknownWord");
    assert_eq!(error["data"]["context"]["word_name"], "NO-SUCH-WORD");
    server.stop().await;
}

#[tokio::test]
async fn test_invalid_params_and_unknown_method() {
    let server = TestServer::start(ServeOptions::default()).await;

    let (status, body) = server.rpc("executeWord", json!({ "stack": [] })).await;
    assert_eq!(status, 200);
    assert_eq!(body["error"]["code"], -32602);
    assert_eq!(
        body["error"]["message"],
        "executeWord requires string \"word_name\""
    );

    let (_, body) = server.rpc("bogusMethod", json!({})).await;
    assert_eq!(body["error"]["code"], -32601);
    assert_eq!(body["error"]["message"], "Method not found: bogusMethod");

    let (_, body) = server
        .rpc("getModuleInfo", json!({ "module_name": "fs" }))
        .await;
    assert_eq!(body["error"]["code"], -32001);
    server.stop().await;
}

#[tokio::test]
async fn test_batch_requests_rejected() {
    let server = TestServer::start(ServeOptions::default()).await;
    let batch = format!("[{}]", any_valid_envelope());
    let (status, body) = server.post_json(batch, &server.endpoint.clone()).await;
    assert_eq!(status, 200);
    assert_eq!(body["error"]["code"], -32600);
    assert_eq!(body["error"]["message"], "Batch requests are not supported");
    server.stop().await;
}

#[tokio::test]
async fn test_malformed_json_is_parse_error() {
    let server = TestServer::start(ServeOptions::default()).await;
    let (status, body) = server
        .post_json("{oops".to_string(), &server.endpoint.clone())
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["error"]["code"], -32700);
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .starts_with("Parse error:"),
        "got: {}",
        body["error"]["message"]
    );
    server.stop().await;
}

#[tokio::test]
async fn test_invalid_envelopes_rejected() {
    let server = TestServer::start(ServeOptions::default()).await;
    let cases = [
        json!({ "jsonrpc": "2.0", "method": "listModules" }), // no id
        json!({ "jsonrpc": "1.0", "id": 1, "method": "listModules" }), // wrong version
        json!({ "id": 1, "method": "listModules" }),          // no jsonrpc
        json!({ "jsonrpc": "2.0", "id": 1, "method": 42 }),   // non-string method
        json!("just a string"),
        json!(42),
    ];
    for envelope in cases {
        let (status, body) = server
            .post_json(envelope.to_string(), &server.endpoint.clone())
            .await;
        assert_eq!(status, 200, "envelope: {envelope}");
        assert_eq!(body["error"]["code"], -32600, "envelope: {envelope}");
        assert_eq!(
            body["error"]["message"], "Invalid JSON-RPC 2.0 request",
            "envelope: {envelope}"
        );
    }
    server.stop().await;
}

#[tokio::test]
async fn test_null_id_is_a_valid_envelope() {
    let server = TestServer::start(ServeOptions::default()).await;
    let envelope = json!({ "jsonrpc": "2.0", "id": null, "method": "listModules", "params": {} });
    let (status, body) = server
        .post_json(envelope.to_string(), &server.endpoint.clone())
        .await;
    assert_eq!(status, 200);
    assert!(body.get("result").is_some(), "got: {body}");
    assert_eq!(body["id"], json!(null));
    server.stop().await;
}

#[tokio::test]
async fn test_id_echoes_back() {
    let server = TestServer::start(ServeOptions::default()).await;
    let envelope =
        json!({ "jsonrpc": "2.0", "id": "abc-123", "method": "listModules", "params": {} });
    let (_, body) = server
        .post_json(envelope.to_string(), &server.endpoint.clone())
        .await;
    assert_eq!(body["id"], "abc-123");
    server.stop().await;
}

#[tokio::test]
async fn test_transport_rejections() {
    let server = TestServer::start(ServeOptions::default()).await;

    // GET -> 405 with Allow: POST
    let client = reqwest::Client::new();
    let response = client.get(&server.endpoint).send().await.unwrap();
    assert_eq!(response.status(), 405);
    assert_eq!(
        response
            .headers()
            .get("allow")
            .and_then(|v| v.to_str().ok()),
        Some("POST")
    );

    // Unknown path -> 404
    let url = format!("http://{}/nope", server.addr());
    let (status, _) = server.post_json(any_valid_envelope(), &url).await;
    assert_eq!(status, 404);

    // Wrong content type -> 415
    let (status, _) = server.post_raw(any_valid_envelope(), "text/plain").await;
    assert_eq!(status, 415);

    // Root path "/" also serves RPC
    let url = format!("http://{}/", server.addr());
    let (status, body) = server.post_json(any_valid_envelope(), &url).await;
    assert_eq!(status, 200);
    assert!(body.get("result").is_some());

    server.stop().await;
}
