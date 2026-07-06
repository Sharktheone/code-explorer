use crate::filter::ScanFilters;
use crate::language::LanguageRegistry;
use crate::tree::{CodeTotals, DirectoryNode, FileNode, LanguageBreakdown, SizeMetric};
use anyhow::{Context, Result, bail};
use ignore::WalkBuilder;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScanSource {
    LocalFolder(PathBuf),
    PublicHttpsRepo { url: String, cache_path: PathBuf },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanRequest {
    pub source: ScanSource,
    pub root_path: PathBuf,
    pub filters: ScanFilters,
    pub metric: SizeMetric,
}

#[derive(Debug, Clone)]
struct FileStat {
    path: PathBuf,
    language: String,
    totals: CodeTotals,
}

#[derive(Debug, Default)]
struct MutableNode {
    totals: CodeTotals,
    languages: HashMap<String, CodeTotals>,
}

pub fn scan_directory(
    root_path: impl AsRef<Path>,
    filters: &ScanFilters,
    metric: SizeMetric,
    registry: &LanguageRegistry,
) -> Result<DirectoryNode> {
    let root_path = root_path
        .as_ref()
        .canonicalize()
        .context("failed to canonicalize scan root")?;
    if !root_path.is_dir() {
        bail!("scan root is not a directory: {}", root_path.display());
    }

    let mut walker = WalkBuilder::new(&root_path);
    walker
        .hidden(!filters.include_hidden)
        .git_ignore(filters.respect_gitignore)
        .git_global(filters.respect_gitignore)
        .git_exclude(filters.respect_gitignore);
    if let Some(max_depth) = filters.max_depth {
        walker.max_depth(Some(max_depth));
    }

    let mut file_stats = Vec::new();
    for entry in walker.build() {
        let entry = entry.context("failed to read directory entry")?;
        let path = entry.path();
        if path == root_path {
            continue;
        }

        if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            if !filters.allows_dir(path) {
                continue;
            }
            continue;
        }

        if !entry
            .file_type()
            .map(|kind| kind.is_file())
            .unwrap_or(false)
        {
            continue;
        }

        if !filters.allows_extension(path.extension().and_then(|value| value.to_str())) {
            continue;
        }

        let language = registry
            .language_for_path(path)
            .unwrap_or_else(|| "Other".to_string());
        if !filters.allows_language(&language) {
            continue;
        }

        if let Ok(totals) = count_file(path) {
            if totals.total_loc > 0 || totals.files > 0 {
                file_stats.push(FileStat {
                    path: path.to_path_buf(),
                    language,
                    totals,
                });
            }
        }
    }

    Ok(build_tree(&root_path, file_stats, metric, registry))
}

fn count_file(path: &Path) -> Result<CodeTotals> {
    let mut totals = CodeTotals {
        files: 1,
        ..CodeTotals::default()
    };

    let config = tokei::Config::default();
    if let Some(language_type) = tokei::LanguageType::from_path(path, &config) {
        match language_type.parse(path.to_path_buf(), &config) {
            Ok(report) => {
                let stats = report.stats.summarise();
                totals.blank_loc = stats.blanks as u64;
                totals.code_loc = stats.code as u64;
                totals.comment_loc = stats.comments as u64;
                totals.total_loc = stats.lines() as u64;
                return Ok(totals);
            }
            Err((error, _)) => return Err(error.into()),
        }
    }

    let content = fs::read_to_string(path)?;
    for line in content.lines() {
        let trimmed = line.trim();
        totals.total_loc += 1;
        if trimmed.is_empty() {
            totals.blank_loc += 1;
        } else if trimmed.starts_with("//")
            || trimmed.starts_with('#')
            || trimmed.starts_with("--")
            || trimmed.starts_with("/*")
            || trimmed.starts_with('*')
        {
            totals.comment_loc += 1;
        } else {
            totals.code_loc += 1;
        }
    }

    Ok(totals)
}

fn build_tree(
    root: &Path,
    files: Vec<FileStat>,
    metric: SizeMetric,
    registry: &LanguageRegistry,
) -> DirectoryNode {
    let mut nodes: BTreeMap<PathBuf, MutableNode> = BTreeMap::new();
    nodes.entry(root.to_path_buf()).or_default();

    for file in &files {
        let mut current = file.path.parent();
        while let Some(dir) = current {
            if !dir.starts_with(root) {
                break;
            }
            let node = nodes.entry(dir.to_path_buf()).or_default();
            node.totals.add_assign(&file.totals);
            node.languages
                .entry(file.language.clone())
                .or_default()
                .add_assign(&file.totals);

            if dir == root {
                break;
            }
            current = dir.parent();
        }
    }

    materialize_node(root, root, &nodes, &files, metric, registry)
}

