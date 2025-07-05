use anyhow::Result;
use librustbelt::{RustAnalyzerish, entities::CursorCoordinates};
use rustyline::{Config, DefaultEditor};
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

    // Configure rustyline with history support
    let config = Config::builder()
        .history_ignore_space(true)
        .completion_type(rustyline::CompletionType::List)
        .build();

    let mut rl = DefaultEditor::with_config(config)?;

    // Load history from file if it exists
    let history_file = format!(
        "{}/.rustbelt_history",
        std::env::var("HOME").unwrap_or_else(|_| ".".to_string())
    );
    let _ = rl.load_history(&history_file); // Ignore errors if file doesn't exist

    // Start loading the project immediately in the background
    println!("Loading project... This may take a moment on first run.");

    // Prime the analyzer by accessing a dummy file in the workspace to trigger project loading
    let dummy_file = Path::new(workspace_path).join("Cargo.toml");
    if dummy_file.exists() {
        let dummy_cursor = CursorCoordinates {
            file_path: dummy_file.to_string_lossy().to_string(),
            line: 1,
            column: 1,
        };
        let _ = analyzer.get_type_hint(&dummy_cursor).await; // This will trigger project loading
    }

    println!("Connected to workspace. Available commands:");
    println!("  help          - Show this help message");
    println!("  type <file> <line> <col> - Get type hint at position");
    println!("  def <file> <line> <col>  - Get definition at position");
    println!("  comp <file> <line> <col> - Get completions at position");
    println!("  refs <file> <line> <col> - Find references to symbol");
    println!("  hints <file>  - View file with inlay hints");
    println!("  rename <file> <line> <col> <new_name> - Rename symbol");
    println!("  quit/exit     - Exit the REPL");
    println!("  Note: File paths can be relative to the workspace or absolute");
    println!("  Use up/down arrows to navigate command history");
    println!();

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
                        let _ = rl.save_history(&history_file); // Save history on exit
                        break;
                    }
                    "help" => {
                        println!("Available commands:");
                        println!("  help          - Show this help message");
                        println!("  type <file> <line> <col> - Get type hint at position");
                        println!("  def <file> <line> <col>  - Get definition at position");
                        println!("  comp <file> <line> <col> - Get completions at position");
                        println!("  refs <file> <line> <col> - Find references to symbol");
                        println!("  hints <file>  - View file with inlay hints");
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
                    "refs" => {
                        if parts.len() != 4 {
                            println!("Usage: refs <file> <line> <col>");
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

                        match analyzer.find_references(&cursor).await {
                            Ok(Some(references)) => {
                                println!("Found {} reference(s):", references.len());
                                for reference in references {
                                    println!("  {}", reference);
                                }
                            }
                            Ok(None) => {
                                println!(
                                    "No references found at {}:{}:{}",
                                    file_path, line, column
                                );
                            }
                            Err(e) => {
                                println!("Error finding references: {}", e);
                            }
                        }
                    }
                    "hints" => {
                        if parts.len() != 2 {
                            println!("Usage: hints <file>");
                            continue;
                        }

                        let file_path = resolve_path(workspace_path, parts[1]);

                        match analyzer.view_inlay_hints(&file_path, None, None).await {
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
                let _ = rl.save_history(&history_file); // Save history on exit
                break;
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                println!("CTRL-D");
                let _ = rl.save_history(&history_file); // Save history on exit
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
