//! Forthic - A stack-based, concatenative language for composable transformations
//!
//! Forthic is designed to enable powerful data transformations and orchestration
//! across multiple runtime environments.

pub mod errors;
pub mod literals;
pub mod utils;

// Re-export commonly used types
pub use errors::{CodeLocation, ForthicError};
pub use literals::ForthicValue;

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::errors::{CodeLocation, ForthicError};
    pub use crate::literals::{ForthicValue, LiteralHandler};
    pub use crate::utils;
}
