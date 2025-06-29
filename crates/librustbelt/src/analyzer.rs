//! Rust-Analyzer Integration Module
//!
//! This module provides a wrapper around rust-analyzer's IDE functionality,
//! making it easy to get type hints, definitions, and other semantic
//! information.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use bimap::BiMap;
use ra_ap_base_db::SourceRoot;
use ra_ap_ide::{
    AnalysisHost, CallableSnippets, CompletionConfig, CompletionFieldsToResolve,
    CompletionItemKind as RaCompletionItemKind, FileId, FilePosition, FileRange, HoverConfig,
    HoverDocFormat, LineCol, SubstTyLen, TextRange, TextSize,
};
use ra_ap_ide_db::{
    ChangeWithProcMacros, FxHashMap, SymbolKind,
    imports::insert_use::{ImportGranularity, InsertUseConfig, PrefixKind},
};
use ra_ap_project_model::{CargoConfig, ProjectManifest, ProjectWorkspace, RustLibSource};
use ra_ap_vfs::{
    AbsPathBuf, Vfs, VfsPath,
    loader::{Handle, LoadingProgress},
};
use ra_ap_vfs_notify as vfs_notify;
use tracing::{debug, error, info, warn};

/// Information about a definition location
#[derive(Debug, Clone)]
pub struct DefinitionInfo {
    /// Path to the file containing the definition
    pub file_path: String,
    /// Line number (1-based) where the definition starts
    pub line: u32,
    /// Column number (1-based) where the definition starts
    pub column: u32,
    /// Line number (1-based) where the definition ends
    pub end_line: u32,
    /// Column number (1-based) where the definition ends
    pub end_column: u32,
    /// Name of the defined symbol
    pub name: String,
    /// Kind of the symbol (function, struct, etc.)
    pub kind: Option<SymbolKind>,
    /// Content of the definition
    pub content: String,
    /// Canonical module path
    pub module: String,
    /// Rustdoc description, if available
    pub description: Option<String>,
}

/// Information about a rename operation result
#[derive(Debug, Clone)]
pub struct RenameResult {
    /// Files that will be changed by the rename operation
    pub file_changes: Vec<FileChange>,
}

/// Information about changes to a single file during rename
#[derive(Debug, Clone)]
pub struct FileChange {
    /// Path to the file that will be changed
    pub file_path: String,
    /// List of text edits to apply to this file
    pub edits: Vec<TextEdit>,
}

/// A single text edit within a file
#[derive(Debug, Clone)]
pub struct TextEdit {
    /// Line number (1-based) where the edit starts
    pub line: u32,
    /// Column number (1-based) where the edit starts
    pub column: u32,
    /// Line number (1-based) where the edit ends
    pub end_line: u32,
    /// Column number (1-based) where the edit ends
    pub end_column: u32,
    /// The text to replace the range with
    pub new_text: String,
}

/// A type hint for a given symbol
#[derive(Debug, Clone)]
pub struct TypeHint {
    pub file_path: String,
    /// Line number (1-based) where the edit starts
    pub line: u32,
    /// Column number (1-based) where the edit starts
    pub column: u32,
    pub symbol: String,
    pub short_type: String,
    pub canonical_type: String,
}

/// A completion item for a given cursor position
#[derive(Debug, Clone)]
pub struct CompletionItem {
    /// The primary name/identifier
    pub name: String,
    /// Alternative names (aliases)
    // pub aliases: Vec<String>,
    /// Required import
    pub required_import: Option<String>,
    /// The trait this method comes from (for trait methods)
    // pub trait_source: Option<String>,
    /// The kind of completion (function, variable, etc.)
    pub kind: Option<String>,
    /// The text to insert when this completion is selected
    // pub insert_text: String,
    /// Function signature or type information
    pub signature: Option<String>,
    /// Documentation for this completion
    pub documentation: Option<String>,
    /// Whether this completion is deprecated
    pub deprecated: bool,
}

/// Main interface to rust-analyzer functionality
#[derive(Debug)]
pub struct RustAnalyzerish {
    host: AnalysisHost,
    vfs: Vfs,
    loader: vfs_notify::NotifyHandle,
    message_receiver: crossbeam_channel::Receiver<ra_ap_vfs::loader::Message>,
    file_map: BiMap<PathBuf, FileId>,
    current_project_root: Option<PathBuf>,
}

