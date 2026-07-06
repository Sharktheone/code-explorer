use crate::tree::{DirectoryNode, SizeMetric};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreemapRect {
    pub id: String,
    pub label: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub value: u64,
    pub color: String,
    pub path: String,
    pub tooltip: String,
}

pub fn layout_children(
    node: &DirectoryNode,
    metric: SizeMetric,
    max_items: usize,
    other_color: &str,
) -> Vec<TreemapRect> {
    let mut children = node.children.clone();
    children.sort_by_key(|child| std::cmp::Reverse(metric.value(&child.totals)));

    let max_items = max_items.max(1);
    let mut visible: Vec<_> = children.into_iter().take(max_items).collect();
    let hidden_total: u64 = node
        .children
        .iter()
        .skip(visible.len())
        .map(|child| metric.value(&child.totals))
        .sum();

    let mut items: Vec<(String, String, u64, String, String, String)> = visible
        .drain(..)
        .map(|child| {
            let (display_child, display_name) = collapse_directory_chain(&child);
            let value = metric.value(&display_child.totals);
            let base_color = display_child
                .languages
                .first()
                .and_then(|language| language.color.clone())
                .unwrap_or_else(|| "#8b949e".to_string());
            let path = display_child.path.to_string_lossy().into_owned();
            (
                path.clone(),
                display_name,
                value,
                varied_color(&base_color, &path),
                path.clone(),
                format!(
                    "{}\n{}\nValue: {}\nFiles: {}\nTotal LOC: {}\nCode: {}\nComments: {}\nBlanks: {}",
                    display_child.name,
                    display_child.path.display(),
                    value,
                    display_child.totals.files,
                    display_child.totals.total_loc,
                    display_child.totals.code_loc,
                    display_child.totals.comment_loc,
                    display_child.totals.blank_loc
                ),
            )
        })
        .collect();

    if hidden_total > 0 {
        items.push((
            "other".to_string(),
            "Other".to_string(),
            hidden_total,
            other_color.to_string(),
            String::new(),
            format!("Other\nValue: {hidden_total}"),
        ));
    }

    let mut rects = Vec::with_capacity(items.len());
    balanced_layout(&items, Rect::new(0.0, 0.0, 1.0, 1.0), &mut rects);
    rects
}

