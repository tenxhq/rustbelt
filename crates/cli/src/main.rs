//! rustbelt CLI
//!
//! Command-line interface for rustbelt providing both MCP mcp
//! functionality and standalone CLI tools.

use clap::{Parser, Subcommand};
use librustbelt::{builder::RustAnalyzerishBuilder, entities::CursorCoordinates};
use rustbelt_server::VERSION;

mod repl;

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
    /// Find all references to a symbol at a specific position
    FindReferences {
        /// Path to the Rust source file
        file_path: String,
        /// Line number (1-based)
        line: u32,
        /// Column number (1-based)
        column: u32,
    },
    /// View a Rust file with embedded inlay hints such as types and named arguments
    ViewInlayHints {
        /// Path to the Rust source file
        file_path: String,
    },
    /// Repl to a workspace for interactive queries
    Repl {
        /// Path to the workspace directory
        workspace_path: String,
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
            let mut analyzer = RustAnalyzerishBuilder::from_file(&file_path)?.build()?;

            let cursor = CursorCoordinates {
                file_path: file_path.clone(),
                line,
                column,
                symbol: None,
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
            let mut analyzer = RustAnalyzerishBuilder::from_file(&file_path)?.build()?;

            let cursor = CursorCoordinates {
                file_path: file_path.clone(),
                line,
                column,
                symbol: None,
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
            let mut analyzer = RustAnalyzerishBuilder::from_file(&file_path)?.build()?;

            let cursor = CursorCoordinates {
                file_path: file_path.clone(),
                line,
                column,
                symbol: None,
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
        Commands::FindReferences {
            file_path,
            line,
            column,
        } => {
            // Initialize logging for debugging
            tracing_subscriber::fmt::init();

            // Initialize a standalone analyzer for CLI usage
            let mut analyzer = RustAnalyzerishBuilder::from_file(&file_path)?.build()?;

            let cursor = CursorCoordinates {
                file_path: file_path.clone(),
                line,
                column,
                symbol: None,
            };

            match analyzer.find_references(&cursor).await {
                Ok(Some(references)) => {
                    println!(
                        "Found {} reference(s) to symbol at {}:{}:{}:",
                        references.len(),
                        file_path,
                        line,
                        column
                    );
                    for reference in references {
                        println!("  {}", reference);
                    }
                }
                Ok(None) => {
                    eprintln!("No references found at {file_path}:{line}:{column}");
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Error finding references: {e}");
                    std::process::exit(1);
                }
            }
        }
        Commands::ViewInlayHints { file_path } => {
            // Initialize logging for debugging
            tracing_subscriber::fmt::init();

            // Initialize a standalone analyzer for CLI usage
            let mut analyzer = RustAnalyzerishBuilder::from_file(&file_path)?.build()?;

            match analyzer.view_inlay_hints(&file_path, None, None).await {
                Ok(annotated_content) => {
                    println!("{}", annotated_content);
                }
                Err(e) => {
                    eprintln!("Error viewing inlay hints: {e}");
                    std::process::exit(1);
                }
            }
        }
        Commands::Repl { workspace_path } => {
            // Initialize logging for debugging
            tracing_subscriber::fmt::init();

            repl::run_repl(&workspace_path).await?;
        }
    }

    Ok(())
}