impl Default for RustAnalyzerish {
    fn default() -> Self {
        Self::new()
    }
}

impl RustAnalyzerish {
    /// Create a new RustAnalyzer instance
    pub fn new() -> Self {
        let (message_sender, message_receiver) = crossbeam_channel::unbounded();
        let vfs = Vfs::default();
        let loader = vfs_notify::NotifyHandle::spawn(message_sender);

        Self {
            host: AnalysisHost::new(Some(100)),
            vfs,
            loader,
            message_receiver,
            // TODO Remove file_map and rely on VFS for file management
            file_map: BiMap::new(),
            current_project_root: None,
        }
    }

    // TODO Change output to use a more structured format
    /// Get type hint information at the specified cursor position
    pub async fn get_type_hint(
        &mut self,
        file_path: &str,
        line: u32,
        column: u32,
    ) -> Result<Option<TypeHint>> {
        let path = PathBuf::from(file_path);

        // Ensure the project/workspace is loaded
        self.ensure_project_loaded(&path).await?;

        // Load the file if not already loaded
        let file_id = self.load_file(&path).await.context("Failed to load file")?;

        let analysis = self.host.analysis();

        // Get the file's line index for position conversion
        let line_index = analysis
            .file_line_index(file_id)
            .map_err(|_| anyhow::anyhow!("Failed to get line index"))?;

        // Convert line/column to text offset from 1-based to 0-based indexing
        let line_col = LineCol {
            line: line.saturating_sub(1),
            col: column.saturating_sub(1),
        };
        let offset = line_index
            .offset(line_col)
            .ok_or_else(|| anyhow::anyhow!("Invalid line/column position: {}:{}", line, column))?;

        // Create TextRange for the hover query
        let text_range = TextRange::new(offset, offset + TextSize::from(1));

        let hover_config = HoverConfig {
            links_in_hover: true,
            memory_layout: None,
            documentation: true,
            keywords: true,
            // TODO Consider using Markdown but figure out how to reliably show symbol names too
            format: HoverDocFormat::PlainText,
            max_trait_assoc_items_count: Some(10),
            max_fields_count: Some(10),
            max_enum_variants_count: Some(10),
            max_subst_ty_len: SubstTyLen::Hide,
            show_drop_glue: false,
        };

        debug!(
            "Attempting hover query for file {:?} at offset {:?} (line {} col {})",
            file_id, offset, line, column
        );

        // Try hover with the configured settings
        let hover_result = match analysis.hover(
            &hover_config,
            FileRange {
                file_id,
                range: text_range,
            },
        ) {
            Ok(Some(hover_result)) => hover_result,
            Ok(None) => {
                debug!(
                    "No hover info available for {}:{}:{}",
                    file_path, line, column
                );
                return Ok(None);
            }
            Err(e) => {
                warn!("Hover analysis failed: {:?}", e);
                bail!("Hover analysis failed: {:?}", e)
            }
        };

        // Get the type information from hover
        let type_info = hover_result.info.markup.as_str();

        // Try to get the symbol name using goto_definition
        let symbol_name =
            match analysis.goto_definition(ra_ap_ide::FilePosition { file_id, offset }) {
                Ok(Some(range_info)) => {
                    // Look for a local definition that represents the variable
                    if let Some(nav) = range_info.info.first() {
                        // Check if this is a local variable by looking at the definition location
                        if nav.file_id == file_id {
                            Some(nav.name.to_string())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            };

        // Combine symbol name with type information
        let result = if let Some(ref name) = symbol_name {
            format!("{name}: {type_info}")
        } else {
            type_info.to_string()
        };

        debug!(
            "Got type hint for {}:{}:{}: {}",
            file_path, line, column, result
        );

        let type_hint = TypeHint {
            file_path: file_path.to_string(),
            line,
            column,
            symbol: symbol_name.unwrap_or_else(|| "unknown".to_string()),
            short_type: type_info.to_string(),
            canonical_type: type_info.to_string(), // TODO
        };

        Ok(Some(type_hint))
    }

    /// Get completion suggestions at the specified cursor position
    pub async fn get_completions(
        &mut self,
        file_path: &str,
        line: u32,
        column: u32,
    ) -> Result<Option<Vec<CompletionItem>>> {
        let path = PathBuf::from(file_path);

        self.ensure_project_loaded(&path).await?;

        let file_id = self.load_file(&path).await.context("Failed to load file")?;

        let analysis = self.host.analysis();

        let line_index = analysis
            .file_line_index(file_id)
            .map_err(|_| anyhow::anyhow!("Failed to get line index"))?;

        let line_col = LineCol {
            line: line.saturating_sub(1),
            col: column.saturating_sub(1),
        };
        let offset = line_index
            .offset(line_col)
            .ok_or_else(|| anyhow::anyhow!("Invalid line/column position: {}:{}", line, column))?;

        debug!(
            "Attempting completions query for file {:?} at offset {:?} (line {} col {})",
            file_id, offset, line, column
        );

        let position = FilePosition { file_id, offset };

        let config = CompletionConfig {
            enable_postfix_completions: true,
            enable_imports_on_the_fly: false, // Keep simple for now
            enable_self_on_the_fly: false,
            enable_auto_iter: true,
            enable_auto_await: true,
            enable_private_editable: false,
            enable_term_search: false,
            term_search_fuel: 400,
            full_function_signatures: false,
            callable: Some(CallableSnippets::FillArguments),
            add_semicolon_to_unit: false,
            snippet_cap: None, // Disable snippets for simplicity
            insert_use: InsertUseConfig {
                granularity: ImportGranularity::Crate,
                enforce_granularity: true,
                prefix_kind: PrefixKind::Plain,
                group: true,
                skip_glob_imports: true,
            },
            prefer_no_std: false,
            prefer_prelude: true,
            prefer_absolute: false,
            snippets: vec![],
            limit: Some(200), // Limit results for performance
            fields_to_resolve: CompletionFieldsToResolve::empty(),
            exclude_flyimport: vec![],
            exclude_traits: &[],
        };

        match analysis.completions(&config, position, Some('.')) {
            Ok(Some(ra_completions)) => {
                let mut completions = Vec::new();

                for ra_completion in ra_completions {
                    // Convert rust-analyzer CompletionItem to our CompletionItem
                    let kind = match ra_completion.kind {
                        RaCompletionItemKind::SymbolKind(symbol_kind) => {
                            Some(format!("{:?}", symbol_kind))
                        }
                        RaCompletionItemKind::Binding => Some("Binding".to_string()),
                        RaCompletionItemKind::BuiltinType => Some("BuiltinType".to_string()),
                        RaCompletionItemKind::InferredType => Some("InferredType".to_string()),
                        RaCompletionItemKind::Keyword => Some("Keyword".to_string()),
                        RaCompletionItemKind::Snippet => Some("Snippet".to_string()),
                        RaCompletionItemKind::UnresolvedReference => {
                            Some("UnresolvedReference".to_string())
                        }
                        RaCompletionItemKind::Expression => Some("Expression".to_string()),
                    };

                    let documentation = ra_completion
                        .documentation
                        .map(|doc| doc.as_str().to_string());

                    // TODO Consider label left/right details
                    let name = ra_completion.label.primary.into();
                    let required_import = if ra_completion.import_to_add.is_empty() {
                        None
                    } else {
                        Some(ra_completion.import_to_add.join(", "))
                    };

                    let completion = CompletionItem {
                        name,
                        required_import,
                        kind,
                        signature: ra_completion.detail,
                        documentation,
                        deprecated: ra_completion.deprecated,
                    };

                    completions.push(completion);
                }

                debug!(
                    "Found {} completions for {}:{}:{}",
                    completions.len(),
                    file_path,
                    line,
                    column
                );

                Ok(Some(completions))
            }
            Ok(None) => {
                debug!(
                    "No completions available for {}:{}:{}",
                    file_path, line, column
                );
                Ok(None)
            }
            Err(e) => {
                warn!("Completion analysis failed: {:?}", e);
                bail!("Completion analysis failed: {:?}", e)
            }
        }
    }

    /// Get definition information at the specified cursor position
    pub async fn get_definition(
        &mut self,
        file_path: &str,
        line: u32,
        column: u32,
    ) -> Result<Option<Vec<DefinitionInfo>>> {
        let path = PathBuf::from(file_path);

        // Ensure the project/workspace is loaded
        self.ensure_project_loaded(&path).await?;

        // Load the file if not already loaded
        let file_id = self.load_file(&path).await.context("Failed to load file")?;

        let analysis = self.host.analysis();

        // Get the file's line index for position conversion
        let line_index = analysis
            .file_line_index(file_id)
            .map_err(|_| anyhow::anyhow!("Failed to get line index"))?;

        // Convert line/column to text offset from 1-based to 0-based indexing
        let line_col = LineCol {
            line: line.saturating_sub(1),
            col: column.saturating_sub(1),
        };
        let offset = line_index
            .offset(line_col)
            .ok_or_else(|| anyhow::anyhow!("Invalid line/column position: {}:{}", line, column))?;

        debug!(
            "Attempting goto_definition query for file {:?} at offset {:?} (line {} col {})",
            file_id, offset, line, column
        );

        // Debug the current character at the offset
        if let Ok(source_text) = analysis.file_text(file_id) {
            let offset_usize: usize = offset.into();
            if offset_usize < source_text.len() {
                let current_char = source_text[offset_usize..].chars().next().unwrap_or('?');
                println!("Current character at offset {offset:?}: '{current_char}'");
            } else {
                debug!(
                    "Offset {:?} is out of bounds for file text length {}",
                    offset,
                    source_text.len()
                );
            }
        } else {
            debug!("Failed to read source text for file ID {:?}", file_id);
        }

        // Query for definitions
        // Use std::panic::catch_unwind to handle potential panics in rust-analyzer
        // Happens when we query colum: 1 row: 1
        // TODO Report bug
        let goto_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            analysis.goto_definition(ra_ap_ide::FilePosition { file_id, offset })
        }));

        let definitions_result = match goto_result {
            Ok(result) => result,
            Err(_panic) => {
                debug!(
                    "Caught panic during goto_definition for {}:{}:{}, likely due to edge case in rust-analyzer",
                    file_path, line, column
                );
                return Ok(None);
            }
        };

        match definitions_result {
            Ok(Some(range_info)) => {
                let mut definitions = Vec::new();

                for nav in range_info.info {
                    // Get file path from file_id
                    if let Ok(line_index) = analysis.file_line_index(nav.file_id) {
                        let start_line_col = line_index.line_col(nav.focus_or_full_range().start());
                        let end_line_col = line_index.line_col(nav.focus_or_full_range().end());

                        let file_path = if self.vfs.exists(nav.file_id) {
                            let vfs_path = self.vfs.file_path(nav.file_id);
                            vfs_path.to_string()
                        } else {
                            return Err(anyhow::anyhow!(
                                "File ID {:?} not found in VFS",
                                &nav.file_id
                            ));
                        };

                        // Get module path using moniker if available
                        let module = if let Ok(Some(moniker_info)) =
                            analysis.moniker(ra_ap_ide::FilePosition {
                                file_id: nav.file_id,
                                offset: nav.focus_or_full_range().start(),
                            }) {
                            // Extract module path from moniker
                            match &moniker_info.info.first() {
                                Some(ra_ap_ide::MonikerResult::Moniker(moniker)) => {
                                    // Build full module path from crate name and description
                                    let crate_name = &moniker.identifier.crate_name;
                                    let module_parts: Vec<String> = moniker
                                        .identifier
                                        .description
                                        .iter()
                                        .map(|desc| desc.name.to_string())
                                        .collect();

                                    if module_parts.is_empty() {
                                        crate_name.clone()
                                    } else {
                                        format!("{}::{}", crate_name, module_parts.join("::"))
                                    }
                                }
                                Some(ra_ap_ide::MonikerResult::Local { .. }) => {
                                    // For local symbols, fall back to container name
                                    nav.container_name
                                        .as_ref()
                                        .map(|name| name.to_string())
                                        .unwrap_or_else(|| "local".to_string())
                                }
                                None => {
                                    // Fall back to container name
                                    nav.container_name
                                        .as_ref()
                                        .map(|name| name.to_string())
                                        .unwrap_or_else(|| "unknown".to_string())
                                }
                            }
                        } else {
                            // Fall back to container name if moniker fails
                            nav.container_name
                                .as_ref()
                                .map(|name| name.to_string())
                                .unwrap_or_else(|| "unknown".to_string())
                        };

                        // Extract definition content from source
                        let content = if let Ok(source_text) = analysis.file_text(nav.file_id) {
                            let full_range = nav.full_range;
                            let start_offset = full_range.start().into();
                            let end_offset = full_range.end().into();

                            if start_offset < source_text.len() && end_offset <= source_text.len() {
                                source_text[start_offset..end_offset].to_string()
                            } else {
                                format!(
                                    "// Content extraction failed: invalid range {start_offset}..{end_offset}"
                                )
                            }
                        } else {
                            "// Content extraction failed: could not read source".to_string()
                        };

                        definitions.push(DefinitionInfo {
                            file_path,
                            line: start_line_col.line + 1, // Convert back to 1-based
                            column: start_line_col.col + 1, // Convert back to 1-based
                            end_line: end_line_col.line + 1,
                            end_column: end_line_col.col + 1,
                            name: nav.name.to_string(),
                            kind: nav.kind,
                            description: nav.description.clone(),
                            module,
                            content,
                        });
                    }
                }

                debug!(
                    "Found {} definitions for {}:{}:{}",
                    definitions.len(),
                    file_path,
                    line,
                    column
                );
                Ok(Some(definitions))
            }
            Ok(None) => {
                debug!(
                    "No definitions available for {}:{}:{}",
                    file_path, line, column
                );
                Ok(None)
            }
            Err(e) => {
                warn!("Goto definition analysis failed: {:?}", e);
                bail!("Goto definition analysis failed: {:?}", e)
            }
        }
    }

