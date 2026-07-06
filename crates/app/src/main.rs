use anyhow::{Context, Result};
use code_explorer_core::language::{FALLBACK_LANGUAGE_COLOR, OTHER_LANGUAGE_COLOR};
use code_explorer_core::scan::{flatten_children_for_list, language_summary};
use code_explorer_core::treemap;
use code_explorer_core::visualization::attention_for;
use code_explorer_core::{
    LanguageBreakdown, LanguageRegistry, PathMatcher, ScanFilters, SizeMetric, scan_directory,
};
use slint::{Brush, Color, ModelRc, VecModel};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

slint::include_modules!();

#[derive(Clone)]
struct AppState {
    registry: Arc<LanguageRegistry>,
    root_path: Option<PathBuf>,
    current_path: Option<PathBuf>,
    root_node: Option<code_explorer_core::DirectoryNode>,
    metric: SizeMetric,
}

fn main() -> Result<()> {
    let registry = Arc::new(LanguageRegistry::bundled()?);
    let app = AppWindow::new()?;
    let state = Arc::new(Mutex::new(AppState {
        registry,
        root_path: None,
        current_path: None,
        root_node: None,
        metric: SizeMetric::TotalLoc,
    }));

    wire_callbacks(&app, state.clone());

    if let Some(argument) = std::env::args().nth(1) {
        let path = PathBuf::from(argument);
        if path.is_dir() {
            start_scan(&app, state.clone(), path);
        } else {
            app.set_status_text(format!("Not a directory: {}", path.display()).into());
        }
    }

    app.run()?;
    Ok(())
}

fn wire_callbacks(app: &AppWindow, state: Arc<Mutex<AppState>>) {
    let weak = app.as_weak();
    let state_for_open = state.clone();
    app.on_open_folder(move || {
        if let Some(app) = weak.upgrade() {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Open folder")
                .pick_folder()
            {
                start_scan(&app, state_for_open.clone(), path);
            }
        }
    });

    let weak = app.as_weak();
    let state_for_clone = state.clone();
    app.on_clone_repository(move |url| {
        if let Some(app) = weak.upgrade() {
            app.set_status_text("Cloning repository...".into());
            match code_explorer_git::clone_public_https(url.as_str(), false) {
                Ok(path) => {
                    start_scan(&app, state_for_clone.clone(), path);
                }
                Err(error) => app.set_status_text(format!("Clone failed: {error:#}").into()),
            }
        }
    });

    let weak = app.as_weak();
    let state_for_rescan = state.clone();
    app.on_rescan(move || {
        if let Some(app) = weak.upgrade() {
            let path = state_for_rescan.lock().unwrap().root_path.clone();
            if let Some(path) = path {
                start_scan(&app, state_for_rescan.clone(), path);
            }
        }
    });

    let weak = app.as_weak();
    let state_for_metric = state.clone();
    app.on_metric_changed(move |index| {
        if let Some(app) = weak.upgrade() {
            state_for_metric.lock().unwrap().metric = metric_from_index(index);
            rescan_from_state(&app, state_for_metric.clone());
        }
    });

    let weak = app.as_weak();
    let state_for_render = state.clone();
    app.on_view_changed(move |_| {
        if let Some(app) = weak.upgrade() {
            render(&app, &state_for_render.lock().unwrap());
        }
    });

    let weak = app.as_weak();
    let state_for_render = state.clone();
    app.on_row_bars_changed(move |_| {
        if let Some(app) = weak.upgrade() {
            render(&app, &state_for_render.lock().unwrap());
        }
    });

    let weak = app.as_weak();
    let state_for_filters = state.clone();
    app.on_filters_changed(move || {
        if let Some(app) = weak.upgrade() {
            rescan_from_state(&app, state_for_filters.clone());
        }
    });

    let weak = app.as_weak();
    let state_for_limit = state.clone();
    app.on_subdir_limit_changed(move |_| {
        if let Some(app) = weak.upgrade() {
            render(&app, &state_for_limit.lock().unwrap());
        }
    });

    let weak = app.as_weak();
    let state_for_enter = state.clone();
    app.on_enter_directory(move |path| {
        if path.is_empty() {
            return;
        }
        if let Some(app) = weak.upgrade() {
            let mut state = state_for_enter.lock().unwrap();
            state.current_path = Some(PathBuf::from(path.as_str()));
            render(&app, &state);
        }
    });

    let weak = app.as_weak();
    app.on_go_up(move || {
        if let Some(app) = weak.upgrade() {
            let mut state = state.lock().unwrap();
            if let (Some(root), Some(current)) = (&state.root_path, &state.current_path) {
                if current != root {
                    state.current_path = current.parent().map(PathBuf::from);
                }
            }
            render(&app, &state);
        }
    });
}

