//! Shared harness for JSON-RPC server tests
//!
//! Each test starts its own server on port 0 (free port) and shuts it down
//! gracefully, so tests are isolated and parallel-safe. Note: tests pass
//! ServeOptions explicitly rather than via FORTHIC_JSONRPC_* env vars —
//! env is process-global and cargo runs tests in parallel threads.
//!
//! (dead_code allowed: this module compiles once per including test crate,
//! and not every crate uses every helper.)
#![allow(dead_code)]

use forthic::jsonrpc::{serve, ServeOptions, ServerHandle};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicI64, Ordering};

pub struct TestServer {
    handle: Option<ServerHandle>,
    pub endpoint: String,
    client: reqwest::Client,
    next_id: AtomicI64,
}

impl TestServer {
    pub async fn start(options: ServeOptions) -> Self {
        let handle = serve(0, options).await.expect("server starts");
        let endpoint = format!("http://{}/rpc", handle.addr());
        Self {
            handle: Some(handle),
            endpoint,
            client: reqwest::Client::new(),
            next_id: AtomicI64::new(1),
        }
    }

    pub fn addr(&self) -> std::net::SocketAddr {
        self.handle.as_ref().expect("server running").addr()
    }

    /// Well-formed JSON-RPC call; returns (HTTP status, parsed body)
    pub async fn rpc(&self, method: &str, params: Value) -> (reqwest::StatusCode, Value) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let body = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        self.post_json(body.to_string(), &self.endpoint).await
    }

    /// Raw body POST with an arbitrary content type; returns (status, text)
    pub async fn post_raw(
        &self,
        body: impl Into<reqwest::Body>,
        content_type: &str,
    ) -> (reqwest::StatusCode, String) {
        let response = self
            .client
            .post(&self.endpoint)
            .header("Content-Type", content_type)
            .body(body)
            .send()
            .await
            .expect("request sends");
        let status = response.status();
        let text = response.text().await.expect("body reads");
        (status, text)
    }

    /// POST a JSON string to an arbitrary URL on this server
    pub async fn post_json(&self, body: String, url: &str) -> (reqwest::StatusCode, Value) {
        let response = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .expect("request sends");
        let status = response.status();
        let text = response.text().await.expect("body reads");
        let value = serde_json::from_str(&text)
            .unwrap_or_else(|_| json!({ "_raw": text }));
        (status, value)
    }

    /// Builder access for requests needing custom headers (auth tests)
    pub fn request(&self) -> reqwest::RequestBuilder {
        self.client
            .post(&self.endpoint)
            .header("Content-Type", "application/json")
    }

    pub async fn stop(mut self) {
        if let Some(handle) = self.handle.take() {
            handle.shutdown().await;
        }
    }
}

/// A syntactically valid listModules envelope, for tests where the payload
/// content doesn't matter
pub fn any_valid_envelope() -> String {
    json!({ "jsonrpc": "2.0", "id": 1, "method": "listModules", "params": {} }).to_string()
}
