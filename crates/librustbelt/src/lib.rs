//! librustbelt - Core library for rustbelt
//!
//! Provides simple interfaces for AI tools to interact with Rust code.

pub mod analyzer;

pub use analyzer::{DefinitionInfo, FileChange, RenameResult, RustAnalyzerish, TextEdit, TypeHint};
