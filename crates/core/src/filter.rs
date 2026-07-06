use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathMatcher {
    pattern: String,
}

impl PathMatcher {
    pub fn new(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
        }
    }

    pub fn is_match(&self, path: &Path) -> bool {
        let value = path.to_string_lossy();
        value.contains(&self.pattern)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanFilters {
    pub include_dirs: Vec<PathMatcher>,
    pub exclude_dirs: Vec<PathMatcher>,
    pub include_extensions: Vec<String>,
    pub exclude_extensions: Vec<String>,
    pub include_languages: Vec<String>,
    pub exclude_languages: Vec<String>,
    pub respect_gitignore: bool,
    pub include_hidden: bool,
    pub max_depth: Option<usize>,
}

impl Default for ScanFilters {
    fn default() -> Self {
        Self {
            include_dirs: Vec::new(),
            exclude_dirs: Vec::new(),
            include_extensions: Vec::new(),
            exclude_extensions: Vec::new(),
            include_languages: Vec::new(),
            exclude_languages: Vec::new(),
            respect_gitignore: true,
            include_hidden: false,
            max_depth: None,
        }
    }
}

impl ScanFilters {
    pub fn allows_dir(&self, path: &Path) -> bool {
        if !self.include_dirs.is_empty()
            && !self
                .include_dirs
                .iter()
                .any(|matcher| matcher.is_match(path))
        {
            return false;
        }
        !self
            .exclude_dirs
            .iter()
            .any(|matcher| matcher.is_match(path))
    }

    pub fn allows_extension(&self, extension: Option<&str>) -> bool {
        let normalized = extension
            .map(|value| format!(".{}", value.trim_start_matches('.')).to_lowercase())
            .unwrap_or_default();

        if !self.include_extensions.is_empty()
            && !self
                .include_extensions
                .iter()
                .any(|value| normalize_extension(value) == normalized)
        {
            return false;
        }

        !self
            .exclude_extensions
            .iter()
            .any(|value| normalize_extension(value) == normalized)
    }

    pub fn allows_language(&self, language: &str) -> bool {
        if !self.include_languages.is_empty()
            && !self
                .include_languages
                .iter()
                .any(|value| value.eq_ignore_ascii_case(language))
        {
            return false;
        }

        !self
            .exclude_languages
            .iter()
            .any(|value| value.eq_ignore_ascii_case(language))
    }
}

fn normalize_extension(value: &str) -> String {
    format!(".{}", value.trim().trim_start_matches('.')).to_lowercase()
}