#[derive(Debug, Clone, Copy)]
struct Rect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl Rect {
    fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

fn balanced_layout(
    items: &[(String, String, u64, String, String, String)],
    rect: Rect,
    output: &mut Vec<TreemapRect>,
) {
    let total: u64 = items.iter().map(|(_, _, value, _, _, _)| *value).sum();
    if items.is_empty() || total == 0 || rect.width <= 0.0 || rect.height <= 0.0 {
        return;
    }

    if items.len() == 1 {
        let (id, label, value, color, path, tooltip) = &items[0];
        output.push(TreemapRect {
            id: id.clone(),
            label: label.clone(),
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
            value: *value,
            color: color.clone(),
            path: path.clone(),
            tooltip: tooltip.clone(),
        });
        return;
    }

    let split = split_near_half(items, total);
    let first_sum: u64 = items[..split]
        .iter()
        .map(|(_, _, value, _, _, _)| *value)
        .sum();
    let fraction = first_sum as f32 / total as f32;

    if rect.width >= rect.height {
        let first_width = rect.width * fraction;
        balanced_layout(
            &items[..split],
            Rect::new(rect.x, rect.y, first_width, rect.height),
            output,
        );
        balanced_layout(
            &items[split..],
            Rect::new(
                rect.x + first_width,
                rect.y,
                rect.width - first_width,
                rect.height,
            ),
            output,
        );
    } else {
        let first_height = rect.height * fraction;
        balanced_layout(
            &items[..split],
            Rect::new(rect.x, rect.y, rect.width, first_height),
            output,
        );
        balanced_layout(
            &items[split..],
            Rect::new(
                rect.x,
                rect.y + first_height,
                rect.width,
                rect.height - first_height,
            ),
            output,
        );
    }
}

fn split_near_half(items: &[(String, String, u64, String, String, String)], total: u64) -> usize {
    let mut sum = 0;
    let mut best_index = 1;
    let mut best_delta = total;

    for (index, (_, _, value, _, _, _)) in items.iter().enumerate().take(items.len() - 1) {
        sum += *value;
        let target_delta = sum.abs_diff(total - sum);
        if target_delta <= best_delta {
            best_delta = target_delta;
            best_index = index + 1;
        }
    }

    best_index.clamp(1, items.len() - 1)
}

fn collapse_directory_chain(node: &DirectoryNode) -> (&DirectoryNode, String) {
    let mut current = node;
    let mut parts = vec![node.name.clone()];

    while current.children.len() == 1 && current.totals == current.children[0].totals {
        current = &current.children[0];
        parts.push(current.name.clone());
    }

    (current, parts.join("/"))
}

fn varied_color(base: &str, key: &str) -> String {
    let Some((mut r, mut g, mut b)) = parse_hex_color(base) else {
        return base.to_string();
    };

    let hash = key
        .bytes()
        .fold(0u32, |acc, byte| acc.wrapping_mul(16777619) ^ byte as u32);
    let factor = 0.78 + (hash % 45) as f32 / 100.0;
    let channel_shift = ((hash >> 8) % 19) as i16 - 9;

    r = adjust_channel(r, factor, channel_shift);
    g = adjust_channel(g, factor, -channel_shift / 2);
    b = adjust_channel(b, factor, channel_shift / 2);

    format!("#{r:02x}{g:02x}{b:02x}")
}

fn parse_hex_color(value: &str) -> Option<(u8, u8, u8)> {
    let trimmed = value.trim().trim_start_matches('#');
    if trimmed.len() != 6 {
        return None;
    }
    let rgb = u32::from_str_radix(trimmed, 16).ok()?;
    Some((
        ((rgb >> 16) & 0xff) as u8,
        ((rgb >> 8) & 0xff) as u8,
        (rgb & 0xff) as u8,
    ))
}

fn adjust_channel(value: u8, factor: f32, shift: i16) -> u8 {
    ((value as f32 * factor).round() as i16 + shift).clamp(28, 235) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{CodeTotals, DirectoryNode};
    use std::path::PathBuf;

    #[test]
    fn treemap_rectangles_stay_in_bounds() {
        let mut root = DirectoryNode::new(PathBuf::from("/repo"), std::path::Path::new("/repo"));
        for index in 0..3 {
            let mut child = DirectoryNode::new(
                PathBuf::from(format!("/repo/dir{index}")),
                std::path::Path::new("/repo"),
            );
            child.totals = CodeTotals {
                total_loc: 10 + index,
                ..CodeTotals::default()
            };
            root.children.push(child);
        }

        let rects = layout_children(&root, SizeMetric::TotalLoc, 2, "#d0d7de");
        assert_eq!(rects.len(), 3);
        for rect in rects {
            assert!(rect.x >= 0.0);
            assert!(rect.y >= 0.0);
            assert!(rect.x + rect.width <= 1.0001);
            assert!(rect.y + rect.height <= 1.0001);
        }
    }

    #[test]
    fn treemap_avoids_barcode_for_many_items() {
        let mut root = DirectoryNode::new(PathBuf::from("/repo"), std::path::Path::new("/repo"));
        for index in 0..30 {
            let mut child = DirectoryNode::new(
                PathBuf::from(format!("/repo/dir{index}")),
                std::path::Path::new("/repo"),
            );
            child.totals = CodeTotals {
                total_loc: 100 - index,
                ..CodeTotals::default()
            };
            root.children.push(child);
        }

        let rects = layout_children(&root, SizeMetric::TotalLoc, 30, "#d0d7de");
        let thin_rects = rects
            .iter()
            .filter(|rect| rect.width < 0.01 || rect.height < 0.01)
            .count();
        assert!(thin_rects < rects.len() / 4);
    }
}