    /// Rename a symbol at the specified cursor position and apply the changes
    /// to disk
    pub async fn rename_symbol(
        &mut self,
        file_path: &str,
        line: u32,
        column: u32,
        new_name: &str,
    ) -> Result<Option<RenameResult>> {
        // Get the rename information
        let rename_result = self
            .get_rename_info(file_path, line, column, new_name)
            .await?;

        if let Some(ref result) = rename_result {
            // Apply the edits to disk
            Self::apply_rename_edits(result).await?;
        }

        Ok(rename_result)
    }

    /// Get rename information without applying changes to disk
    pub async fn get_rename_info(
        &mut self,
        file_path: &str,
        line: u32,
        column: u32,
        new_name: &str,
    ) -> Result<Option<RenameResult>> {
        let path = PathBuf::from(file_path);

        // Ensure the project/workspace is loaded
        self.ensure_project_loaded(&path).await?;

        // Load the file if not already loaded
        let file_id = self.load_file(&path).await.context("Failed to load file")?;

        let analysis = self.host.analysis();

        // Get the file's line index for position conversion
        let line_index = analysis
            .file_line_index(file_id)
            .map_err(|_| anyhow::anyhow!("Failed to get line index"))?;

        // Convert line/column to text offset from 1-based to 0-based indexing
        let line_col = LineCol {
            line: line.saturating_sub(1),
            col: column.saturating_sub(1),
        };
        let offset = line_index
            .offset(line_col)
            .ok_or_else(|| anyhow::anyhow!("Invalid line/column position: {}:{}", line, column))?;

        debug!(
            "Attempting rename for file {:?} at offset {:?} (line {} col {}) to '{}'",
            file_id, offset, line, column, new_name
        );

        let position = FilePosition { file_id, offset };

        // TODO Consider separating this to a separate tool
        // First, prepare the rename to validate it's possible
        // let prepare_result = match analysis.prepare_rename(position) {
        //     Ok(result) => result,
        //     Err(e) => {
        //         warn!("Failed to prepare rename: {:?}", e);
        //         bail!("Failed to prepare rename: {:?}", e)
        //     }
        // };

        // let _prepare_range_info = match prepare_result {
        //     Ok(range_info) => range_info,
        //     Err(rename_error) => {
        //         debug!("Rename not possible: {:?}", rename_error);
        //         return Ok(None);
        //     }
        // };

        // Perform the actual rename
        let rename_result = match analysis.rename(position, new_name) {
            Ok(result) => result,
            Err(e) => {
                warn!("Failed to perform rename: {:?}", e);
                bail!("Failed to perform rename: {:?}", e)
            }
        };

        let source_change = match rename_result {
            Ok(source_change) => source_change,
            Err(rename_error) => {
                debug!("Rename failed: {:?}", rename_error);
                return Ok(None);
            }
        };

        // Convert SourceChange to our RenameResult format
        let mut file_changes = Vec::new();

        for (file_id, edit_tuple) in source_change.source_file_edits {
            // Get file path from file_id
            let file_path = if self.vfs.exists(file_id) {
                let vfs_path = self.vfs.file_path(file_id);
                vfs_path.to_string()
            } else {
                return Err(anyhow::anyhow!("File ID {:?} not found in VFS", file_id));
            };

            // Get line index for this file
            let file_line_index = analysis
                .file_line_index(file_id)
                .map_err(|_| anyhow::anyhow!("Failed to get line index for file {:?}", file_id))?;

            // Convert text edits - the tuple is (TextEdit, Option<SnippetEdit>)
            let mut edits = Vec::new();
            let text_edit = &edit_tuple.0; // Get the TextEdit from the tuple

            for edit in text_edit.iter() {
                let start_line_col = file_line_index.line_col(edit.delete.start());
                let end_line_col = file_line_index.line_col(edit.delete.end());

                edits.push(TextEdit {
                    line: start_line_col.line + 1,  // Convert to 1-based
                    column: start_line_col.col + 1, // Convert to 1-based
                    end_line: end_line_col.line + 1,
                    end_column: end_line_col.col + 1,
                    new_text: edit.insert.clone(),
                });
            }

            file_changes.push(FileChange { file_path, edits });
        }

        debug!(
            "Rename successful: {} file(s) will be changed",
            file_changes.len()
        );

        Ok(Some(RenameResult { file_changes }))
    }

