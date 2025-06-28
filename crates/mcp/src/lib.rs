//! rustbelt MCP Server
//!
//! This mcp provides rust-analyzer functionality via the Model Context
//! Protocol (MCP). It exposes IDE capabilities like type hints,
//! go-to-definition, and more as MCP tools.

mod ruskel;

use std::sync::Arc;

use async_trait::async_trait;
use librustbelt::RustAnalyzerish;
use serde::{Deserialize, Serialize};
use tenx_mcp::{Result, connection::Connection, error::Error, schema::*, schemars};
use tokio::sync::Mutex;
use tracing::info;

const NAME: &str = "rustbelt";

pub const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "-",
    env!("VERGEN_GIT_SHA"),
    " (",
    env!("VERGEN_BUILD_DATE"),
    ")"
);

/// Parameters for the get_type_hint tool
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TypeHintParams {
    /// Absolute path to the Rust source file
    pub file_path: String,
    /// Line number (1-based)
    pub line: u32,
    /// Column number (1-based)
    pub column: u32,
}

/// Parameters for the get_definition tool
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetDefinitionParams {
    /// Absolute path to the Rust source file
    pub file_path: String,
    /// Line number (1-based)
    pub line: u32,
    /// Column number (1-based)
    pub column: u32,
}

/// Parameters for the get_completions tool
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetCompletionsParams {
    /// Absolute path to the Rust source file
    pub file_path: String,
    /// Line number (1-based)
    pub line: u32,
    /// Column number (1-based)
    pub column: u32,
}

/// Parameters for the rename_symbol tool
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RenameParams {
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

/// Rust-Analyzer MCP mcp connection
#[derive(Debug, Clone)]
pub struct RustAnalyzerConnection {
    analyzer: Arc<Mutex<RustAnalyzerish>>,
}

impl Default for RustAnalyzerConnection {
    fn default() -> Self {
        Self {
            analyzer: Arc::new(Mutex::new(RustAnalyzerish::new())),
        }
    }
}

#[async_trait]
impl Connection for RustAnalyzerConnection {
    async fn initialize(
        &mut self,
        _protocol_version: String,
        _capabilities: ClientCapabilities,
        _client_info: Implementation,
    ) -> Result<InitializeResult> {
        Ok(InitializeResult::new(NAME, VERSION)
            .with_capabilities(ServerCapabilities::default().with_tools(None)))
    }

    async fn tools_list(&mut self) -> Result<ListToolsResult> {
        Ok(ListToolsResult::default()
            .with_tool(
                Tool::new(
                    "ruskel",
                    ToolInputSchema::from_json_schema::<RuskelParams>(),
                )
                .with_description(
                    "Generate a Rust code skeleton for a crate, showing its public API structure",
                ),
            )
            .with_tool(
                Tool::new(
                    "get_type_hint",
                    ToolInputSchema::from_json_schema::<TypeHintParams>(),
                )
                .with_description("Get type information for a symbol at the given cursor position"),
            )
            .with_tool(
                Tool::new(
                    "get_definition",
                    ToolInputSchema::from_json_schema::<GetDefinitionParams>(),
                )
                .with_description(
                    "Get definition location for a symbol at the given cursor position",
                ),
            )
            .with_tool(
                Tool::new(
                    "get_completions",
                    ToolInputSchema::from_json_schema::<GetCompletionsParams>(),
                )
                .with_description("Get completion suggestions at the given cursor position"),
            )
            .with_tool(
                Tool::new(
                    "rename_symbol",
                    ToolInputSchema::from_json_schema::<RenameParams>(),
                )
                .with_description("Rename a symbol across the workspace"),
            ))
    }

    async fn tools_call(
        &mut self,
        name: String,
        arguments: Option<serde_json::Value>,
    ) -> Result<CallToolResult> {
        match name.as_str() {
            "get_type_hint" => {
                let params = match arguments {
                    Some(args) => serde_json::from_value::<TypeHintParams>(args)?,
                    None => return Err(Error::InvalidParams("No arguments provided".to_string())),
                };

                match self
                    .analyzer
                    .lock()
                    .await
                    .get_type_hint(&params.file_path, params.line, params.column)
                    .await
                {
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
            "get_definition" => {
                let params = match arguments {
                    Some(args) => serde_json::from_value::<GetDefinitionParams>(args)?,
                    None => return Err(Error::InvalidParams("No arguments provided".to_string())),
                };

                match self
                    .analyzer
                    .lock()
                    .await
                    .get_definition(&params.file_path, params.line, params.column)
                    .await
                {
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
            "get_completions" => {
                let params = match arguments {
                    Some(args) => serde_json::from_value::<GetCompletionsParams>(args)?,
                    None => return Err(Error::InvalidParams("No arguments provided".to_string())),
                };

                match self
                    .analyzer
                    .lock()
                    .await
                    .get_completions(&params.file_path, params.line, params.column)
                    .await
                {
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
            "ruskel" => {
                let params = match arguments {
                    Some(args) => match serde_json::from_value::<RuskelParams>(args) {
                        Ok(params) => params,
                        Err(e) => {
                            return Ok(CallToolResult::new()
                                .with_text_content(format!("Invalid arguments: {e}"))
                                .is_error(true));
                        }
                    },
                    None => {
                        return Ok(CallToolResult::new()
                            .with_text_content("No arguments provided")
                            .is_error(true));
                    }
                };

                match ruskel::generate_skeleton(
                    &params.target,
                    &params.features,
                    params.all_features,
                    params.no_default_features,
                    params.private,
                )
                .await
                {
                    Ok(skeleton) => Ok(CallToolResult::new()
                        .with_text_content(skeleton)
                        .is_error(false)),
                    Err(e) => Ok(CallToolResult::new()
                        .with_text_content(format!("Error generating skeleton: {e}"))
                        .is_error(true)),
                }
            }
            "rename_symbol" => {
                let params = match arguments {
                    Some(args) => serde_json::from_value::<RenameParams>(args)?,
                    None => return Err(Error::InvalidParams("No arguments provided".to_string())),
                };

                match self
                    .analyzer
                    .lock()
                    .await
                    .rename_symbol(
                        &params.file_path,
                        params.line,
                        params.column,
                        &params.new_name,
                    )
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
            _ => Err(Error::ToolNotFound(name)),
        }
    }
}

pub async fn serve_stdio() -> Result<()> {
    tenx_mcp::Server::default()
        .with_connection_factory(|| Box::new(RustAnalyzerConnection::default()))
        .serve_stdio()
        .await
}

pub async fn serve_tcp(addr: String) -> Result<()> {
    info!("Starting Rust-Analyzer MCP mcp on {}", addr);

    tenx_mcp::Server::default()
        .with_connection_factory(|| Box::new(RustAnalyzerConnection::default()))
        .serve_tcp(addr)
        .await
}
