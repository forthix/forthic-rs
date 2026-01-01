# Forthic Rust Runtime

A Rust implementation of the Forthic stack-based concatenative programming language.

## Overview

Forthic is a stack-based, concatenative language designed for composable transformations. This is the official **Rust** runtime implementation, providing full compatibility with other Forthic runtimes while leveraging Rust's memory safety and zero-cost abstractions for high performance.

**[Learn more at forthix.com →](https://forthix.com)**

## Features

* ✅ Complete Forthic language implementation
* ✅ All 8 standard library modules
* ✅ Memory-safe, thread-safe execution environment
* ✅ Seamless interop with Rust types


## Development

```bash
# Compile
cargo build

# Run tests
cargo test

# Check types and borrow checker
cargo check

# Run with optimizations
cargo run --release

```

## Standard Library Modules

* **core**: Stack operations, variables, control flow
* **array**: Data transformation (MAP, SELECT, SORT, etc.)
* **record**: Dictionary/HashMap operations
* **string**: Text processing
* **math**: Arithmetic operations
* **boolean**: Logical operations
* **datetime**: Date/time manipulation (using `chrono`)
* **json**: JSON serialization (using `serde_json`)

## License

BSD 2-CLAUSE

## Links

* **[forthix.com](https://forthix.com)** - Learn about Forthic and Categorical Coding
* **[Category Theory for Coders](https://forthix.com/blog/category-theory-for-the-rest-of-us-coders)** - Understand the foundations
* [Forthic Language Specification](https://github.com/forthix/forthic)
* [TypeScript Runtime](https://github.com/forthix/forthic-ts) (reference implementation)