    /// Ensure the project workspace is loaded for the given file path
    async fn ensure_project_loaded(&mut self, file_path: &Path) -> Result<()> {
        let project_root = self.find_project_root(file_path)?;

        // Check if we already loaded a project
        // TODO Support multiple projects
        if self.current_project_root.is_some() {
            if self.current_project_root.as_ref() == Some(&project_root) {
                return Ok(());
            } else {
                error!(
                    "Attempting to change workspaces, from {:?} to {:?}.",
                    self.current_project_root, project_root
                );
                return Err(anyhow::anyhow!(
                    "Cannot change workspaces after a project has already been loaded. Current: {:?}, New: {:?}",
                    self.current_project_root,
                    project_root
                ));
            }
        }

        info!("Loading project workspace from: {}", project_root.display());
        self.load_workspace(&project_root).await?;
        self.current_project_root = Some(project_root);

        Ok(())
    }

    /// Find the project root by looking for Cargo.toml
    fn find_project_root(&self, file_path: &Path) -> Result<PathBuf> {
        let mut current = if file_path.is_absolute() {
            info!(
                "Finding project root for absolute path: {}",
                file_path.display()
            );
            file_path.to_path_buf()
        } else {
            info!(
                "Finding project root for relative path: {}",
                file_path.display()
            );
            std::env::current_dir()?.join(file_path)
        };

        loop {
            info!("Checking for Cargo.toml in: {}", current.display());
            let cargo_toml = current.join("Cargo.toml");
            if cargo_toml.exists() {
                return Ok(current.to_path_buf());
            }

            if let Some(parent) = current.parent() {
                current = parent.to_path_buf();
            } else {
                // No Cargo.toml found, create a temporary project structure
                return Err(anyhow::anyhow!(
                    "No Cargo.toml found in parent directories of {}",
                    file_path.display()
                ));
            }
        }
    }

