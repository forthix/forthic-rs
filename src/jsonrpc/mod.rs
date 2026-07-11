//! JSON-RPC 2.0 multi-runtime support (feature = "jsonrpc")
//!
//! Wire-format compatible with forthic-ts (`src/jsonrpc/` + `src/grpc/serializer.ts`)
//! so a forthic-ts `JsonRpcClient` can call the Rust runtime unchanged.
//! See `plans/JSONRPC-PLAN.md` for the full design.
//!
//! Phase 1: the `StackValue` serializer. Later phases add errors, the servicer,
//! and the HTTP server.

pub mod serializer;

pub use serializer::{deserialize_value, serialize_value, SerializerError};
