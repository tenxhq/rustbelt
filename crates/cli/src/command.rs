use anyhow::Result;
use clap::{Parser, Subcommand};
use librustbelt::{
    analyzer::RustAnalyzerish, builder::RustAnalyzerishBuilder, entities::CursorCoordinates,
};

// Unified command wrapper for both CLI and REPL use
#[derive(Parser)]
#[command(no_binary_name = true)]
pub struct CommandWrapper {
    #[command(subcommand)]
    pub command: AnalyzerCommand,
}

// Base commands without workspace path - used by both CLI and REPL
#[derive(Subcommand)]
#[command(no_binary_name = true)]
pub enum AnalyzerCommand {
    /// Get type hint for a specific position
    TypeHint {
        /// Path to the Rust source file
        file_path: String,
        /// Line number (1-based)
        line: u32,
        /// Column number (1-based)
        column: u32,
        /// Optional symbol name to search for near the coordinates
        #[arg(long)]
        symbol: Option<String>,
    },

    /// Get definition details for a symbol at a specific position
    GetDefinition {
        /// Path to the Rust source file
        file_path: String,
        /// Line number (1-based)
        line: u32,
        /// Column number (1-based)
        column: u32,
        /// Optional symbol name to search for near the coordinates
        #[arg(long)]
        symbol: Option<String>,
    },

    /// Get completion suggestions at a specific position
    GetCompletions {
        /// Path to the Rust source file
        file_path: String,
        /// Line number (1-based)
        line: u32,
        /// Column number (1-based)
        column: u32,
        /// Optional symbol name to search for near the coordinates
        #[arg(long)]
        symbol: Option<String>,
    },

    /// Find all references to a symbol at a specific position
    FindReferences {
        /// Path to the Rust source file
        file_path: String,
        /// Line number (1-based)
        line: u32,
        /// Column number (1-based)
        column: u32,
        /// Optional symbol name to search for near the coordinates
        #[arg(long)]
        symbol: Option<String>,
    },

    /// View a Rust file with embedded inlay hints such as types and named arguments
    ViewInlayHints {
        /// Path to the Rust source file
        file_path: String,
        /// Starting line number (1-based, optional)
        #[arg(long)]
        start_line: Option<u32>,
        /// Ending line number (1-based, optional)
        #[arg(long)]
        end_line: Option<u32>,
    },

    /// Get available code assists (code actions) at a specific position
    GetAssists {
        /// Path to the Rust source file
        file_path: String,
        /// Line number (1-based)
        line: u32,
        /// Column number (1-based)
        column: u32,
        /// Optional symbol name to search for near the coordinates
        #[arg(long)]
        symbol: Option<String>,
    },

    /// Apply a specific code assist at a position
    ApplyAssist {
        /// Path to the Rust source file
        file_path: String,
        /// Line number (1-based)
        line: u32,
        /// Column number (1-based)
        column: u32,
        /// ID of the assist to apply
        assist_id: String,
        /// Optional symbol name to search for near the coordinates
        #[arg(long)]
        symbol: Option<String>,
    },

    /// Search for symbols across the entire workspace
    GetWorkspaceSymbols {
        /// Path to *any* file inside the target workspace (used to locate `Cargo.toml`)
        file_path: String,
        /// Case-insensitive query string to search for
        query: String,
    },

    /// Rename a symbol at a specific position
    RenameSymbol {
        /// Path to the Rust source file
        file_path: String,
        /// Line number (1-based)
        line: u32,
        /// Column number (1-based)
        column: u32,
        /// New name for the symbol
        new_name: String,
        /// Optional symbol name to search for near the coordinates
        #[arg(long)]
        symbol: Option<String>,
    },
}

