use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const FALLBACK_LANGUAGE_COLOR: &str = "#8b949e";
pub const OTHER_LANGUAGE_COLOR: &str = "#d0d7de";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LanguageDefinition {
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub filenames: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub group: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LanguageRegistry {
    definitions: IndexMap<String, LanguageDefinition>,
    extension_lookup: HashMap<String, String>,
    filename_lookup: HashMap<String, String>,
}

impl LanguageRegistry {
    pub fn from_json_str(input: &str) -> Result<Self> {
        let definitions: IndexMap<String, LanguageDefinition> = serde_json::from_str(input)
            .context("failed to parse generated language definitions")?;
        Ok(Self::from_definitions(definitions))
    }

    pub fn from_definitions(definitions: IndexMap<String, LanguageDefinition>) -> Self {
        let mut extension_lookup = HashMap::new();
        let mut filename_lookup = HashMap::new();

        for (language, definition) in &definitions {
            for extension in &definition.extensions {
                insert_language_mapping(
                    &mut extension_lookup,
                    &definitions,
                    extension.to_lowercase(),
                    language,
                );
            }
            for filename in &definition.filenames {
                insert_language_mapping(
                    &mut filename_lookup,
                    &definitions,
                    filename.to_lowercase(),
                    language,
                );
            }
        }

        Self {
            definitions,
            extension_lookup,
            filename_lookup,
        }
    }

    pub fn bundled() -> Result<Self> {
        Self::from_json_str(include_str!("../../../assets/languages.generated.json"))
    }

    pub fn color_for(&self, language: &str) -> Option<&str> {
        self.definitions
            .get(language)
            .and_then(|definition| definition.color.as_deref())
    }

    pub fn color_or_fallback(&self, language: &str) -> &str {
        self.color_for(language).unwrap_or(FALLBACK_LANGUAGE_COLOR)
    }

    pub fn language_for_path(&self, path: &std::path::Path) -> Option<String> {
        if let Some(filename) = path
            .file_name()
            .map(|value| value.to_string_lossy().to_lowercase())
        {
            if let Some(language) = self.filename_lookup.get(&filename) {
                return Some(language.clone());
            }
        }

        path.extension()
            .map(|extension| format!(".{}", extension.to_string_lossy()).to_lowercase())
            .and_then(|extension| self.extension_lookup.get(&extension).cloned())
    }
}

fn insert_language_mapping(
    lookup: &mut HashMap<String, String>,
    definitions: &IndexMap<String, LanguageDefinition>,
    key: String,
    candidate: &str,
) {
    let Some(existing) = lookup.get(&key) else {
        lookup.insert(key, candidate.to_string());
        return;
    };

    let existing_has_color = definitions
        .get(existing)
        .and_then(|definition| definition.color.as_ref())
        .is_some();
    let candidate_has_color = definitions
        .get(candidate)
        .and_then(|definition| definition.color.as_ref())
        .is_some();

    if !existing_has_color && candidate_has_color {
        lookup.insert(key, candidate.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_languages_include_rust_color() {
        let registry = LanguageRegistry::bundled().expect("bundled language json");
        assert_eq!(registry.color_for("Rust"), Some("#dea584"));
        assert_eq!(
            registry.language_for_path(std::path::Path::new("src/main.rs")),
            Some("Rust".to_string())
        );
    }
}
