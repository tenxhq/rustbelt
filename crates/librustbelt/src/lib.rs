//! librustbelt - Core library for rustbelt
//!
//! Provides simple interfaces for AI tools to interact with Rust code.

pub mod analyzer;
pub mod entities;
pub mod file_watcher;

pub use analyzer::RustAnalyzerish;
pub use entities::{
    CompletionItem, CursorCoordinates, DefinitionInfo, FileChange, ReferenceInfo, RenameResult,
    TextEdit, TypeHint,
};
