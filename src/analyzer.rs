//! Rust-Analyzer Integration Module
//!
//! This module provides a wrapper around rust-analyzer's IDE functionality,
//! making it easy to get type hints, definitions, and other semantic
//! information.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use ra_ap_base_db::SourceRoot;
use ra_ap_ide::{
    AnalysisHost, FileId, FileRange, HoverConfig, HoverDocFormat, LineCol, SubstTyLen, TextRange,
    TextSize,
};
use ra_ap_ide_db::{ChangeWithProcMacros, FxHashMap};
use ra_ap_project_model::{CargoConfig, ProjectManifest, ProjectWorkspace, RustLibSource};
use ra_ap_vfs::{
    AbsPathBuf, Vfs, VfsPath,
    loader::{Handle, LoadingProgress},
};
use ra_ap_vfs_notify as vfs_notify;
use tracing::{debug, info, warn};

/// Main interface to rust-analyzer functionality
#[derive(Debug)]
pub struct RustAnalyzer {
    host: AnalysisHost,
    vfs: Vfs,
    loader: vfs_notify::NotifyHandle,
    message_receiver: crossbeam_channel::Receiver<ra_ap_vfs::loader::Message>,
    file_map: HashMap<PathBuf, FileId>,
    current_project_root: Option<PathBuf>,
    workspace_loaded: bool,
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
            host: AnalysisHost::new(None),
            vfs,
            loader,
            message_receiver,
            file_map: HashMap::new(),
            current_project_root: None,
            workspace_loaded: false,
        }
    }

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
            format: HoverDocFormat::Markdown,
            max_trait_assoc_items_count: Some(5),
            max_fields_count: Some(5),
            max_enum_variants_count: Some(5),
            max_subst_ty_len: SubstTyLen::Unlimited,
            show_drop_glue: false,
        };

        debug!(
            "Attempting hover query for file {:?} at offset {:?} (line {} col {})",
            file_id, offset, line, column
        );

        // Try hover with the configured settings
        match analysis.hover(
            &hover_config,
            FileRange {
                file_id,
                range: text_range,
            },
        ) {
            Ok(Some(hover_result)) => {
                let markup = hover_result.info.markup.as_str();
                debug!(
                    "Got type hint for {}:{}:{}: {}",
                    file_path, line, column, markup
                );
                Ok(Some(markup.to_string()))
            }
            Ok(None) => {
                debug!(
                    "No hover info available for {}:{}:{}",
                    file_path, line, column
                );
                Ok(None)
            }
            Err(e) => {
                warn!("Hover analysis failed: {:?}", e);
                bail!("Hover analysis failed: {:?}", e)
            }
        }
    }

    /// Ensure the project workspace is loaded for the given file path
    async fn ensure_project_loaded(&mut self, file_path: &Path) -> Result<()> {
        let project_root = self.find_project_root(file_path)?;

        // Check if we already loaded this project
        if self.current_project_root.as_ref() == Some(&project_root) && self.workspace_loaded {
            return Ok(());
        }

        info!("Loading project workspace from: {}", project_root.display());
        self.load_workspace(&project_root).await?;
        self.current_project_root = Some(project_root);
        self.workspace_loaded = true;

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
                let mut new_contents = None;
                if let ra_ap_vfs::Change::Create(v, _) | ra_ap_vfs::Change::Modify(v, _) =
                    changed_file.change
                    && let Ok(text) = std::str::from_utf8(&v)
                {
                    new_contents = Some(text.to_owned());
                }
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
        if let Some(&file_id) = self.file_map.get(path) {
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