    /// Load workspace using evcxr's approach
    async fn load_workspace(&mut self, project_root: &Path) -> Result<()> {
        let mut change = ChangeWithProcMacros::default();

        let cargo_toml_path = project_root.join("Cargo.toml");

        if cargo_toml_path.exists() {
            // Load project using Cargo.toml
            let abs_cargo_toml = cargo_toml_path.canonicalize().with_context(|| {
                format!("Failed to canonicalize path: {}", cargo_toml_path.display())
            })?;
            let manifest =
                ProjectManifest::from_manifest_file(AbsPathBuf::assert_utf8(abs_cargo_toml))?;
            let config = CargoConfig {
                sysroot: Some(RustLibSource::Discover),
                ..CargoConfig::default()
            };

            let workspace = ProjectWorkspace::load(manifest, &config, &|_| {})?;

            // Set up VFS loader
            let load = workspace
                .to_roots()
                .iter()
                .map(|root| {
                    ra_ap_vfs::loader::Entry::Directories(ra_ap_vfs::loader::Directories {
                        extensions: vec!["rs".to_owned()],
                        include: root.include.clone(),
                        exclude: root.exclude.clone(),
                    })
                })
                .collect();

            self.loader.set_config(ra_ap_vfs::loader::Config {
                version: 1,
                load,
                watch: vec![],
            });

            // Process VFS messages
            // TODO Perform this in a concurrent tokio task
            for message in &self.message_receiver {
                match message {
                    ra_ap_vfs::loader::Message::Progress { n_done, .. } => {
                        if n_done == LoadingProgress::Finished {
                            break;
                        }
                    }
                    ra_ap_vfs::loader::Message::Loaded { files }
                    | ra_ap_vfs::loader::Message::Changed { files } => {
                        for (path, contents) in files {
                            let vfs_path: VfsPath = path.to_path_buf().into();
                            self.vfs.set_file_contents(vfs_path, contents.clone());
                        }
                    }
                }
            }

            // Apply VFS changes
            for (file_id, changed_file) in self.vfs.take_changes() {
                let new_contents = match changed_file.change {
                    ra_ap_vfs::Change::Create(v, _) | ra_ap_vfs::Change::Modify(v, _) => {
                        if let Ok(text) = std::str::from_utf8(&v) {
                            Some(text.to_owned())
                        } else {
                            None
                        }
                    }
                    ra_ap_vfs::Change::Delete => None,
                };
                change.change_file(file_id, new_contents);
            }

            // Set up source roots
            change.set_roots(
                ra_ap_vfs::file_set::FileSetConfig::default()
                    .partition(&self.vfs)
                    .into_iter()
                    .map(SourceRoot::new_local)
                    .collect(),
            );

            // Set up crate graph
            let (crate_graph, _) = workspace.to_crate_graph(
                &mut |path| {
                    self.vfs
                        .file_id(&path.to_path_buf().into())
                        .map(|(id, _)| id)
                },
                &FxHashMap::default(),
            );

            change.set_crate_graph(crate_graph);
        } else {
            // Create minimal project structure if no Cargo.toml
            debug!(
                "No Cargo.toml found in project root: {}",
                project_root.display()
            );
            Err(anyhow::anyhow!(
                "No Cargo.toml found in project root: {}",
                project_root.display()
            ))?;
        }

        self.host.apply_change(change);
        debug!("Workspace loaded successfully");
        Ok(())
    }

