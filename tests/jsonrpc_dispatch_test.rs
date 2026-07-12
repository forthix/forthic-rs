//! Servicer + dispatch tests (transport-independent)
//!
//! Exercises the four RPC methods through `dispatch` with raw JSON-RPC
//! envelopes, asserting the exact wire shapes forthic-ts's JsonRpcClient
//! expects. HTTP-level behavior (auth, body limits, envelope validation)
//! is Phase 3 and tested there.

// ForthicError is large; accepted trade-off (see lib.rs / backlog item 11)
#![allow(clippy::result_large_err)]

use forthic::jsonrpc::{dispatch, ForthicJsonRpcServicer, JsonRpcRequest};
use serde_json::{json, Value};

fn rpc(method: &str, params: Value) -> Value {
    rpc_with_options(method, params, false)
}

fn rpc_with_options(method: &str, params: Value, expose_error_details: bool) -> Value {
    let servicer = ForthicJsonRpcServicer::new();
    let request: JsonRpcRequest = serde_json::from_value(
        json!({ "jsonrpc": "2.0", "id": 7, "method": method, "params": params }),
    )
    .expect("request parses");
    dispatch(&servicer, &request, expose_error_details)
}

fn error_of(response: &Value) -> &Value {
    response.get("error").expect("expected an error response")
}

// ===== Envelope basics =====

#[test]
fn test_response_echoes_envelope() {
    let response = rpc("listModules", json!({}));
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 7);
    assert!(response.get("error").is_none());
}

#[test]
fn test_string_and_null_ids_echo_back() {
    let servicer = ForthicJsonRpcServicer::new();
    for id in [json!("abc-123"), json!(null)] {
        let request: JsonRpcRequest = serde_json::from_value(
            json!({ "jsonrpc": "2.0", "id": id, "method": "listModules", "params": {} }),
        )
        .unwrap();
        assert_eq!(dispatch(&servicer, &request, false)["id"], id);
    }
}

#[test]
fn test_unknown_method() {
    let response = rpc("bogusMethod", json!({}));
    let error = error_of(&response);
    assert_eq!(error["code"], -32601);
    assert_eq!(error["message"], "Method not found: bogusMethod");
}

// ===== executeWord =====

#[test]
fn test_execute_word_runs_on_supplied_stack() {
    let response = rpc(
        "executeWord",
        json!({
            "word_name": "SWAP",
            "stack": [ { "int_value": 1 }, { "int_value": 2 } ]
        }),
    );
    assert_eq!(
        response["result"]["result_stack"],
        json!([ { "int_value": 2 }, { "int_value": 1 } ])
    );
}

#[test]
fn test_execute_word_reaches_standard_library() {
    // "+" comes from the math module — proves Interpreter::standard wires
    // the stdlib into the servicer's fresh interpreters
    let response = rpc(
        "executeWord",
        json!({
            "word_name": "+",
            "stack": [ { "int_value": 40 }, { "int_value": 2 } ]
        }),
    );
    assert_eq!(
        response["result"]["result_stack"],
        json!([ { "int_value": 42 } ])
    );
}

#[test]
fn test_execute_word_param_validation() {
    let cases = [
        (
            json!({ "stack": [] }),
            "executeWord requires string \"word_name\"",
        ),
        (
            json!({ "word_name": 42, "stack": [] }),
            "executeWord requires string \"word_name\"",
        ),
        (
            json!({ "word_name": "DUP" }),
            "executeWord requires array \"stack\"",
        ),
        (
            json!({ "word_name": "DUP", "stack": "nope" }),
            "executeWord requires array \"stack\"",
        ),
        (json!(null), "executeWord requires string \"word_name\""),
    ];
    for (params, expected_message) in cases {
        let response = rpc("executeWord", params.clone());
        let error = error_of(&response);
        assert_eq!(error["code"], -32602, "params: {params}");
        assert_eq!(error["message"], expected_message, "params: {params}");
    }
}