fn materialize_node(
    path: &Path,
    root: &Path,
    nodes: &BTreeMap<PathBuf, MutableNode>,
    files: &[FileStat],
    metric: SizeMetric,
    registry: &LanguageRegistry,
) -> DirectoryNode {
    let mutable = nodes.get(path);
    let totals = mutable.map(|node| node.totals.clone()).unwrap_or_default();
    let mut languages = mutable
        .map(|node| language_breakdowns(&node.languages, metric, &totals, registry))
        .unwrap_or_default();

    languages.sort_by_key(|language| std::cmp::Reverse(metric.value(&language.totals)));
    let prominent_language = languages.first().map(|language| language.language.clone());

    let mut children: Vec<_> = nodes
        .keys()
        .filter(|candidate| candidate.parent() == Some(path))
        .map(|child_path| materialize_node(child_path, root, nodes, files, metric, registry))
        .collect();
    children.sort_by_key(|child| std::cmp::Reverse(metric.value(&child.totals)));

    let mut direct_files: Vec<_> = files
        .iter()
        .filter(|file| file.path.parent() == Some(path))
        .map(|file| FileNode {
            path: file.path.clone(),
            name: file
                .path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| file.path.to_string_lossy().into_owned()),
            totals: file.totals.clone(),
            language: file.language.clone(),
            color: registry.color_for(&file.language).map(str::to_string),
        })
        .collect();
    direct_files.sort_by_key(|file| std::cmp::Reverse(metric.value(&file.totals)));

    let mut node = DirectoryNode::new(path.to_path_buf(), root);
    node.totals = totals;
    node.languages = languages;
    node.prominent_language = prominent_language;
    node.files = direct_files;
    node.children = children;
    node
}

fn language_breakdowns(
    languages: &HashMap<String, CodeTotals>,
    metric: SizeMetric,
    directory_totals: &CodeTotals,
    registry: &LanguageRegistry,
) -> Vec<LanguageBreakdown> {
    let total = metric.value(directory_totals).max(1) as f32;
    let mut output: Vec<_> = languages
        .iter()
        .map(|(language, totals)| {
            let value = metric.value(totals) as f32;
            LanguageBreakdown {
                language: language.clone(),
                color: registry.color_for(language).map(str::to_string),
                totals: totals.clone(),
                percent: value / total * 100.0,
            }
        })
        .collect();
    output.sort_by_key(|language| std::cmp::Reverse(metric.value(&language.totals)));
    output
}

pub fn language_summary(
    node: &DirectoryNode,
    metric: SizeMetric,
    small_threshold_percent: f32,
) -> Vec<LanguageBreakdown> {
    let mut summary = Vec::new();
    let mut other = CodeTotals::default();

    for language in &node.languages {
        if language.percent < small_threshold_percent {
            other.add_assign(&language.totals);
        } else {
            summary.push(language.clone());
        }
    }

    if metric.value(&other) > 0 {
        let total = metric.value(&node.totals).max(1) as f32;
        summary.push(LanguageBreakdown {
            language: "Other".to_string(),
            color: Some(crate::language::OTHER_LANGUAGE_COLOR.to_string()),
            percent: metric.value(&other) as f32 / total * 100.0,
            totals: other,
        });
    }

    summary
}

pub fn flatten_children_for_list(
    node: &DirectoryNode,
    metric: SizeMetric,
) -> IndexMap<PathBuf, DirectoryNode> {
    let mut children = node.children.clone();
    children.sort_by_key(|child| std::cmp::Reverse(metric.value(&child.totals)));
    children
        .into_iter()
        .map(|child| (child.path.clone(), child))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn scans_nested_directory_totals() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("src")).unwrap();
        let mut file = fs::File::create(temp.path().join("src/main.rs")).unwrap();
        writeln!(file, "fn main() {{").unwrap();
        writeln!(file, "    println!(\"hi\");").unwrap();
        writeln!(file, "}}").unwrap();

        let registry = LanguageRegistry::bundled().unwrap();
        let root = scan_directory(
            temp.path(),
            &ScanFilters::default(),
            SizeMetric::TotalLoc,
            &registry,
        )
        .unwrap();

        assert_eq!(root.totals.files, 1);
        assert_eq!(root.totals.total_loc, 3);
        assert_eq!(root.languages[0].language, "Rust");
        assert_eq!(root.children[0].name, "src");
    }

    #[test]
    fn skips_unreadable_or_binary_files_without_failing_scan() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("main.rs"), "fn main() {}\n").unwrap();
        fs::write(temp.path().join("blob.bin"), [0, 159, 146, 150]).unwrap();

        let registry = LanguageRegistry::bundled().unwrap();
        let root = scan_directory(
            temp.path(),
            &ScanFilters::default(),
            SizeMetric::TotalLoc,
            &registry,
        )
        .unwrap();

        assert_eq!(root.totals.files, 1);
        assert_eq!(root.languages[0].language, "Rust");
    }
}