    /// Load a file into the analysis host
    async fn load_file(&mut self, path: &Path) -> Result<FileId> {
        // Check if file is already loaded
        // TODO Check in VFS if file is loaded
        if let Some(&file_id) = self.file_map.get_by_left(path) {
            return Ok(file_id);
        }

        // Read file contents
        let contents = tokio::fs::read_to_string(path)
            .await
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        // Add file to VFS
        let abs_path =
            AbsPathBuf::assert_utf8(path.canonicalize().unwrap_or_else(|_| path.to_path_buf()));
        let vfs_path: VfsPath = abs_path.into();
        self.vfs
            .set_file_contents(vfs_path.clone(), Some(contents.bytes().collect()));

        let (file_id, _) = self
            .vfs
            .file_id(&vfs_path)
            .ok_or_else(|| anyhow::anyhow!("Failed to get file ID from VFS"))?;

        // Update file contents in the change
        let mut change = ChangeWithProcMacros::default();
        change.change_file(file_id, Some(contents));
        self.host.apply_change(change);

        self.file_map.insert(path.to_path_buf(), file_id);

        debug!("Loaded file: {} -> {:?}", path.display(), file_id);
        Ok(file_id)
    }

    /// Apply rename edits to files on disk using rust-analyzer's
    /// TextEditBuilder
    pub async fn apply_rename_edits(rename_result: &RenameResult) -> anyhow::Result<()> {
        use ra_ap_ide::{TextRange, TextSize};
        use ra_ap_ide_db::text_edit::TextEditBuilder;
        use tokio::fs;

        for file_change in &rename_result.file_changes {
            // Read the current file content
            let mut content = fs::read_to_string(&file_change.file_path)
                .await
                .map_err(|e| {
                    anyhow::anyhow!("Failed to read file {}: {}", file_change.file_path, e)
                })?;

            // Create TextEditBuilder to handle multiple edits atomically
            let mut builder = TextEditBuilder::default();

            // Create line index for position conversion
            let lines: Vec<&str> = content.lines().collect();

            // Add all edits to the builder (no need to sort - TextEditBuilder handles
            // ordering)
            for edit in &file_change.edits {
                // TODO use line_index.offset

                // Get the file's line index for position conversion
                // let line_index = analysis
                // .file_line_index(file_id)
                // .map_err(|_| anyhow::anyhow!("Failed to get line index"))?;

                // Convert line/column to text offset from 1-based to 0-based indexing
                // let line_col = LineCol {
                //     line: line.saturating_sub(1),
                //     col: column.saturating_sub(1),
                // };
                // let offset = line_index
                //     .offset(line_col)
                //     .ok_or_else(|| anyhow::anyhow!("Invalid line/column position: {}:{}",
                // line, column))?;

                // Convert 1-based line/column to character offset
                let start_offset = Self::line_col_to_offset(&lines, edit.line, edit.column)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Invalid start position {}:{} in file {}",
                            edit.line,
                            edit.column,
                            file_change.file_path
                        )
                    })?;

