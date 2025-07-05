use ra_ap_ide::LineCol;
use ra_ap_ide_db::SymbolKind;
use serde::{Deserialize, Serialize};

/// Cursor coordinates for specifying position in a file
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct CursorCoordinates {
    /// Absolute path to the Rust source file
    pub file_path: String,
    /// Line number (1-based)
    pub line: u32,
    /// Column number (1-based)
    pub column: u32,
}

impl From<&CursorCoordinates> for LineCol {
    fn from(cursor: &CursorCoordinates) -> Self {
        LineCol {
            line: cursor.line.saturating_sub(1),
            col: cursor.column.saturating_sub(1),
        }
    }
}

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
    pub canonical_types: Vec<String>,
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

/// Information about a reference location
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceInfo {
    /// Path to the file containing the reference
    pub file_path: String,
    /// Line number (1-based) where the reference starts
    pub line: u32,
    /// Column number (1-based) where the reference starts
    pub column: u32,
    /// Line number (1-based) where the reference ends
    pub end_line: u32,
    /// Column number (1-based) where the reference ends
    pub end_column: u32,
    /// Name of the referenced symbol
    pub name: String,
    /// Content of the reference (the line containing the reference)
    pub content: String,
    /// Whether this is a definition (true) or usage (false)
    pub is_definition: bool,
}

impl std::fmt::Display for TypeHint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}\n```md\n{}\n```\nRelevant types: {}",
            self.file_path,
            self.line,
            self.column,
            self.symbol,
            self.canonical_types.join(", ")
        )
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

impl std::fmt::Display for ReferenceInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ref_type = if self.is_definition { "def" } else { "ref" };
        write!(
            f,
            "{}:{}:{} ({}) - {}",
            self.file_path,
            self.line,
            self.column,
            ref_type,
            self.content.trim()
        )
    }
}
