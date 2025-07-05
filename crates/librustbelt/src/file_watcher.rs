//! File watching module for rust-analyzer integration
//!
//! This module handles file system watching and VFS synchronization,
//! keeping the analysis host updated with file changes.

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, unbounded};
use ra_ap_ide::{AnalysisHost, FileId};
use ra_ap_ide_db::ChangeWithProcMacros;
use ra_ap_vfs::loader::{Handle, Message as LoaderMessage};
use ra_ap_vfs::{AbsPathBuf, Vfs, VfsPath};
use ra_ap_vfs_notify::NotifyHandle;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::{debug, info, trace};

/// File watching configuration and state
#[derive(Debug)]
pub struct FileWatcher {
    vfs_receiver: Option<Receiver<LoaderMessage>>,
    vfs_handle: Option<NotifyHandle>,
    file_watcher_task: Option<JoinHandle<()>>,
    vfs: Arc<Mutex<Vfs>>,
}

impl Default for FileWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl FileWatcher {
    /// Create a new file watcher
    pub fn new() -> Self {
        Self {
            vfs_receiver: None,
            vfs_handle: None,
            file_watcher_task: None,
            vfs: Arc::new(Mutex::new(Vfs::default())),
        }
    }

    /// Set up file watching for the workspace
    pub async fn setup_file_watching(
        &mut self,
        abs_project_root: AbsPathBuf,
        vfs: Arc<Mutex<Vfs>>,
        host: Arc<Mutex<AnalysisHost>>,
    ) -> Result<()> {
        tracing::info!(
            "Setting up file watching for workspace: {}",
            abs_project_root
        );

        // Replace our VFS with the loaded workspace VFS
        self.vfs = vfs;

        // Create a channel for VFS loader messages
        let (sender, receiver) = unbounded::<LoaderMessage>();

        // Start the file watcher using the same pattern as rust-analyzer
        let vfs_handle: NotifyHandle = Handle::spawn(sender);

        // Store the receiver and handle
        self.vfs_receiver = Some(receiver);
        self.vfs_handle = Some(vfs_handle);

        // Configure the VFS to watch the workspace files
        self.configure_vfs_watching(abs_project_root).await?;

        // Spawn the file watcher task to process changes
        self.spawn_file_watcher_task(host).await?;

        Ok(())
    }

    /// Spawn a background task to continuously process file changes
    pub async fn spawn_file_watcher_task(&mut self, host: Arc<Mutex<AnalysisHost>>) -> Result<()> {
        let Some(ref receiver) = self.vfs_receiver else {
            return Err(anyhow::anyhow!("VFS receiver not initialized"));
        };

        // Clone the receiver for the background task
        let receiver_clone = receiver.clone();
        let vfs_clone = self.vfs();

        let task_handle = tokio::spawn(async move {
            Self::file_watcher_task_loop(receiver_clone, vfs_clone, host).await;
        });

        self.file_watcher_task = Some(task_handle);
        Ok(())
    }

