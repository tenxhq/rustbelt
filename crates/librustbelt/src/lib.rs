//! librustbelt - Core library for rustbelt MCP mcp
//!
//! Provides rust-analyzer functionality and ruskel code skeleton generation

pub mod analyzer;
// pub mod ruskel {
//     pub mod ruskel_analyzer;
// }

pub use analyzer::{DefinitionInfo, FileChange, RenameResult, RustAnalyzerish, TextEdit, TypeHint};
// pub use ruskel::ruskel_analyzer::RuskelAnalyzer;
