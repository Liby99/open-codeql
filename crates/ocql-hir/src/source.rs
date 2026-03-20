use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ocql_common::Span;

use crate::FileId;

/// Owns all source text and provides file/line/column mapping.
pub struct SourceManager {
    files: Vec<FileEntry>,
    path_to_id: HashMap<PathBuf, FileId>,
}

struct FileEntry {
    path: PathBuf,
    source: String,
    /// Byte offsets of the start of each line (0-indexed).
    line_starts: Vec<usize>,
}

/// A human-readable source location.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceLocation {
    pub file: FileId,
    pub path: PathBuf,
    pub line: usize,   // 1-based
    pub column: usize,  // 1-based
}

impl std::fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.path.display(), self.line, self.column)
    }
}

impl SourceManager {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            path_to_id: HashMap::new(),
        }
    }

    /// Register a source file. Returns its FileId.
    /// If the file is already registered, returns the existing FileId.
    pub fn add_file(&mut self, path: PathBuf, source: String) -> FileId {
        if let Some(&id) = self.path_to_id.get(&path) {
            return id;
        }
        let id = FileId(self.files.len() as u32);
        let line_starts = compute_line_starts(&source);
        self.path_to_id.insert(path.clone(), id);
        self.files.push(FileEntry {
            path,
            source,
            line_starts,
        });
        id
    }

    /// Look up a file by path.
    pub fn get_file_id(&self, path: &Path) -> Option<FileId> {
        self.path_to_id.get(path).copied()
    }

    /// Get the source text for a file.
    pub fn source(&self, id: FileId) -> &str {
        &self.files[id.0 as usize].source
    }

    /// Get the file path.
    pub fn path(&self, id: FileId) -> &Path {
        &self.files[id.0 as usize].path
    }

    /// Number of registered files.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Convert a byte offset span to a human-readable location.
    pub fn location(&self, file: FileId, span: Span) -> SourceLocation {
        let entry = &self.files[file.0 as usize];
        let (line, col) = offset_to_line_col(&entry.line_starts, span.start);
        SourceLocation {
            file,
            path: entry.path.clone(),
            line,
            column: col,
        }
    }

    /// Extract the source text for a given span.
    pub fn span_text(&self, file: FileId, span: Span) -> &str {
        let source = &self.files[file.0 as usize].source;
        let start = span.start.min(source.len());
        let end = span.end.min(source.len());
        &source[start..end]
    }

    /// Get the line text containing a byte offset.
    pub fn line_text(&self, file: FileId, span: Span) -> &str {
        let entry = &self.files[file.0 as usize];
        let (line, _) = offset_to_line_col(&entry.line_starts, span.start);
        let line_start = entry.line_starts[line - 1];
        let line_end = if line < entry.line_starts.len() {
            entry.line_starts[line]
        } else {
            entry.source.len()
        };
        entry.source[line_start..line_end].trim_end_matches('\n').trim_end_matches('\r')
    }

    /// Format a diagnostic with source context, like:
    /// ```text
    /// error: undefined variable `y`
    ///  --> path/to/file.ql:5:12
    ///   |
    /// 5 | where y > 0
    ///   |       ^
    /// ```
    pub fn format_diagnostic(&self, diag: &crate::Diagnostic) -> String {
        use std::fmt::Write;
        let mut out = String::new();

        // Severity and message
        let severity_str = match diag.severity {
            crate::Severity::Error => "error",
            crate::Severity::Warning => "warning",
            crate::Severity::Info => "info",
        };
        writeln!(out, "{severity_str}: {}", diag.message).unwrap();

        // Location
        if diag.span != Span::dummy() {
            let loc = self.location(diag.file, diag.span);
            writeln!(out, " --> {loc}").unwrap();

            // Source context
            let line_text = self.line_text(diag.file, diag.span);
            let (line_num, col) = offset_to_line_col(
                &self.files[diag.file.0 as usize].line_starts,
                diag.span.start,
            );
            let line_num_width = line_num.to_string().len();
            let padding = " ".repeat(line_num_width);
            writeln!(out, "{padding} |").unwrap();
            writeln!(out, "{line_num} | {line_text}").unwrap();

            // Underline
            let underline_offset = col - 1;
            let underline_len = (diag.span.end - diag.span.start).max(1);
            let underline = "^".repeat(underline_len.min(line_text.len().saturating_sub(underline_offset)));
            writeln!(
                out,
                "{padding} | {}{underline}",
                " ".repeat(underline_offset)
            )
            .unwrap();
        }

        // Notes
        for note in &diag.notes {
            if note.span != Span::dummy() {
                let loc = self.location(note.file, note.span);
                writeln!(out, " = note: {} ({loc})", note.message).unwrap();
            } else {
                writeln!(out, " = note: {}", note.message).unwrap();
            }
        }

        out
    }

    /// Iterate all file IDs.
    pub fn file_ids(&self) -> impl Iterator<Item = FileId> {
        (0..self.files.len() as u32).map(FileId)
    }
}

fn compute_line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (i, ch) in source.bytes().enumerate() {
        if ch == b'\n' {
            starts.push(i + 1);
        }
    }
    starts
}

fn offset_to_line_col(line_starts: &[usize], offset: usize) -> (usize, usize) {
    let line = match line_starts.binary_search(&offset) {
        Ok(i) => i,
        Err(i) => i.saturating_sub(1),
    };
    let col = offset - line_starts[line] + 1;
    (line + 1, col) // 1-based
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_column_mapping() {
        let mut sm = SourceManager::new();
        let source = "line one\nline two\nline three\n";
        let id = sm.add_file(PathBuf::from("test.ql"), source.to_string());

        // Start of file
        let loc = sm.location(id, Span::new(0, 1));
        assert_eq!(loc.line, 1);
        assert_eq!(loc.column, 1);

        // Start of line 2 ("line two" starts at offset 9)
        let loc = sm.location(id, Span::new(9, 10));
        assert_eq!(loc.line, 2);
        assert_eq!(loc.column, 1);

        // "two" starts at offset 14
        let loc = sm.location(id, Span::new(14, 17));
        assert_eq!(loc.line, 2);
        assert_eq!(loc.column, 6);
    }

    #[test]
    fn span_text() {
        let mut sm = SourceManager::new();
        let source = "from int x\nwhere x > 0\nselect x";
        let id = sm.add_file(PathBuf::from("test.ql"), source.to_string());
        assert_eq!(sm.span_text(id, Span::new(0, 4)), "from");
        assert_eq!(sm.span_text(id, Span::new(9, 10)), "x");
    }

    #[test]
    fn duplicate_file() {
        let mut sm = SourceManager::new();
        let path = PathBuf::from("test.ql");
        let id1 = sm.add_file(path.clone(), "source".to_string());
        let id2 = sm.add_file(path, "source".to_string());
        assert_eq!(id1, id2);
    }
}