fn start_scan(app: &AppWindow, state: Arc<Mutex<AppState>>, path: PathBuf) {
    app.set_status_text("Indexing directory...".into());
    app.set_current_source(path.to_string_lossy().into_owned().into());

    let filters = filters_from_app(app);
    let (metric, registry) = {
        let state = state.lock().unwrap();
        (state.metric, state.registry.clone())
    };
    let weak = app.as_weak();

    thread::spawn(move || {
        let canonical = path.canonicalize().context("failed to canonicalize root");
        let result = canonical.and_then(|canonical| {
            scan_directory(&canonical, &filters, metric, &registry).map(|root| (canonical, root))
        });

        let _ = slint::invoke_from_event_loop(move || {
            let Some(app) = weak.upgrade() else {
                return;
            };

            match result {
                Ok((canonical, root)) => {
                    let mut state = state.lock().unwrap();
                    state.root_path = Some(canonical);
                    state.current_path = state.root_path.clone();
                    state.root_node = Some(root);
                    render(&app, &state);
                }
                Err(error) => {
                    app.set_status_text(format!("Scan failed: {error:#}").into());
                }
            }
        });
    });
}

fn rescan_from_state(app: &AppWindow, state: Arc<Mutex<AppState>>) {
    let path = state.lock().unwrap().root_path.clone();
    if let Some(path) = path {
        start_scan(app, state, path);
    }
}

fn render(app: &AppWindow, state: &AppState) {
    let Some(root_node) = &state.root_node else {
        return;
    };
    let current_path = state.current_path.as_deref().unwrap_or(&root_node.path);
    let current = find_node(root_node, current_path).unwrap_or(root_node);
    let metric = state.metric;

    app.set_breadcrumb(current.path.to_string_lossy().into_owned().into());
    app.set_breadcrumb_segments(model_from_vec(breadcrumb_segments(
        state.root_path.as_deref().unwrap_or(&root_node.path),
        &current.path,
    )));
    app.set_current_dir_name(current.name.clone().into());
    app.set_current_total_loc(format_number(current.totals.total_loc).into());
    app.set_current_code_loc(format_number(current.totals.code_loc).into());
    app.set_current_comment_loc(format_number(current.totals.comment_loc).into());
    app.set_current_blank_loc(format_number(current.totals.blank_loc).into());
    app.set_current_files(format_number(current.totals.files).into());
    app.set_current_language(
        current
            .prominent_language
            .as_deref()
            .map(|language| format!("Prominent language: {language}"))
            .unwrap_or_else(|| "No prominent language".to_string())
            .into(),
    );
    app.set_language_segments(model_from_vec(
        language_summary(current, metric, 2.0)
            .into_iter()
            .map(language_segment_from_breakdown)
            .collect(),
    ));

    let rows = directory_rows(current, metric);
    app.set_directory_empty(rows.is_empty());
    app.set_directory_rows(model_from_vec(rows));

    let rects = treemap::layout_children(
        current,
        metric,
        app.get_subdir_limit() as usize,
        OTHER_LANGUAGE_COLOR,
    )
    .into_iter()
    .map(|rect| BoxRect {
        path: rect.path.into(),
        label: rect.label.into(),
        color: brush_from_hex(&rect.color),
        value: format_number(rect.value).into(),
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
        tooltip: rect.tooltip.into(),
    })
    .collect();
    app.set_box_rects(model_from_vec(rects));

    app.set_status_text(
        format!(
            "{} indexed: {} files, {} total LOC, {} code LOC",
            current.name,
            format_number(current.totals.files),
            format_number(current.totals.total_loc),
            format_number(current.totals.code_loc)
        )
        .into(),
    );
}

