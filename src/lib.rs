//! Forthic - A stack-based, concatenative language for composable transformations
//!
//! Forthic is designed to enable powerful data transformations and orchestration
//! across multiple runtime environments.

pub mod errors;
pub mod interpreter;
pub mod literals;
pub mod module;
pub mod tokenizer;
pub mod utils;
pub mod word_options;

// Re-export commonly used types
pub use errors::{CodeLocation, ForthicError};
pub use interpreter::{Interpreter, Stack};
pub use literals::ForthicValue;
pub use module::{Module, Variable, Word};
pub use tokenizer::{Token, TokenType, Tokenizer};
pub use word_options::WordOptions;

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::errors::{CodeLocation, ForthicError};
    pub use crate::interpreter::{Interpreter, Stack};
    pub use crate::literals::{ForthicValue, LiteralHandler};
    pub use crate::module::{Module, Variable, Word};
    pub use crate::tokenizer::{Token, TokenType, Tokenizer};
    pub use crate::utils;
    pub use crate::word_options::WordOptions;
}
