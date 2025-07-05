//! rustbelt MCP Server
//!
//! This mcp provides rust-analyzer functionality via the Model Context
//! Protocol (MCP). It exposes IDE capabilities like type hints,
//! go-to-definition, and more as MCP tools.

use libruskel::Ruskel;
use librustbelt::{RustAnalyzerish, entities::CursorCoordinates};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tenx_mcp::{Result, ServerCtx, mcp_server, schema::*, schemars, tool};
use tokio::sync::Mutex;
use tracing::info;

pub const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "-",
    env!("VERGEN_GIT_SHA"),
    " (",
    env!("VERGEN_BUILD_DATE"),
    ")"
);

/// Parameters for the rename_symbol tool
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RenameParams {
    // TODO Do not nest CursorCoordinates here until tenx-mcp properly reports schema
    /// Absolute path to the Rust source file
    pub file_path: String,
    /// Line number (1-based)
    pub line: u32,
    /// Column number (1-based)
    pub column: u32,
    /// New name for the symbol
    pub new_name: String,
}

/// Parameters for the ruskel tool
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RuskelParams {
    /// Target specification (crate path, published crate name, or module path)
    pub target: String,
    /// Optional specific features to enable
    #[serde(default)]
    pub features: Vec<String>,
    /// Enable all features
    #[serde(default)]
    pub all_features: bool,
    /// Disable default features
    #[serde(default)]
    pub no_default_features: bool,
    /// Include private items in the skeleton
    #[serde(default)]
    pub private: bool,
}

/// Parameters for the view_inlay_hints tool
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ViewInlayHintsParams {
    /// Absolute path to the Rust source file
    pub file_path: String,
    /// Optional starting line number (1-based, inclusive)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_line: Option<u32>,
    /// Optional ending line number (1-based, inclusive)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u32>,
}

/// Rust-Analyzer MCP server connection
#[derive(Debug, Clone, Default)]
pub struct Rustbelt {
    analyzer: Arc<Mutex<RustAnalyzerish>>,
}

impl Rustbelt {
    fn new() -> Self {
        Self {
            analyzer: Arc::new(Mutex::new(RustAnalyzerish::new())),
        }
    }
}

#[mcp_server]
impl Rustbelt {
    /// Generate a Rust code skeleton for a crate, showing its public API structure
    /// returns a single Rust source file that lists the
    /// *public API (or optionally private items) of any crate or module path, with all
    /// bodies stripped*. Useful for large‑language models that need to look up item
    /// names, signatures, derives, feature‑gated cfgs, and doc‑comments while writing
    /// or reviewing Rust code.
    ///
    /// ### When a model should call this tool
    /// 1. It needs a function/trait/struct signature it can't recall.
    /// 2. The user asks for examples or docs from a crate.
    /// 3. The model wants to verify what features gate a symbol.
    ///
    /// ### Target syntax examples
    /// - `serde`               →  latest serde on crates.io
    /// - `serde@1.0.160`      →  specific published version
    /// - `serde::de::Deserialize` →  narrow output to one module/type for small contexts
    /// - `/path/to/crate` or `/path/to/crate::submod` →  local workspace paths
    ///
    /// ### Output format
    /// Plain UTF‑8 text containing valid Rust code, with implementation omitted.
    ///
    /// ### Tips for LLMs
    /// - Request deep module paths (e.g. `tokio::sync::mpsc`) to keep the reply below
    ///   your token budget.
    /// - Pass `all_features=true` or `features=[…]` when a symbol is behind a feature gate.
    #[tool]
    async fn ruskel(&self, _ctx: &ServerCtx, params: RuskelParams) -> Result<CallToolResult> {
        let ruskel = Ruskel::new();

        match ruskel.render(
            &params.target,
            params.no_default_features,
            params.all_features,
            params.features.to_vec(),
            params.private,
        ) {
            Ok(skeleton) => Ok(CallToolResult::new()
                .with_text_content(skeleton)
                .is_error(false)),
            Err(e) => Ok(CallToolResult::new()
                .with_text_content(format!("Error generating skeleton: {e}"))
                .is_error(true)),
        }
    }

    /// Get type information for a symbol at a specific position in Rust code
    ///
    /// Provides detailed type information including variable types, function signatures,
    /// struct/enum definitions, and generic parameters. Use this when you need to understand
    /// the type of a symbol for code analysis, refactoring, or generating type-aware code.
    ///
    /// Returns human-readable type information or indicates if no type data is available.
    #[tool]
    async fn get_type_hint(
        &self,
        _ctx: &ServerCtx,
        cursor: CursorCoordinates,
    ) -> Result<CallToolResult> {
        match self.analyzer.lock().await.get_type_hint(&cursor).await {
            Ok(Some(type_info)) => Ok(CallToolResult::new()
                .with_text_content(type_info.to_string())
                .is_error(false)),
            Ok(None) => Ok(CallToolResult::new()
                .with_text_content("No type information available at this position")
                .is_error(false)),
            Err(e) => Ok(CallToolResult::new()
                .with_text_content(format!("Error getting type hint: {e}"))
                .is_error(true)),
        }
    }