fn directory_rows(
    node: &code_explorer_core::DirectoryNode,
    metric: SizeMetric,
) -> Vec<DirectoryRow> {
    let children = flatten_children_for_list(node, metric);
    let parent_total = metric.value(&node.totals).max(1) as f32;
    let count = children.len().max(1) as f32;

    let mut rows: Vec<_> = children
        .into_iter()
        .enumerate()
        .map(|(index, (_, child))| {
            let (display_child, display_name) = collapse_directory_chain(&child);
            let metric_value = metric.value(&display_child.totals);
            let percent = metric_value as f32 / parent_total * 100.0;
            let rank_percent = (index as f32 + 1.0) / count;
            let attention = attention_for(&display_child.path, rank_percent, metric_value);
            let prominent = display_child
                .prominent_language
                .clone()
                .unwrap_or_else(|| "Other".to_string());
            let language_color = display_child
                .languages
                .first()
                .and_then(|language| language.color.clone())
                .unwrap_or_else(|| FALLBACK_LANGUAGE_COLOR.to_string());

            DirectoryRow {
                path: display_child.path.to_string_lossy().into_owned().into(),
                name: display_name.into(),
                kind: "dir".into(),
                is_directory: true,
                metric: format_number(metric_value).into(),
                percent: format!("{percent:.1}%").into(),
                percent_value: percent,
                files: format_number(display_child.totals.files).into(),
                language: prominent.into(),
                language_color: brush_from_hex(&language_color),
                attention_color: brush_from_hex(attention_color(attention)),
                tooltip: directory_tooltip(display_child, metric).into(),
                row_bar: model_from_vec(
                    display_child
                        .languages
                        .iter()
                        .take(8)
                        .cloned()
                        .map(language_segment_from_breakdown)
                        .collect(),
                ),
            }
        })
        .collect();

    let file_parent_total = metric.value(&node.totals).max(1) as f32;
    rows.extend(node.files.iter().map(|file| {
        let metric_value = metric.value(&file.totals);
        let percent = metric_value as f32 / file_parent_total * 100.0;
        let color = file
            .color
            .clone()
            .unwrap_or_else(|| FALLBACK_LANGUAGE_COLOR.to_string());
        let language = LanguageBreakdown {
            language: file.language.clone(),
            color: Some(color.clone()),
            totals: file.totals.clone(),
            percent: 100.0,
        };

        DirectoryRow {
            path: file.path.to_string_lossy().into_owned().into(),
            name: file.name.clone().into(),
            kind: "file".into(),
            is_directory: false,
            metric: format_number(metric_value).into(),
            percent: format!("{percent:.1}%").into(),
            percent_value: percent,
            files: "1".into(),
            language: file.language.clone().into(),
            language_color: brush_from_hex(&color),
            attention_color: brush_from_hex("#57606a"),
            tooltip: file_tooltip(file, metric).into(),
            row_bar: model_from_vec(vec![language_segment_from_breakdown(language)]),
        }
    }));

    rows
}

fn breadcrumb_segments(
    root: &std::path::Path,
    current: &std::path::Path,
) -> Vec<BreadcrumbSegment> {
    let mut segments = Vec::new();
    segments.push(BreadcrumbSegment {
        label: root
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.to_string_lossy().into_owned())
            .into(),
        path: root.to_string_lossy().into_owned().into(),
    });

    if let Ok(relative) = current.strip_prefix(root) {
        let mut path = root.to_path_buf();
        for component in relative.components() {
            path.push(component.as_os_str());
            segments.push(BreadcrumbSegment {
                label: component.as_os_str().to_string_lossy().into_owned().into(),
                path: path.to_string_lossy().into_owned().into(),
            });
        }
    }

    segments
}

fn language_segment_from_breakdown(language: LanguageBreakdown) -> LanguageSegment {
    let color = language
        .color
        .clone()
        .unwrap_or_else(|| FALLBACK_LANGUAGE_COLOR.to_string());
    LanguageSegment {
        name: language.language.clone().into(),
        color: brush_from_hex(&color),
        percent: language.percent,
        label: format!("{} {:.1}%", language.language, language.percent).into(),
        tooltip: language_tooltip(&language).into(),
    }
}

fn collapse_directory_chain(
    node: &code_explorer_core::DirectoryNode,
) -> (&code_explorer_core::DirectoryNode, String) {
    let mut current = node;
    let mut parts = vec![node.name.clone()];

    while current.children.len() == 1 && current.totals == current.children[0].totals {
        current = &current.children[0];
        parts.push(current.name.clone());
    }

    (current, parts.join("/"))
}

fn directory_tooltip(node: &code_explorer_core::DirectoryNode, metric: SizeMetric) -> String {
    let metric_label = metric.label();
    let metric_value = format_number(metric.value(&node.totals));
    let language = node
        .prominent_language
        .clone()
        .unwrap_or_else(|| "Other".to_string());
    format!(
        "{}\n{}\n{}: {}\nFiles: {}\nTotal LOC: {}\nCode: {}\nComments: {}\nBlanks: {}\nProminent language: {}",
        node.name,
        node.path.display(),
        metric_label,
        metric_value,
        format_number(node.totals.files),
        format_number(node.totals.total_loc),
        format_number(node.totals.code_loc),
        format_number(node.totals.comment_loc),
        format_number(node.totals.blank_loc),
        language
    )
}

