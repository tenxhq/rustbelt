//! librustbelt - Core library for rustbelt
//!
//! Provides simple interfaces for AI tools to interact with Rust code.

pub mod analyzer;
mod entities;

pub use analyzer::RustAnalyzerish;
pub use entities::{DefinitionInfo, FileChange, RenameResult, TextEdit, TypeHint};
