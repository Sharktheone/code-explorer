use crate::tree::{CodeTotals, LanguageBreakdown};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisualizationMode {
    DirectoryList,
    BoxesGrid,
    Sunburst,
    Icicle,
    PackedBubbles,
    LanguageHeatmap,
    StackedBars,
    DepthHistogram,
    ExtensionDonut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttentionLevel {
    TopTen,
    TopQuarter,
    TopHalf,
    Small,
    Empty,
    Vendor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationItem {
    pub id: String,
    pub path: PathBuf,
    pub label: String,
    pub metric_value: u64,
    pub percent_of_parent: f32,
    pub dominant_language: Option<String>,
    pub languages: Vec<LanguageBreakdown>,
    pub totals: CodeTotals,
    pub attention: AttentionLevel,
}

pub fn attention_for(
    path: &std::path::Path,
    rank_percent: f32,
    metric_value: u64,
) -> AttentionLevel {
    let path_text = path.to_string_lossy().to_lowercase();
    if ["target", "node_modules", "dist", "build", ".venv"]
        .iter()
        .any(|needle| {
            path_text
                .split(std::path::MAIN_SEPARATOR)
                .any(|part| part == *needle)
        })
    {
        return AttentionLevel::Vendor;
    }
    if metric_value == 0 {
        return AttentionLevel::Empty;
    }
    if rank_percent <= 0.10 {
        AttentionLevel::TopTen
    } else if rank_percent <= 0.25 {
        AttentionLevel::TopQuarter
    } else if rank_percent <= 0.50 {
        AttentionLevel::TopHalf
    } else {
        AttentionLevel::Small
    }
}
