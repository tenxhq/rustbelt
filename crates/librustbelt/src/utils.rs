//! Utility functions for Rust analyzer operations
//!
//! This module contains static utility functions that don't require
//! an analyzer instance, such as text editing operations.

use std::path::Path;

use anyhow::Result;
use ra_ap_ide::{LineCol, LineIndex, TextRange, TextSize};
use ra_ap_ide_db::text_edit::TextEditBuilder;
use tokio::fs;

use super::entities::{FileChange, RenameResult};

/// Utility functions for Rust analyzer operations
pub struct RustAnalyzerUtils;

impl RustAnalyzerUtils {
    /// Apply rename edits to files on disk using rust-analyzer's TextEditBuilder
    pub async fn apply_rename_edits(rename_result: &RenameResult) -> Result<()> {
        for file_change in &rename_result.file_changes {
            // Read the current file content
            let mut content = fs::read_to_string(&file_change.file_path)
                .await
                .map_err(|e| {
                    anyhow::anyhow!("Failed to read file {}: {}", file_change.file_path, e)
                })?;

            // Create TextEditBuilder to handle multiple edits atomically
            let mut builder = TextEditBuilder::default();

            // Create line index for UTF-8 safe position conversion
            let line_index = LineIndex::new(&content);

            // Add all edits to the builder (no need to sort - TextEditBuilder handles ordering)
            for edit in &file_change.edits {
                // Convert 1-based line/column to character offset using LineIndex for UTF-8 safety
                let start_offset =
                    Self::line_col_to_offset_with_index(&line_index, edit.line, edit.column)
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "Invalid start position {}:{} in file {}",
                                edit.line,
                                edit.column,
                                file_change.file_path
                            )
                        })?;

                let end_offset = Self::line_col_to_offset_with_index(
                    &line_index,
                    edit.end_line,
                    edit.end_column,
                )
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Invalid end position {}:{} in file {}",
                        edit.end_line,
                        edit.end_column,
                        file_change.file_path
                    )
                })?;

                // Create rust-analyzer TextRange
                let range = TextRange::new(start_offset, end_offset);

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

    /// Convert 1-based line/column to TextSize offset using LineIndex for UTF-8 safety
    pub fn line_col_to_offset_with_index(
        line_index: &LineIndex,
        line: u32,
        column: u32,
    ) -> Option<TextSize> {
        let line_col = LineCol {
            line: line.saturating_sub(1),
            col: column.saturating_sub(1),
        };
        line_index.offset(line_col)
    }

    /// Helper method to get line content from file text
    pub fn get_line_content(file_text: &str, line_number: usize) -> Option<String> {
        let lines: Vec<&str> = file_text.lines().collect();
        if line_number < lines.len() {
            Some(lines[line_number].to_string())
        } else {
            None
        }
    }

    /// Convert a PathBuf to AbsPathBuf for rust-analyzer operations
    pub fn path_to_abs_path(path: &Path) -> Result<ra_ap_vfs::AbsPathBuf> {
        use anyhow::Context;
        use ra_ap_vfs::AbsPathBuf;

        let abs_path = AbsPathBuf::assert_utf8(
            path.canonicalize()
                .with_context(|| format!("Failed to canonicalize path: {}", path.display()))?,
        );
        Ok(abs_path)
    }

    /// Apply a file change to disk (used by assists)
    pub async fn apply_file_change(file_change: &FileChange) -> Result<()> {
        // Read the current file content
        let mut content = fs::read_to_string(&file_change.file_path)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read file {}: {}", file_change.file_path, e))?;

        // Create TextEditBuilder to handle multiple edits atomically
        let mut builder = TextEditBuilder::default();

        // Create line index for UTF-8 safe position conversion
        let line_index = LineIndex::new(&content);

        // Add all edits to the builder (no need to sort - TextEditBuilder handles ordering)
        for edit in &file_change.edits {
            // Convert 1-based line/column to character offset using LineIndex for UTF-8 safety
            let start_offset =
                Self::line_col_to_offset_with_index(&line_index, edit.line, edit.column)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Invalid start position {}:{} in file {}",
                            edit.line,
                            edit.column,
                            file_change.file_path
                        )
                    })?;

            let end_offset =
                Self::line_col_to_offset_with_index(&line_index, edit.end_line, edit.end_column)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Invalid end position {}:{} in file {}",
                            edit.end_line,
                            edit.end_column,
                            file_change.file_path
                        )
                    })?;

            let text_range = TextRange::new(start_offset, end_offset);
            builder.replace(text_range, edit.new_text.clone());
        }

        // Build the final text edit and apply it
        let text_edit = builder.finish();
        text_edit.apply(&mut content);

        // Write the modified content back to the file
        fs::write(&file_change.file_path, content)
            .await
            .map_err(|e| {
                anyhow::anyhow!("Failed to write file {}: {}", file_change.file_path, e)
            })?;

        Ok(())
    }
}
