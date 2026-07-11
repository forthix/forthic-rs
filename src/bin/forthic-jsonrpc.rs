//! Forthic JSON-RPC server binary
//!
//! ```text
//! forthic-jsonrpc [--port 8765] [--host 127.0.0.1] [--token SECRET]
//! ```
//!
//! Defaults are conservative (loopback only, no auth needed). Binding a
//! non-loopback host without --token logs a security warning; the server
//! executes caller-supplied Forthic code.

use forthic::jsonrpc::{serve, ServeOptions};

fn flag_value(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let port: u16 = flag_value(&args, "--port")
        .and_then(|v| v.parse().ok())
        .unwrap_or(8765);
    let options = ServeOptions {
        host: flag_value(&args, "--host"),
        token: flag_value(&args, "--token"),
        ..ServeOptions::default()
    };

    match serve(port, options).await {
        Ok(handle) => {
            println!("Forthic JSON-RPC server listening on {}", handle.addr());
            tokio::signal::ctrl_c()
                .await
                .expect("failed to listen for ctrl-c");
            println!("Shutting down");
            handle.shutdown().await;
        }
        Err(e) => {
            eprintln!("Fatal error: {e}");
            std::process::exit(1);
        }
    }
}
