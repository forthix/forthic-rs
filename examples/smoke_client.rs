//! Cross-runtime smoke: drive a remote Forthic JSON-RPC server with the rs
//! JsonRpcClient. The client-side half of the wire-compatibility proof
//! (scripts/smoke_ts_client.sh proves the server side).
//!
//! Usage: smoke_client <port> <expected-runtime> <expected-error-type>
//!   e.g. smoke_client 18996 typescript UnknownWordError
//!        smoke_client 18995 python UnknownWordError

use chrono::TimeZone;
use forthic::jsonrpc::{ClientError, JsonRpcClient};
use forthic::literals::ForthicValue;

fn check(cond: bool, message: &str) {
    if !cond {
        eprintln!("SMOKE FAILED: {message}");
        std::process::exit(1);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (port, expected_runtime, expected_error_type) = match &args[..] {
        [_, port, runtime, error_type] => (port.clone(), runtime.clone(), error_type.clone()),
        _ => {
            eprintln!("usage: smoke_client <port> <expected-runtime> <expected-error-type>");
            std::process::exit(2);
        }
    };
    let client = JsonRpcClient::new(&format!("127.0.0.1:{port}")).expect("client");

    // 1. Mixed-type stack round-trips through the remote runtime
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
            ForthicValue::Array(vec![
                ForthicValue::Int(2),
                ForthicValue::Record(
                    [(
                        "deep".to_string(),
                        ForthicValue::String("record".to_string()),
                    )]
                    .into_iter()
                    .collect(),
                ),
            ]),
        ]),
        ForthicValue::Date(chrono::NaiveDate::from_ymd_opt(2020, 6, 5).unwrap()),
        ForthicValue::Time(chrono::NaiveTime::from_hms_opt(9, 30, 0).unwrap()),
        ForthicValue::DateTime(zoned),
    ];
    let result = client.execute_word("DUP", &stack).expect("executeWord");
    check(
        result.len() == stack.len() + 1,
        &format!(
            "DUP: expected {} items, got {}",
            stack.len() + 1,
            result.len()
        ),
    );
    check(result[..stack.len()] == stack[..], "stack round-tripped");
    check(
        result[stack.len()] == stack[stack.len() - 1],
        "DUP duplicated the zoned datetime",
    );

    // 2. executeSequence
    let seq = client
        .execute_sequence(&["DUP", "+"], &[ForthicValue::Int(21)])
        .expect("executeSequence");
    check(
        seq == vec![ForthicValue::Int(42)],
        "executeSequence: expected [42]",
    );

    // 3. listModules
    let _modules = client.list_modules().expect("listModules");

    // 4. Rich errors surface with the remote runtime's metadata
    match client.execute_word("NO-SUCH-WORD", &[]) {
        Err(ClientError::Remote(info)) => {
            check(
                info.runtime == expected_runtime,
                &format!(
                    "error runtime: {} (expected {expected_runtime})",
                    info.runtime
                ),
            );
            check(
                info.error_type == expected_error_type,
                &format!(
                    "error type: {} (expected {expected_error_type})",
                    info.error_type
                ),
            );
            check(
                info.context.get("word_name").map(String::as_str) == Some("NO-SUCH-WORD"),
                "error context intact",
            );
        }
        other => check(false, &format!("expected Remote error, got {other:?}")),
    }

    println!("cross-runtime smoke OK (rs client <-> {expected_runtime} server)");
}