// For REPL use - reuses existing analyzer connection
pub async fn execute_analyzer_command_with_instance(
    command: AnalyzerCommand,
    analyzer: &mut RustAnalyzerish,
) -> Result<()> {
    match command {
        AnalyzerCommand::TypeHint {
            file_path,
            line,
            column,
            symbol,
        } => {
            let cursor = CursorCoordinates {
                file_path: file_path.clone(),
                line,
                column,
                symbol,
            };

            match analyzer.get_type_hint(&cursor).await {
                Ok(Some(type_info)) => {
                    println!("Type Hint:\n-----\n{}\n------", type_info);
                }
                Ok(None) => {
                    println!(
                        "No type information available at {}:{}:{}",
                        file_path, line, column
                    );
                }
                Err(e) => {
                    println!("Error getting type hint: {}", e);
                }
            }
        }
        AnalyzerCommand::GetDefinition {
            file_path,
            line,
            column,
            symbol,
        } => {
            let cursor = CursorCoordinates {
                file_path: file_path.clone(),
                line,
                column,
                symbol,
            };

            match analyzer.get_definition(&cursor).await {
                Ok(Some(definitions)) => {
                    println!("Found {} definition(s):", definitions.len());
                    for def in definitions {
                        println!("  {}", def);
                    }
                }
                Ok(None) => {
                    println!("No definitions found at {}:{}:{}", file_path, line, column);
                }
                Err(e) => {
                    println!("Error getting definitions: {}", e);
                }
            }
        }
        AnalyzerCommand::GetCompletions {
            file_path,
            line,
            column,
            symbol,
        } => {
            let cursor = CursorCoordinates {
                file_path: file_path.clone(),
                line,
                column,
                symbol,
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
                    println!("No completions found at {}:{}:{}", file_path, line, column);
                }
                Err(e) => {
                    println!("Error getting completions: {}", e);
                }
            }
        }
        AnalyzerCommand::FindReferences {
            file_path,
            line,
            column,
            symbol,
        } => {
            let cursor = CursorCoordinates {
                file_path: file_path.clone(),
                line,
                column,
                symbol,
            };

            match analyzer.find_references(&cursor).await {
                Ok(Some(references)) => {
                    println!("Found {} reference(s):", references.len());
                    for reference in references {
                        println!("  {}", reference);
                    }
                }
                Ok(None) => {
                    println!("No references found at {}:{}:{}", file_path, line, column);
                }
                Err(e) => {
                    println!("Error finding references: {}", e);
                }
            }
        }
        AnalyzerCommand::ViewInlayHints {
            file_path,
            start_line,
            end_line,
        } => {
            match analyzer
                .view_inlay_hints(&file_path, start_line, end_line)
                .await
            {
                Ok(annotated_content) => {
                    println!("File with inlay hints:");
                    println!("=====================================");
                    println!("{}", annotated_content);
                    println!("=====================================");
                }
                Err(e) => {
                    println!("Error viewing inlay hints: {}", e);
                }
            }
        }
        AnalyzerCommand::GetAssists {
            file_path,
            line,
            column,
            symbol,
        } => {
            let cursor = CursorCoordinates {
                file_path: file_path.clone(),
                line,
                column,
                symbol,
            };

            match analyzer.get_assists(&cursor).await {
                Ok(Some(assists)) => {
                    println!(
                        "Available assists at {}:{}:{} ({} items):",
                        file_path,
                        line,
                        column,
                        assists.len()
                    );
                    for assist in assists {
                        println!("  {} ({}): {}", assist.label, assist.id, assist.target);
                    }
                }
                Ok(None) => {
                    println!("No assists available at {}:{}:{}", file_path, line, column);
                }
                Err(e) => {
                    println!("Error getting assists: {}", e);
                }
            }
        }
        AnalyzerCommand::ApplyAssist {
            file_path,
            line,
            column,
            assist_id,
            symbol,
        } => {
            let cursor = CursorCoordinates {
                file_path: file_path.clone(),
                line,
                column,
                symbol,
            };

            match analyzer.apply_assist(&cursor, &assist_id).await {
                Ok(Some(source_change)) => {
                    println!("Successfully applied assist '{}':", assist_id);
                    for file_change in &source_change.file_changes {
                        println!("  Modified file: {}", file_change.file_path);
                        println!("    {} edits applied", file_change.edits.len());
                    }
                }
                Ok(None) => {
                    println!(
                        "Assist '{}' not available at {}:{}:{}",
                        assist_id, file_path, line, column
                    );
                }
                Err(e) => {
                    println!("Error applying assist '{}': {}", assist_id, e);
                }
            }
        }
        AnalyzerCommand::RenameSymbol {
            file_path,
            line,
            column,
            new_name,
            symbol,
        } => {
            let cursor = CursorCoordinates {
                file_path: file_path.clone(),
                line,
                column,
                symbol,
            };

            match analyzer.rename_symbol(&cursor, &new_name).await {
                Ok(Some(changes)) => {
                    println!(
                        "Rename successful! {} file(s) changed:",
                        changes.file_changes.len()
                    );
                    for change in &changes.file_changes {
                        println!("  {}: {} edit(s)", change.file_path, change.edits.len());
                    }
                }
                Ok(None) => {
                    println!(
                        "No symbol found to rename at {}:{}:{}",
                        file_path, line, column
                    );
                }
                Err(e) => {
                    println!("Error renaming symbol: {}", e);
                }
            }
        }
        AnalyzerCommand::GetWorkspaceSymbols { file_path, query } => {
            match analyzer.get_workspace_symbols(&query).await {
                Ok(Some(symbols)) => {
                    println!(
                        "Found {} symbol(s) matching '{}':",
                        symbols.len(),
                        query
                    );
                    for sym in symbols {
                        println!("  {}", sym);
                    }
                }
                Ok(None) => {
                    println!("No symbols found matching '{}' in workspace", query);
                }
                Err(e) => {
                    println!("Error searching workspace symbols: {}", e);
                }
            }
        }
    }
    Ok(())
}

// For CLI use - creates new analyzer instance for single command
pub(crate) async fn execute_analyzer_command(
    command: AnalyzerCommand,
    workspace_path: &str,
) -> Result<()> {
    let mut analyzer = RustAnalyzerishBuilder::from_file(workspace_path)?.build()?;
    execute_analyzer_command_with_instance(command, &mut analyzer).await
}

pub(crate) fn extract_workspace_path(command: &AnalyzerCommand) -> String {
    match command {
        AnalyzerCommand::TypeHint { file_path, .. }
        | AnalyzerCommand::GetDefinition { file_path, .. }
        | AnalyzerCommand::GetCompletions { file_path, .. }
        | AnalyzerCommand::FindReferences { file_path, .. }
        | AnalyzerCommand::ViewInlayHints { file_path, .. }
        | AnalyzerCommand::GetAssists { file_path, .. }
        | AnalyzerCommand::ApplyAssist { file_path, .. }
        | AnalyzerCommand::RenameSymbol { file_path, .. } => file_path.clone(),
        AnalyzerCommand::GetWorkspaceSymbols { file_path, .. } => file_path.clone(),
    }
}