    /// Background task loop that continuously processes file changes
    async fn file_watcher_task_loop(
        receiver: Receiver<LoaderMessage>,
        vfs: Arc<Mutex<Vfs>>,
        host: Arc<Mutex<AnalysisHost>>,
    ) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));

        loop {
            interval.tick().await;

            let mut has_changes = false;
            let mut change = ChangeWithProcMacros::default();

            // Process all pending messages from the file watcher
            while let Ok(message) = receiver.try_recv() {
                match message {
                    LoaderMessage::Progress {
                        n_total, n_done, ..
                    } => {
                        trace!("File watching progress: {:?}/{:?}", n_done, n_total);
                    }
                    LoaderMessage::Loaded { files } => {
                        debug!("Files initially loaded: {} files", files.len());
                        has_changes = true;

                        // Process the loaded files
                        for (abs_path, contents) in files {
                            debug!("File loaded: {:?}", abs_path);

                            // Convert AbsPath to VfsPath and get FileId
                            let vfs_path: VfsPath = abs_path.into();
                            let vfs_guard = vfs.lock().await;
                            if let Some((file_id, _)) = vfs_guard.file_id(&vfs_path) {
                                // Convert contents to String for the analysis host
                                let text_contents =
                                    contents.and_then(|bytes| String::from_utf8(bytes).ok());

                                // Add to the change for the analysis host
                                change.change_file(file_id, text_contents);
                            }
                        }
                    }
                    LoaderMessage::Changed { files } => {
                        debug!("Files changed: {} files", files.len());
                        has_changes = true;

                        // Process the changed files
                        for (abs_path, contents) in files {
                            debug!("File changed: {:?}", abs_path);

                            // Convert AbsPath to VfsPath and get FileId
                            let vfs_path: VfsPath = abs_path.into();
                            {
                                let mut vfs_guard = vfs.lock().await;
                                if let Some((file_id, _)) = vfs_guard.file_id(&vfs_path) {
                                    // Update VFS with new contents
                                    vfs_guard.set_file_contents(vfs_path, contents.clone());

                                    // Convert contents to String for the analysis host
                                    let text_contents =
                                        contents.and_then(|bytes| String::from_utf8(bytes).ok());

                                    // Add to the change for the analysis host
                                    change.change_file(file_id, text_contents);
                                }
                            }
                        }
                    }
                }
            }

            // Apply all changes to the analysis host if we have any
            if has_changes {
                let mut host_guard = host.lock().await;
                host_guard.apply_change(change);
                debug!("Applied file changes to analysis host");
            }
        }
    }

    /// Configure VFS to watch workspace files
    async fn configure_vfs_watching(&mut self, abs_project_root: AbsPathBuf) -> Result<()> {
        let Some(ref mut loader) = self.vfs_handle else {
            return Ok(());
        };

        debug!("Configuring VFS watching for: {}", abs_project_root);

        let config = ra_ap_vfs::loader::Config {
            load: vec![
                // Watch the entire project directory for changes
                ra_ap_vfs::loader::Entry::Directories(ra_ap_vfs::loader::Directories {
                    extensions: vec!["rs".to_string(), "toml".to_string()],
                    include: vec![abs_project_root.clone()],
                    exclude: vec![
                        abs_project_root.join("target"),
                        abs_project_root.join(".git"),
                    ],
                }),
            ],
            watch: vec![0], // Watch the first (and only) load entry
            version: 0,
        };

        // Set the configuration on the loader
        loader.set_config(config);

        debug!("VFS watching configuration set");
        Ok(())
    }

    pub async fn wait_for_file(&self, path: &Path) -> Result<FileId> {
        let vfs_path = Self::path_to_vfs_path(path)?;

        // Wait for the file to be loaded into the VFS
        let max_attempts = 100; // Wait up to 10 seconds (100 * 100ms)
        let mut attempts = 0;

        loop {
            {
                let vfs_guard = self.vfs.lock().await;
                if let Some((file_id, _)) = vfs_guard.file_id(&vfs_path) {
                    info!("File {} loaded into VFS as {:?}", path.display(), file_id);
                    return Ok(file_id);
                }
            }

            attempts += 1;
            if attempts >= max_attempts {
                return Err(anyhow::anyhow!(
                    "Timeout waiting for file to be loaded into VFS: {:?}",
                    path
                ));
            }

            debug!(
                "Waiting for file to be loaded into VFS: {} (attempt {}/{})",
                path.display(),
                attempts,
                max_attempts
            );

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    /// Check if a file exists in the VFS
    pub async fn file_exists(&self, file_id: FileId) -> bool {
        let vfs = self.vfs.lock().await;
        vfs.exists(file_id)
    }

    /// Get file path from file ID
    pub async fn file_path(&self, file_id: FileId) -> Option<String> {
        let vfs = self.vfs.lock().await;
        if vfs.exists(file_id) {
            Some(vfs.file_path(file_id).to_string())
        } else {
            None
        }
    }

    /// Get a reference to the VFS
    pub fn vfs(&self) -> Arc<Mutex<Vfs>> {
        Arc::clone(&self.vfs)
    }

    /// Convert a PathBuf to VfsPath for VFS operations
    pub fn path_to_vfs_path(path: &Path) -> Result<VfsPath> {
        let abs_path = AbsPathBuf::assert_utf8(
            path.canonicalize()
                .with_context(|| format!("Failed to canonicalize path: {}", path.display()))?,
        );
        Ok(abs_path.into())
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        if let Some(task) = self.file_watcher_task.take() {
            task.abort();
        }
    }
}
