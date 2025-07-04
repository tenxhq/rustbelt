use anyhow::Result;
use librustbelt::{RustAnalyzerish, entities::CursorCoordinates};
use rustyline::DefaultEditor;
use std::path::Path;

fn resolve_path(workspace_path: &str, file_path: &str) -> String {
    if Path::new(file_path).is_absolute() {
        file_path.to_string()
    } else {
        Path::new(workspace_path)
            .join(file_path)
            .to_string_lossy()
            .to_string()
    }
}

pub async fn run_repl(workspace_path: &str) -> Result<()> {
    println!("Connecting to workspace: {}", workspace_path);

    // Initialize a standalone analyzer for the workspace
    let mut analyzer = RustAnalyzerish::new();

    // TODO: We might want to add a method to explicitly set the workspace root
    // For now, we'll rely on the analyzer to detect the workspace from file operations

    println!("Connected to workspace. Available commands:");
    println!("  help          - Show this help message");
    println!("  type <file> <line> <col> - Get type hint at position");
    println!("  def <file> <line> <col>  - Get definition at position");
    println!("  comp <file> <line> <col> - Get completions at position");
    println!("  rename <file> <line> <col> <new_name> - Rename symbol");
    println!("  quit/exit     - Exit the REPL");
    println!("  Note: File paths can be relative to the workspace or absolute");
    println!();

    let mut rl = DefaultEditor::new()?;

    loop {
        let readline = rl.readline("rustbelt> ");
        match readline {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                rl.add_history_entry(line)?;

                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.is_empty() {
                    continue;
                }

                match parts[0] {
                    "quit" | "exit" => {
                        println!("Goodbye!");
                        break;
                    }
                    "help" => {
                        println!("Available commands:");
                        println!("  help          - Show this help message");
                        println!("  type <file> <line> <col> - Get type hint at position");
                        println!("  def <file> <line> <col>  - Get definition at position");
                        println!("  comp <file> <line> <col> - Get completions at position");
                        println!("  rename <file> <line> <col> <new_name> - Rename symbol");
                        println!("  quit/exit     - Exit the REPL");
                    }
                    "type" => {
                        if parts.len() != 4 {
                            println!("Usage: type <file> <line> <col>");
                            continue;
                        }

                        let file_path = resolve_path(workspace_path, parts[1]);
                        let line: u32 = match parts[2].parse() {
                            Ok(l) => l,
                            Err(_) => {
                                println!("Invalid line number: {}", parts[2]);
                                continue;
                            }
                        };
                        let column: u32 = match parts[3].parse() {
                            Ok(c) => c,
                            Err(_) => {
                                println!("Invalid column number: {}", parts[3]);
                                continue;
                            }
                        };

                        let cursor = CursorCoordinates {
                            file_path: file_path.clone(),
                            line,
                            column,
                        };

                        match analyzer.get_type_hint(&cursor).await {
                            Ok(Some(type_info)) => {
                                println!("Type: {}", type_info);
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
                    "def" => {
                        if parts.len() != 4 {
                            println!("Usage: def <file> <line> <col>");
                            continue;
                        }

                        let file_path = resolve_path(workspace_path, parts[1]);
                        let line: u32 = match parts[2].parse() {
                            Ok(l) => l,
                            Err(_) => {
                                println!("Invalid line number: {}", parts[2]);
                                continue;
                            }
                        };
                        let column: u32 = match parts[3].parse() {
                            Ok(c) => c,
                            Err(_) => {
                                println!("Invalid column number: {}", parts[3]);
                                continue;
                            }
                        };

                        let cursor = CursorCoordinates {
                            file_path: file_path.clone(),
                            line,
                            column,
                        };

                        match analyzer.get_definition(&cursor).await {
                            Ok(Some(definitions)) => {
                                println!("Found {} definition(s):", definitions.len());
                                for def in definitions {
                                    println!("  {}", def);
                                }
                            }
                            Ok(None) => {
                                println!(
                                    "No definitions found at {}:{}:{}",
                                    file_path, line, column
                                );
                            }
                            Err(e) => {
                                println!("Error getting definitions: {}", e);
                            }
                        }
                    }
                    "comp" => {
                        if parts.len() != 4 {
                            println!("Usage: comp <file> <line> <col>");
                            continue;
                        }

                        let file_path = resolve_path(workspace_path, parts[1]);
                        let line: u32 = match parts[2].parse() {
                            Ok(l) => l,
                            Err(_) => {
                                println!("Invalid line number: {}", parts[2]);
                                continue;
                            }
                        };
                        let column: u32 = match parts[3].parse() {
                            Ok(c) => c,
                            Err(_) => {
                                println!("Invalid column number: {}", parts[3]);
                                continue;
                            }
                        };

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
                                println!(
                                    "No completions found at {}:{}:{}",
                                    file_path, line, column
                                );
                            }
                            Err(e) => {
                                println!("Error getting completions: {}", e);
                            }
                        }
                    }
                    "rename" => {
                        if parts.len() != 5 {
                            println!("Usage: rename <file> <line> <col> <new_name>");
                            continue;
                        }

                        let file_path = resolve_path(workspace_path, parts[1]);
                        let line: u32 = match parts[2].parse() {
                            Ok(l) => l,
                            Err(_) => {
                                println!("Invalid line number: {}", parts[2]);
                                continue;
                            }
                        };
                        let column: u32 = match parts[3].parse() {
                            Ok(c) => c,
                            Err(_) => {
                                println!("Invalid column number: {}", parts[3]);
                                continue;
                            }
                        };
                        let new_name = parts[4].to_string();

                        let cursor = CursorCoordinates {
                            file_path: file_path.clone(),
                            line,
                            column,
                        };

                        match analyzer.rename_symbol(&cursor, &new_name).await {
                            Ok(Some(changes)) => {
                                println!(
                                    "Rename successful! {} file(s) changed:",
                                    changes.file_changes.len()
                                );
                                for change in &changes.file_changes {
                                    println!(
                                        "  {}: {} edit(s)",
                                        change.file_path,
                                        change.edits.len()
                                    );
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
                    _ => {
                        println!(
                            "Unknown command: {}. Type 'help' for available commands.",
                            parts[0]
                        );
                    }
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }

    Ok(())
}
