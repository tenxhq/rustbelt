//! rustbelt CLI
//!
//! Command-line interface for rustbelt providing both MCP mcp
//! functionality and standalone CLI tools.

use clap::{Parser, Subcommand};
use librustbelt::{RustAnalyzerish, entities::CursorCoordinates};
use rustbelt_server::VERSION;

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
    /// Start the MCP server (defaults to stdio mode)
    Serve {
        /// Use TCP mode instead of default stdio mode
        #[arg(long)]
        tcp: bool,
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
    /// Get completion suggestions at a specific position
    GetCompletions {
        /// Path to the Rust source file
        file_path: String,
        /// Line number (1-based)
        line: u32,
        /// Column number (1-based)
        column: u32,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { tcp, host, port } => {
            // Only initialize logging for TCP mode
            // In stdio mode, logging would interfere with JSON-RPC communication
            if tcp {
                tracing_subscriber::fmt::init();
            }

            if tcp {
                // Run in TCP mode for debugging
                let addr = format!("{host}:{port}");
                rustbelt_server::serve_tcp(addr).await?;
            } else {
                // Run in stdio mode - recommended for MCP clients (default)
                rustbelt_server::serve_stdio().await?;
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

            let cursor = CursorCoordinates {
                file_path: file_path.clone(),
                line,
                column,
            };

            match analyzer.get_type_hint(&cursor).await {
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

            let cursor = CursorCoordinates {
                file_path: file_path.clone(),
                line,
                column,
            };

            match analyzer.get_definition(&cursor).await {
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
        Commands::GetCompletions {
            file_path,
            line,
            column,
        } => {
            // Initialize logging for debugging
            tracing_subscriber::fmt::init();

            // Initialize a standalone analyzer for CLI usage
            let mut analyzer = RustAnalyzerish::new();

            let cursor = CursorCoordinates {
                file_path: file_path.clone(),
                line,
                column,
            };

            match analyzer.get_completions(&cursor).await {
                Ok(Some(completions)) => {
                    println!(
                        "Available completions at {}:{}:{} ({} items):",
                        file_path,
                        line,
                        column,
                        completions.len()
                    );
                    for completion in completions {
                        println!("  {}", completion);
                    }
                }
                Ok(None) => {
                    eprintln!("No completions found at {file_path}:{line}:{column}");
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Error getting completions: {e}");
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}