fn file_tooltip(file: &code_explorer_core::FileNode, metric: SizeMetric) -> String {
    let metric_label = metric.label();
    let metric_value = format_number(metric.value(&file.totals));
    format!(
        "{}\n{}\n{}: {}\nFiles: 1\nTotal LOC: {}\nCode: {}\nComments: {}\nBlanks: {}\nLanguage: {}",
        file.name,
        file.path.display(),
        metric_label,
        metric_value,
        format_number(file.totals.total_loc),
        format_number(file.totals.code_loc),
        format_number(file.totals.comment_loc),
        format_number(file.totals.blank_loc),
        file.language
    )
}

fn language_tooltip(language: &LanguageBreakdown) -> String {
    format!(
        "{} {:.1}%\nFiles: {}\nTotal LOC: {}\nCode: {}\nComments: {}\nBlanks: {}",
        language.language,
        language.percent,
        format_number(language.totals.files),
        format_number(language.totals.total_loc),
        format_number(language.totals.code_loc),
        format_number(language.totals.comment_loc),
        format_number(language.totals.blank_loc)
    )
}

fn find_node<'a>(
    node: &'a code_explorer_core::DirectoryNode,
    path: &std::path::Path,
) -> Option<&'a code_explorer_core::DirectoryNode> {
    if node.path == path {
        return Some(node);
    }
    node.children
        .iter()
        .find_map(|child| find_node(child, path))
}

fn filters_from_app(app: &AppWindow) -> ScanFilters {
    ScanFilters {
        include_extensions: split_filter_list(app.get_include_extensions().as_str()),
        exclude_extensions: split_filter_list(app.get_exclude_extensions().as_str()),
        include_languages: split_filter_list(app.get_include_languages().as_str()),
        exclude_languages: split_filter_list(app.get_exclude_languages().as_str()),
        exclude_dirs: split_filter_list(app.get_exclude_dirs().as_str())
            .into_iter()
            .map(PathMatcher::new)
            .collect(),
        respect_gitignore: app.get_respect_gitignore(),
        include_hidden: app.get_include_hidden(),
        max_depth: app.get_max_depth().as_str().trim().parse::<usize>().ok(),
        ..ScanFilters::default()
    }
}

fn split_filter_list(input: &str) -> Vec<String> {
    input
        .split([',', ' ', ';'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn metric_from_index(index: i32) -> SizeMetric {
    match index {
        1 => SizeMetric::CodeLoc,
        2 => SizeMetric::FileCount,
        _ => SizeMetric::TotalLoc,
    }
}

fn attention_color(attention: code_explorer_core::visualization::AttentionLevel) -> &'static str {
    match attention {
        code_explorer_core::visualization::AttentionLevel::TopTen => "#cf222e",
        code_explorer_core::visualization::AttentionLevel::TopQuarter => "#bc4c00",
        code_explorer_core::visualization::AttentionLevel::TopHalf => "#bf8700",
        code_explorer_core::visualization::AttentionLevel::Small => "#0969da",
        code_explorer_core::visualization::AttentionLevel::Empty => "#8c959f",
        code_explorer_core::visualization::AttentionLevel::Vendor => "#8250df",
    }
}

fn format_number(value: u64) -> String {
    let text = value.to_string();
    let mut output = String::new();
    for (index, character) in text.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            output.push(',');
        }
        output.push(character);
    }
    output.chars().rev().collect()
}

fn model_from_vec<T: Clone + 'static>(values: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(values))
}

fn brush_from_hex(value: &str) -> Brush {
    let trimmed = value.trim().trim_start_matches('#');
    if trimmed.len() != 6 {
        return Brush::from(Color::from_rgb_u8(0x8b, 0x94, 0x9e));
    }

    let Ok(rgb) = u32::from_str_radix(trimmed, 16) else {
        return Brush::from(Color::from_rgb_u8(0x8b, 0x94, 0x9e));
    };

    Brush::from(Color::from_rgb_u8(
        ((rgb >> 16) & 0xff) as u8,
        ((rgb >> 8) & 0xff) as u8,
        (rgb & 0xff) as u8,
    ))
}
