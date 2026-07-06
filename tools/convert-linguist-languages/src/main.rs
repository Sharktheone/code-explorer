use anyhow::{Context, Result, bail};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct LinguistLanguage {
    #[serde(default)]
    r#type: Option<String>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    extensions: Vec<String>,
    #[serde(default)]
    filenames: Vec<String>,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    group: Option<String>,
}

#[derive(Debug, Serialize)]
struct AppLanguage {
    #[serde(skip_serializing_if = "Option::is_none")]
    r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    color: Option<String>,
    #[serde(default)]
    extensions: Vec<String>,
    #[serde(default)]
    filenames: Vec<String>,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    group: Option<String>,
}

fn main() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .context("failed to locate workspace root")?;
    let input_path = root.join("languages.yml");
    let output_path = root.join("assets/languages.generated.json");

    let input = fs::read_to_string(&input_path)
        .with_context(|| format!("failed to read {}", input_path.display()))?;
    let parsed: IndexMap<String, LinguistLanguage> =
        serde_yaml::from_str(&input).context("failed to parse Linguist languages.yml")?;

    let mut output = IndexMap::new();
    for (name, language) in parsed {
        if let Some(color) = &language.color {
            validate_color(color).with_context(|| format!("invalid color for {name}"))?;
        }

        output.insert(
            name,
            AppLanguage {
                r#type: language.r#type,
                color: language.color,
                extensions: language.extensions,
                filenames: language.filenames,
                aliases: language.aliases,
                group: language.group,
            },
        );
    }

    fs::create_dir_all(output_path.parent().unwrap()).context("failed to create assets dir")?;
    let json = serde_json::to_string_pretty(&output)?;
    fs::write(&output_path, format!("{json}\n"))
        .with_context(|| format!("failed to write {}", output_path.display()))?;

    Ok(())
}

fn validate_color(color: &str) -> Result<()> {
    let valid = color.len() == 7
        && color.starts_with('#')
        && color[1..]
            .chars()
            .all(|character| character.is_ascii_hexdigit());
    if valid {
        Ok(())
    } else {
        bail!("expected #RRGGBB, got {color}")
    }
}