    /// Get definition location for a symbol at a specific position in Rust code
    ///
    /// Finds where symbols are defined - functions, types, variables, modules, macros,
    /// and more. Essential for code navigation and understanding symbol relationships.
    ///
    /// Returns definition locations as "file_path:line_number:column_number" format,
    /// or indicates if no definitions are found.
    #[tool]
    async fn get_definition(
        &self,
        _ctx: &ServerCtx,
        cursor: CursorCoordinates,
    ) -> Result<CallToolResult> {
        match self.analyzer.lock().await.get_definition(&cursor).await {
            Ok(Some(definitions)) => {
                let result_text = definitions
                    .iter()
                    .map(|def| def.to_string())
                    .collect::<Vec<_>>()
                    .join("\n");

                Ok(CallToolResult::new()
                    .with_text_content(result_text)
                    .is_error(false))
            }
            Ok(None) => Ok(CallToolResult::new()
                .with_text_content("No definitions found at this position")
                .is_error(false)),
            Err(e) => Ok(CallToolResult::new()
                .with_text_content(format!("Error getting definitions: {e}"))
                .is_error(true)),
        }
    }

    /// Get completion suggestions at a specific position in Rust code
    ///
    /// Provides intelligent code completion suggestions including available methods,
    /// functions, variables, keywords, imports, and more based on the current context.
    ///
    /// Returns a list of completion suggestions with types and descriptions.
    #[tool]
    async fn get_completions(
        &self,
        _ctx: &ServerCtx,
        cursor: CursorCoordinates,
    ) -> Result<CallToolResult> {
        match self.analyzer.lock().await.get_completions(&cursor).await {
            Ok(Some(completions)) => {
                let result_text = completions
                    .iter()
                    .map(|comp| comp.to_string())
                    .collect::<Vec<_>>()
                    .join("\n");

                Ok(CallToolResult::new()
                    .with_text_content(result_text)
                    .is_error(false))
            }
            Ok(None) => Ok(CallToolResult::new()
                .with_text_content("No completions found at this position")
                .is_error(false)),
            Err(e) => Ok(CallToolResult::new()
                .with_text_content(format!("Error getting completions: {e}"))
                .is_error(true)),
        }
    }

    /// Rename a symbol across the workspace
    ///
    /// Performs intelligent, workspace-wide symbol renaming that preserves code
    /// correctness and updates all references. Works with functions, types, variables,
    /// modules, macros, and more.
    ///
    /// Returns a summary of all changes made with file paths and line numbers, or
    /// explains why the rename is not possible.
    #[tool]
    async fn rename_symbol(
        &self,
        _ctx: &ServerCtx,
        params: RenameParams,
    ) -> Result<CallToolResult> {
        let cursor = CursorCoordinates {
            file_path: params.file_path,
            line: params.line,
            column: params.column,
        };
        match self
            .analyzer
            .lock()
            .await
            .rename_symbol(&cursor, &params.new_name)
            .await
        {
            Ok(Some(rename_result)) => {
                let result_text = rename_result.to_string();

                Ok(CallToolResult::new()
                    .with_text_content(result_text)
                    .is_error(false))
            }
            Ok(None) => Ok(CallToolResult::new()
                .with_text_content("Symbol cannot be renamed at this position")
                .is_error(false)),
            Err(e) => Ok(CallToolResult::new()
                .with_text_content(format!("Error performing rename: {e}"))
                .is_error(true)),
        }
    }

    /// View a Rust file with inlay hints embedded
    ///
    /// Enhances code readability by displaying inline type annotations and other
    /// helpful hints directly within the source code, including inferred types,
    /// parameter names, return types, and implicit conversions.
    ///
    /// If start_line and end_line are provided, only the specified range of lines
    /// will be returned with inlay hints. Both parameters are 1-based and inclusive.
    /// If neither parameter is provided, the entire file is processed.
    ///
    /// Returns the source file content (full file or specified range) with inlay hints embedded as inline annotations.
    #[tool]
    async fn view_inlay_hints(
        &self,
        _ctx: &ServerCtx,
        params: ViewInlayHintsParams,
    ) -> Result<CallToolResult> {
        match self
            .analyzer
            .lock()
            .await
            .view_inlay_hints(&params.file_path, params.start_line, params.end_line)
            .await
        {
            Ok(annotated_content) => Ok(CallToolResult::new()
                .with_text_content(annotated_content)
                .is_error(false)),
            Err(e) => Ok(CallToolResult::new()
                .with_text_content(format!("Error viewing inlay hints: {e}"))
                .is_error(true)),
        }
    }
}

pub async fn serve_stdio() -> Result<()> {
    tenx_mcp::Server::default()
        .with_connection(Rustbelt::new)
        .serve_stdio()
        .await
}

pub async fn serve_tcp(addr: String) -> Result<()> {
    info!("Starting Rustbelt MCP server on {}", addr);

    tenx_mcp::Server::default()
        .with_connection(Rustbelt::new)
        .serve_tcp(addr)
        .await
}