#[test]
fn test_execute_word_unknown_word_is_runtime_error() {
    let response = rpc(
        "executeWord",
        json!({ "word_name": "NO-SUCH-WORD", "stack": [] }),
    );
    let error = error_of(&response);
    assert_eq!(error["code"], -32000);
    let data = &error["data"];
    assert_eq!(data["runtime"], "rust");
    assert_eq!(data["error_type"], "UnknownWord");
    assert_eq!(data["context"]["word_name"], "NO-SUCH-WORD");
    assert_eq!(data["message"], error["message"]);
}

#[test]
fn test_execute_word_bad_stack_item_is_runtime_error() {
    let response = rpc(
        "executeWord",
        json!({ "word_name": "DUP", "stack": [ { "bogus_value": 1 } ] }),
    );
    let error = error_of(&response);
    assert_eq!(error["code"], -32000);
    assert_eq!(error["data"]["error_type"], "SerializerError");
}

#[test]
fn test_execute_word_time_literal_round_trips() {
    // "9:30" parses as a Time literal; times cross the wire as
    // plain_time_value since the coordinated ts/rs extension
    let response = rpc("executeWord", json!({ "word_name": "9:30", "stack": [] }));
    assert_eq!(
        response["result"]["result_stack"],
        json!([ { "plain_time_value": { "iso8601_time": "09:30:00" } } ])
    );
}

#[test]
fn test_execute_word_unserializable_result_is_runtime_error() {
    // A bare "[" leaves the interpreter-internal StartArrayMarker on the
    // stack — exercises the result-serialization error path
    let response = rpc("executeWord", json!({ "word_name": "[", "stack": [] }));
    let error = error_of(&response);
    assert_eq!(error["code"], -32000);
    assert_eq!(error["data"]["error_type"], "SerializerError");
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("Unsupported Forthic type"),
        "got: {}",
        error["message"]
    );
}

// ===== executeSequence =====

#[test]
fn test_execute_sequence_runs_words_in_order() {
    let response = rpc(
        "executeSequence",
        json!({
            "word_names": ["DUP", "+"],
            "stack": [ { "int_value": 21 } ]
        }),
    );
    assert_eq!(
        response["result"]["result_stack"],
        json!([ { "int_value": 42 } ])
    );
}

#[test]
fn test_execute_sequence_param_validation() {
    let cases = [
        (
            json!({ "stack": [] }),
            "executeSequence requires string[] \"word_names\"",
        ),
        (
            json!({ "word_names": ["DUP", 5], "stack": [] }),
            "executeSequence requires string[] \"word_names\"",
        ),
        (
            json!({ "word_names": "DUP", "stack": [] }),
            "executeSequence requires string[] \"word_names\"",
        ),
        (
            json!({ "word_names": ["DUP"] }),
            "executeSequence requires array \"stack\"",
        ),
    ];
    for (params, expected_message) in cases {
        let response = rpc("executeSequence", params.clone());
        let error = error_of(&response);
        assert_eq!(error["code"], -32602, "params: {params}");
        assert_eq!(error["message"], expected_message, "params: {params}");
    }
}

#[test]
fn test_execute_sequence_error_carries_word_sequence_context() {
    let response = rpc(
        "executeSequence",
        json!({ "word_names": ["DUP", "NO-SUCH-WORD"], "stack": [ { "int_value": 1 } ] }),
    );
    let error = error_of(&response);
    assert_eq!(error["code"], -32000);
    assert_eq!(
        error["data"]["context"]["word_sequence"],
        "DUP, NO-SUCH-WORD"
    );
    assert!(error["data"]["context"].get("word_name").is_none());
}

// ===== listModules / getModuleInfo =====

#[test]
fn test_list_modules_is_empty_for_now() {
    // No runtime-specific modules registered yet (ts has fs; rs doesn't)
    let response = rpc("listModules", json!({}));
    assert_eq!(response["result"]["modules"], json!([]));
}

