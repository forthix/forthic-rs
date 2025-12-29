//! Standard Forthic modules
//!
//! This module contains the standard library modules for Forthic:
//! - **boolean**: Comparison, logic, and membership operations
//! - **math**: Arithmetic operations (to be implemented)
//! - **core**: Stack operations (to be implemented)
//! - **array**: Data transformation (to be implemented)
//! - **record**: Dictionary operations (to be implemented)
//! - **string**: Text processing (to be implemented)
//! - **json**: Serialization (to be implemented)
//! - **datetime**: Date/time operations (to be implemented)

pub mod array;
pub mod boolean;
pub mod core;
pub mod datetime;
pub mod json;
pub mod math;
pub mod record;
pub mod string;

pub use array::ArrayModule;
pub use boolean::BooleanModule;
pub use core::CoreModule;
pub use datetime::DateTimeModule;
pub use json::JSONModule;
pub use math::MathModule;
pub use record::RecordModule;
pub use string::StringModule;
