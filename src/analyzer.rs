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
    AnalysisHost, FileId, FileRange, HoverConfig, HoverDocFormat, LineCol, SubstTyLen, TextRange,
    TextSize,
};
use ra_ap_ide_db::{ChangeWithProcMacros, FxHashMap, SymbolKind};
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

/// Main interface to rust-analyzer functionality
#[derive(Debug)]
pub struct RustAnalyzer {
    host: AnalysisHost,
    vfs: Vfs,
    loader: vfs_notify::NotifyHandle,
    message_receiver: crossbeam_channel::Receiver<ra_ap_vfs::loader::Message>,
    file_map: BiMap<PathBuf, FileId>,
    current_project_root: Option<PathBuf>,
}

impl Default for RustAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl RustAnalyzer {
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
    ) -> Result<Option<String>> {
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
        let result = if let Some(name) = symbol_name {
            format!("{name}: {type_info}")
        } else {
            type_info.to_string()
        };

        debug!(
            "Got type hint for {}:{}:{}: {}",
            file_path, line, column, result
        );
        Ok(Some(result))
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
}
