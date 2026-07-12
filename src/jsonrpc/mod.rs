//! JSON-RPC 2.0 multi-runtime support (feature = "jsonrpc")
//!
//! Wire-format compatible with forthic-ts (`src/jsonrpc/` + `src/grpc/serializer.ts`)
//! so a forthic-ts `JsonRpcClient` can call the Rust runtime unchanged.
//! See `plans/JSONRPC-PLAN.md` for the full design.
//!
pub mod client;
pub mod errors;
pub mod http;
pub mod serializer;
pub mod server;

pub use client::{ClientError, JsonRpcClient, RemoteErrorInfo};
pub use errors::{ErrorInfo, JsonRpcErrorCode, MethodError};
pub use http::{serve, ServeOptions, ServerHandle};
pub use serializer::{deserialize_value, serialize_value, SerializerError};
pub use server::{dispatch, ForthicJsonRpcServicer, JsonRpcRequest};
