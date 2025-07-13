use std::path::Path;

use anyhow::Result;
use clap::Parser;
use librustbelt::{builder::RustAnalyzerishBuilder, entities::CursorCoordinates};
use rustyline::{Config, DefaultEditor};

use crate::command::{CommandWrapper, execute_analyzer_command_with_instance};

pub async fn run_repl(workspace_path: &str) -> Result<()> {
    println!("Connecting to workspace: {}", workspace_path);

    // Initialize a standalone analyzer for the workspace
    let mut analyzer = RustAnalyzerishBuilder::from_file(workspace_path)?.build()?;

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
            symbol: None,
        };
        let _ = analyzer.get_type_hint(&dummy_cursor).await; // This will trigger project loading
    }

    println!("Connected to workspace.");
    print_repl_help();

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
                        print_repl_help();
                    }
                    _ => {
                        // Try to parse as an analyzer command using clap
                        match CommandWrapper::try_parse_from(parts) {
                            Ok(wrapper) => {
                                match execute_analyzer_command_with_instance(
                                    wrapper.command,
                                    &mut analyzer,
                                )
                                .await
                                {
                                    Ok(_) => {}
                                    Err(e) => {
                                        println!("Command failed: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                println!("Invalid command: {}", e);
                                println!("Type 'help' for available commands.");
                            }
                        }
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

fn print_repl_help() {
    println!("Available commands:");

    // Generate help text from clap command definitions
    use clap::CommandFactory;
    let app = CommandWrapper::command();

    println!("  {:<20} description", "command");
    // Get the subcommands directly from the CommandWrapper
    for subcommand in app.get_subcommands() {
        // The subcommand here is the "command" field which contains our actual commands
        let name = subcommand.get_name();
        let about = subcommand.get_about().unwrap_or_default();

        // Convert command name from CamelCase to kebab-case for display
        let display_name = name
            .chars()
            .enumerate()
            .map(|(i, c)| {
                if i > 0 && c.is_uppercase() {
                    format!("-{}", c.to_lowercase())
                } else {
                    c.to_lowercase().to_string()
                }
            })
            .collect::<String>();

        println!("  {:<20} {}", display_name, about);
    }

    println!("  {:<20} Show this help message", "help");
    println!("  {:<20} Exit the REPL", "quit/exit");
    println!();
    println!("Note: File paths can be relative to the workspace or absolute");
    println!("      Use --symbol to specify a symbol name when coordinates are ambiguous");
    println!("      Use up/down arrows to navigate command history");
}
