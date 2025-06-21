//! Rust-Analyzer MCP Server
//!
//! This server provides rust-analyzer functionality via the Model Context
//! Protocol (MCP). It exposes IDE capabilities like type hints,
//! go-to-definition, and more as MCP tools.

use std::sync::Arc;

use async_trait::async_trait;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use tenx_mcp::{Result, Server, connection::Connection, error::Error, schema::*, schemars};
use tokio::sync::Mutex;
use tracing::info;

pub mod analyzer;
pub mod ruskel;
use analyzer::RustAnalyzerish;
use ruskel::ruskel_analyzer::RuskelAnalyzer;
use crate::analyzer::{DefinitionInfo, FileChange, RenameResult, TextEdit, TypeHint};

const NAME: &str = "rustbelt";

pub const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "-",
    env!("VERGEN_GIT_SHA"),
    " (",
    env!("VERGEN_BUILD_DATE"),
    ")"
);


#[derive(Parser)]
#[command(name = "rustbelt")]
#[command(about = "rustbelt MCP Server - power up your Rust development")]
#[command(version = VERSION)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the MCP server
    Serve {
        /// Use stdio mode (recommended for MCP clients)
        #[arg(long)]
        stdio: bool,
        /// Host for TCP mode
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Port for TCP mode
        #[arg(long, default_value = "3001")]
        port: u16,
    },
    /// Get type hint for a specific position
    TypeHint {
        /// Path to the Rust source file
        file_path: String,
        /// Line number (1-based)
        line: u32,
        /// Column number (1-based)
        column: u32,
    },
    /// Get definition details for a symbol at a specific position
    GetDefinition {
        /// Path to the Rust source file
        file_path: String,
        /// Line number (1-based)
        line: u32,
        /// Column number (1-based)
        column: u32,
    },
}

/// Parameters for the get_type_hint tool
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
struct TypeHintParams {
    /// Absolute path to the Rust source file
    file_path: String,
    /// Line number (1-based)
    line: u32,
    /// Column number (1-based)
    column: u32,
}

/// Parameters for the get_definition tool
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
struct GetDefinitionParams {
    /// Absolute path to the Rust source file
    file_path: String,
    /// Line number (1-based)
    line: u32,
    /// Column number (1-based)
    column: u32,
}

/// Parameters for the rename_symbol tool
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
struct RenameParams {
    /// Absolute path to the Rust source file
    file_path: String,
    /// Line number (1-based)
    line: u32,
    /// Column number (1-based)
    column: u32,
    /// New name for the symbol
    new_name: String,
}

/// Parameters for the ruskel tool
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
struct RuskelParams {
    /// Target specification (crate path, published crate name, or module path)
    target: String,
    /// Optional specific features to enable
    #[serde(default)]
    features: Vec<String>,
    /// Enable all features
    #[serde(default)]
    all_features: bool,
    /// Disable default features
    #[serde(default)]
    no_default_features: bool,
    /// Include private items in the skeleton
    #[serde(default)]
    private: bool,
}

/// Rust-Analyzer MCP server connection
#[derive(Debug, Clone)]
struct RustAnalyzerConnection {
    analyzer: Arc<Mutex<RustAnalyzerish>>,
    ruskel_analyzer: Arc<Mutex<RuskelAnalyzer>>,
}

impl Default for RustAnalyzerConnection {
    fn default() -> Self {
        Self {
            analyzer: Arc::new(Mutex::new(RustAnalyzerish::new())),
            ruskel_analyzer: Arc::new(Mutex::new(RuskelAnalyzer::new())),
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

                match self
                    .ruskel_analyzer
                    .lock()
                    .await
                    .generate_skeleton(
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { stdio, host, port } => {
            // Only initialize logging for TCP mode
            // In stdio mode, logging would interfere with JSON-RPC communication
            if !stdio {
                tracing_subscriber::fmt::init();
            }

            if stdio {
                // Run in stdio mode - recommended for MCP clients
                Server::default()
                    .with_connection_factory(|| Box::new(RustAnalyzerConnection::default()))
                    .serve_stdio()
                    .await?;
            } else {
                // Run in TCP mode for debugging
                let addr = format!("{host}:{port}");
                info!("Starting Rust-Analyzer MCP server on {}", addr);

                Server::default()
                    .with_connection_factory(|| Box::new(RustAnalyzerConnection::default()))
                    .serve_tcp(addr)
                    .await?;
            }
        }
        Commands::TypeHint {
            file_path,
            line,
            column,
        } => {
            // Initialize logging for debugging
            tracing_subscriber::fmt::init();

            // Initialize a standalone analyzer for CLI usage
            let mut analyzer = RustAnalyzerish::new();

            match analyzer.get_type_hint(&file_path, line, column).await {
                Ok(Some(type_info)) => {
                    println!("The type information is:\n{type_info}");
                }
                Ok(None) => {
                    eprintln!("No type information available at {file_path}:{line}:{column}");
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Error getting type hint: {e}");
                    std::process::exit(1);
                }
            }
        }
        Commands::GetDefinition {
            file_path,
            line,
            column,
        } => {
            // Initialize logging for debugging
            tracing_subscriber::fmt::init();

            // Initialize a standalone analyzer for CLI usage
            let mut analyzer = RustAnalyzerish::new();

            match analyzer.get_definition(&file_path, line, column).await {
                Ok(Some(definitions)) => {
                    println!("Found {} definition(s):", definitions.len());
                    for def in definitions {
                        println!("  {def}");
                    }
                }
                Ok(None) => {
                    eprintln!("No definitions found at {file_path}:{line}:{column}");
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Error getting definitions: {e}");
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}

impl std::fmt::Display for TypeHint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.symbol, self.short_type)
    }
}

impl std::fmt::Display for DefinitionInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{} - {} ({:?})",
            self.file_path, self.line, self.column, self.name, self.kind
        )
    }
}

impl std::fmt::Display for RenameResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "Successfully renamed symbol in {} file(s):",
            self.file_changes.len()
        )?;
        writeln!(f)?;
        for file_change in &self.file_changes {
            writeln!(f, "{file_change}")?;
        }
        Ok(())
    }
}

impl std::fmt::Display for FileChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.file_path)?;
        for edit in &self.edits {
            writeln!(f, "  ↳ {edit}")?;
        }
        Ok(())
    }
}

impl std::fmt::Display for TextEdit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}-{}:{} → '{}'",
            self.line, self.column, self.end_line, self.end_column, self.new_text
        )
    }
}
