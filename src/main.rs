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
use analyzer::RustAnalyzer;

const NAME: &str = "tenx-lsp";
const VERSION: &str = "0.0.1";

#[derive(Parser)]
#[command(name = "tenx-lsp")]
#[command(about = "Rust-Analyzer MCP Server")]
#[command(version)]
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

/// Rust-Analyzer MCP server connection
#[derive(Debug, Clone)]
struct RustAnalyzerConnection {
    analyzer: Arc<Mutex<RustAnalyzer>>,
}

impl Default for RustAnalyzerConnection {
    fn default() -> Self {
        Self {
            analyzer: Arc::new(Mutex::new(RustAnalyzer::new())),
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
        Ok(ListToolsResult::default().with_tool(
            Tool::new(
                "get_type_hint",
                ToolInputSchema::from_json_schema::<TypeHintParams>(),
            )
            .with_description("Get type information for a symbol at the given cursor position"),
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
                        .with_text_content(type_info)
                        .is_error(false)),
                    Ok(None) => Ok(CallToolResult::new()
                        .with_text_content("No type information available at this position")
                        .is_error(false)),
                    Err(e) => Ok(CallToolResult::new()
                        .with_text_content(format!("Error getting type hint: {e}"))
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
            let mut analyzer = RustAnalyzer::new();

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
    }

    Ok(())
}