#[test]
fn test_get_module_info_unknown_module() {
    let response = rpc("getModuleInfo", json!({ "module_name": "fs" }));
    let error = error_of(&response);
    assert_eq!(error["code"], -32001);
    assert_eq!(error["message"], "Module 'fs' not found");
}

#[test]
fn test_get_module_info_param_validation() {
    let response = rpc("getModuleInfo", json!({}));
    let error = error_of(&response);
    assert_eq!(error["code"], -32602);
    assert_eq!(
        error["message"],
        "getModuleInfo requires string \"module_name\""
    );
}

// ===== Error-detail sanitization =====

// Both tokenizer errors (unterminated string) and interpreter errors
// (unknown word — located since the word-locations work) are probed, so
// stripping is verified against errors that genuinely have something to
// strip.

#[test]
fn test_error_details_stripped_by_default() {
    for word_name in ["'unterminated", "NO-SUCH-WORD"] {
        let response = rpc(
            "executeWord",
            json!({ "word_name": word_name, "stack": [] }),
        );
        let data = &error_of(&response)["data"];
        assert!(
            data.get("word_location").is_none(),
            "stripped for {word_name}: {data}"
        );
        assert!(data.get("stack_trace").is_none());
    }
}

#[test]
fn test_error_details_exposed_on_request() {
    for (word_name, error_type) in [
        ("'unterminated", "UnterminatedString"),
        ("NO-SUCH-WORD", "UnknownWord"),
    ] {
        let response = rpc_with_options(
            "executeWord",
            json!({ "word_name": word_name, "stack": [] }),
            true,
        );
        let data = &error_of(&response)["data"];
        assert_eq!(data["error_type"], error_type);
        assert!(
            data.get("word_location").and_then(Value::as_str).is_some(),
            "expected word_location for {word_name} in {data}"
        );
    }
}

// ===== Round-trip through execution =====

#[test]
fn test_rich_values_round_trip_through_execution() {
    // A zoned datetime survives deserialize → stack → serialize untouched
    let zoned = json!({
        "zoned_datetime_value": {
            "iso8601": "2020-06-05T10:15:00-07:00[America/Los_Angeles]",
            "timezone": "America/Los_Angeles"
        }
    });
    let response = rpc(
        "executeWord",
        json!({ "word_name": "DUP", "stack": [ zoned ] }),
    );
    assert_eq!(response["result"]["result_stack"], json!([zoned, zoned]));
}

#[test]
fn test_get_module_info_returns_real_word_docs() {
    use forthic::module::{Module, ModuleWord};
    use std::sync::Arc;

    let mut module = Module::new("host".to_string());
    module.add_exportable_word(Arc::new(ModuleWord::with_doc(
        "GREET".to_string(),
        |context| {
            context.stack_push(forthic::literals::ForthicValue::String("hi".to_string()));
            Ok(())
        },
        "( -- greeting:string )",
        "Push a greeting",
    )));
    // Doc-less direct registration falls back to the placeholder shape
    module.add_exportable_word(Arc::new(ModuleWord::new("MYSTERY".to_string(), |_| Ok(()))));

    let mut servicer = ForthicJsonRpcServicer::new();
    servicer.add_runtime_module(module);
    let request: JsonRpcRequest = serde_json::from_value(serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "getModuleInfo",
        "params": { "module_name": "host" }
    }))
    .unwrap();
    let response = dispatch(&servicer, &request, false);

    let words = response["result"]["words"].as_array().unwrap();
    assert_eq!(words[0]["name"], "GREET");
    assert_eq!(words[0]["stack_effect"], "( -- greeting:string )");
    assert_eq!(words[0]["description"], "Push a greeting");
    assert_eq!(words[1]["name"], "MYSTERY");
    assert_eq!(words[1]["stack_effect"], "( -- )");
}
