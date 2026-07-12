//! JsonRpcClient tests: the rs client driving the rs server in-process.
//! Cross-runtime coverage (this client vs the ts and py servers) lives in
//! scripts/smoke_ts_server.sh and scripts/smoke_py_server.sh.

#![cfg(feature = "jsonrpc")]
// ForthicError is large; accepted trade-off (see lib.rs / backlog item 11)
#![allow(clippy::result_large_err)]

mod common;

use chrono::TimeZone;
use common::TestServer;
use forthic::jsonrpc::{ClientError, JsonRpcClient, ServeOptions};
use forthic::literals::ForthicValue;

/// The client is blocking; run it off the tokio test runtime
async fn on_blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    tokio::task::spawn_blocking(f).await.expect("blocking task")
}

#[tokio::test]
async fn test_execute_word_round_trips_mixed_stack() {
    let server = TestServer::start(ServeOptions::default()).await;
    let addr = server.addr().to_string();

    let result = on_blocking(move || {
        let client = JsonRpcClient::new(&addr).expect("client");
        let la: chrono_tz::Tz = "America/Los_Angeles".parse().unwrap();
        let zoned = la.with_ymd_and_hms(2020, 6, 5, 10, 15, 0).unwrap();
        let stack = vec![
            ForthicValue::Int(42),
            ForthicValue::String("hello".to_string()),
            ForthicValue::Float(3.25),
            ForthicValue::Bool(true),
            ForthicValue::Null,
            ForthicValue::Array(vec![
                ForthicValue::Int(1),
                ForthicValue::Array(vec![ForthicValue::Int(2)]),
            ]),
            ForthicValue::Date(chrono::NaiveDate::from_ymd_opt(2020, 6, 5).unwrap()),
            ForthicValue::Time(chrono::NaiveTime::from_hms_opt(9, 30, 0).unwrap()),
            ForthicValue::DateTime(zoned),
        ];
        let result = client.execute_word("DUP", &stack).expect("executeWord");
        (stack, result)
    })
    .await;

    let (stack, result) = result;
    assert_eq!(result.len(), stack.len() + 1, "DUP added one item");
    assert_eq!(&result[..stack.len()], &stack[..], "stack round-tripped");
    assert_eq!(
        result[stack.len()],
        stack[stack.len() - 1],
        "DUP duplicated the top"
    );

    server.stop().await;
}

#[tokio::test]
async fn test_execute_sequence() {
    let server = TestServer::start(ServeOptions::default()).await;
    let addr = server.addr().to_string();

    let result = on_blocking(move || {
        let client = JsonRpcClient::new(&addr).expect("client");
        client
            .execute_sequence(&["DUP", "+"], &[ForthicValue::Int(21)])
            .expect("executeSequence")
    })
    .await;
    assert_eq!(result, vec![ForthicValue::Int(42)]);

    server.stop().await;
}

#[tokio::test]
async fn test_list_modules() {
    let server = TestServer::start(ServeOptions::default()).await;
    let addr = server.addr().to_string();

    let modules = on_blocking(move || {
        let client = JsonRpcClient::new(&addr).expect("client");
        client.list_modules().expect("listModules")
    })
    .await;
    assert!(modules.is_empty(), "no runtime-specific modules by default");

    server.stop().await;
}

#[tokio::test]
async fn test_remote_errors_carry_metadata() {
    let server = TestServer::start(ServeOptions::default()).await;
    let addr = server.addr().to_string();

    let err = on_blocking(move || {
        let client = JsonRpcClient::new(&addr).expect("client");
        client
            .execute_word("NO-SUCH-WORD", &[])
            .expect_err("unknown word errors")
    })
    .await;

    match err {
        ClientError::Remote(info) => {
            assert_eq!(info.runtime, "rust");
            assert_eq!(info.error_type, "UnknownWord");
            assert_eq!(
                info.context.get("word_name").map(String::as_str),
                Some("NO-SUCH-WORD")
            );
        }
        other => panic!("expected ClientError::Remote, got {other}"),
    }

    server.stop().await;
}

#[tokio::test]
async fn test_bearer_token_auth() {
    let server = TestServer::start(ServeOptions {
        token: Some("sekrit".to_string()),
        ..ServeOptions::default()
    })
    .await;
    let addr = server.addr().to_string();

    let (unauthorized, authorized) = on_blocking(move || {
        let no_token = JsonRpcClient::new(&addr).expect("client");
        let unauthorized = no_token.execute_word("DUP", &[ForthicValue::Int(1)]);
        let with_token = JsonRpcClient::new(&addr)
            .expect("client")
            .with_token("sekrit");
        let authorized = with_token.execute_word("DUP", &[ForthicValue::Int(1)]);
        (unauthorized, authorized)
    })
    .await;

    match unauthorized {
        Err(ClientError::Http { status: 401, .. }) => {}
        other => panic!("expected 401, got {other:?}"),
    }
    assert_eq!(
        authorized.expect("token accepted"),
        vec![ForthicValue::Int(1), ForthicValue::Int(1)]
    );

    server.stop().await;
}

#[tokio::test]
async fn test_bad_address_is_a_protocol_error() {
    match JsonRpcClient::new("not-an-address") {
        Err(ClientError::Protocol(msg)) => assert!(msg.contains("host:port"), "got: {msg}"),
        other => panic!("expected Protocol error, got {other:?}"),
    }
}