                let end_offset = Self::line_col_to_offset(&lines, edit.end_line, edit.end_column)
                    .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Invalid end position {}:{} in file {}",
                        edit.end_line,
                        edit.end_column,
                        file_change.file_path
                    )
                })?;

                // Create rust-analyzer TextRange
                let range = TextRange::new(
                    TextSize::from(start_offset as u32),
                    TextSize::from(end_offset as u32),
                );

                // Add the replacement to the builder
                builder.replace(range, edit.new_text.clone());
            }

            // Build the TextEdit and apply it atomically
            let text_edit = builder.finish();
            text_edit.apply(&mut content);

            // Write the modified content back to the file
            fs::write(&file_change.file_path, content)
                .await
                .map_err(|e| {
                    anyhow::anyhow!("Failed to write file {}: {}", file_change.file_path, e)
                })?;
        }

        Ok(())
    }

    // TODO Use ra line_index.offset instead of this implementation
    /// Convert 1-based line/column to 0-based character offset
    fn line_col_to_offset(lines: &[&str], line: u32, column: u32) -> Option<usize> {
        let line_idx = (line.saturating_sub(1)) as usize;
        let col_idx = (column.saturating_sub(1)) as usize;

        if line_idx >= lines.len() {
            return None;
        }

        // Calculate offset by summing all characters in previous lines plus newlines
        let mut offset = 0;
        for line in lines.iter().take(line_idx) {
            offset += line.len() + 1; // +1 for newline character
        }

        // Add column offset, but ensure it doesn't exceed line length
        let line_content = lines[line_idx];
        if col_idx > line_content.len() {
            return None;
        }

        offset += col_idx;
        Some(offset)
    }
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
            "{}:{}:{}\n{}",
            self.file_path, self.line, self.column, self.content
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

impl std::fmt::Display for CompletionItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)?;
        if let Some(ref kind) = self.kind {
            write!(f, " ({kind})")?;
        }
        if let Some(ref sig) = self.signature {
            write!(f, " - {sig}")?;
        }
        Ok(())
    }
}
