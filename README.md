# tenx-rustkit

A set of Rust specific tools to provide enhanced tools via the MCP protocol. These tools provide IDE functionality like type hints, go-to-definition, and semantic analysis.

## Overview

tenx-rustkit bridges rust-analyzer's powerful IDE capabilities with the Model Context Protocol, allowing AI assistants and other MCP clients to access Rust language intelligence. The server analyzes Rust projects and provides semantic information about code symbols, types, and structure.

## Usage

### MCP Server Mode (Recommended)

Start the server in stdio mode for MCP clients:

```bash
tenx-rustkit serve --stdio
```

Or start in TCP mode for debugging:

```bash
tenx-rustkit serve --host 127.0.0.1 --port 3001
```

### CLI Mode

Get type information directly from the command line:

```bash
tenx-rustkit type-hint /path/to/file.rs 10 15
```

## Available Tools

| Tool Name | Status | Description | Parameters |
|-----------|--------|-------------|------------|
| `get_type_hint` | Implemented | Get type information for a symbol at cursor position | `file_path` (string), `line` (number 1-indexed), `column` (number 1-indexed) |


## Planned Improvements

### General functionality

- [ ] Cache rust-analyzer analysis results to speed up subsequent queries on the same project
- [ ] Only reload changed files instead of entire project on updates
- [ ] Implement LRU cache for loaded projects to manage memory usage
- [ ] Handle multiple Rust projects simultaneously
- [ ] Honor rust-analyzer and project-specific configurations


### Additional tools

| Tool Name | Status | Description | Parameters |
|-----------|--------|-------------|------------|
| `goto_definition` | Planned | Navigate to symbol definition | `file_path`, `line`, `column` |
| `rename_symbol` | Planned | Rename a symbol across the workspace | `file_path`, `line`, `column`, `new_name` |
| `find_references` | Planned | Find all references to a symbol | `file_path`, `line`, `column` |
| `get_completions` | Planned | Get code completion suggestions | `file_path`, `line`, `column`, `trigger_character?` |
| `get_signature_help` | Planned | Get function signature information | `file_path`, `line`, `column` |
| `get_document_symbols` | Planned | Get all symbols in a document | `file_path` |
| `get_workspace_symbols` | Planned | Search for symbols across workspace | `query` |
| `format_document` | Planned | Format a Rust document | `file_path` |
| `get_diagnostics` | Planned | Get compiler errors and warnings | `file_path` |

## Requirements

- Rust 1.80+ nightly (uses Rust 2024 edition)
- A Rust project with `Cargo.toml` for analysis

## Community

Want to contribute? Have ideas or feature requests? Come tell us about it on
[Discord](https://discord.gg/fHmRmuBDxF).


## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Related Projects

- Built on top of [rust-analyzer](https://github.com/rust-lang/rust-analyzer)
- Powered by [tenx-mcp](https://github.com/tenxhq/tenx-mcp)
