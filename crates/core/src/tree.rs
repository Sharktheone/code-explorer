use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SizeMetric {
    TotalLoc,
    CodeLoc,
    FileCount,
}

impl SizeMetric {
    pub fn value(self, totals: &CodeTotals) -> u64 {
        match self {
            SizeMetric::TotalLoc => totals.total_loc,
            SizeMetric::CodeLoc => totals.code_loc,
            SizeMetric::FileCount => totals.files,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SizeMetric::TotalLoc => "Total LOC",
            SizeMetric::CodeLoc => "Code LOC",
            SizeMetric::FileCount => "Files",
        }
    }
}

impl Default for SizeMetric {
    fn default() -> Self {
        Self::TotalLoc
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeTotals {
    pub files: u64,
    pub code_loc: u64,
    pub comment_loc: u64,
    pub blank_loc: u64,
    pub total_loc: u64,
}

impl CodeTotals {
    pub fn add_assign(&mut self, other: &Self) {
        self.files += other.files;
        self.code_loc += other.code_loc;
        self.comment_loc += other.comment_loc;
        self.blank_loc += other.blank_loc;
        self.total_loc += other.total_loc;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageBreakdown {
    pub language: String,
    pub color: Option<String>,
    pub totals: CodeTotals,
    pub percent: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileNode {
    pub path: PathBuf,
    pub name: String,
    pub totals: CodeTotals,
    pub language: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryNode {
    pub path: PathBuf,
    pub name: String,
    pub depth: usize,
    pub totals: CodeTotals,
    pub languages: Vec<LanguageBreakdown>,
    pub prominent_language: Option<String>,
    pub files: Vec<FileNode>,
    pub children: Vec<DirectoryNode>,
}

impl DirectoryNode {
    pub fn new(path: PathBuf, root: &std::path::Path) -> Self {
        let depth = path
            .strip_prefix(root)
            .ok()
            .map(|p| p.components().count())
            .unwrap_or_default();
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());

        Self {
            path,
            name,
            depth,
            totals: CodeTotals::default(),
            languages: Vec::new(),
            prominent_language: None,
            files: Vec::new(),
            children: Vec::new(),
        }
    }
}
