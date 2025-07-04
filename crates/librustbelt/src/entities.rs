use ra_ap_ide_db::SymbolKind;

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
