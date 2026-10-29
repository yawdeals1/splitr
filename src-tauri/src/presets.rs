//! Preset library (FR-70, FR-72, SRS 6.1). Presets are data: edit presets/presets.json, not this file.

use serde::{Deserialize, Serialize};

const BUNDLED: &str = include_str!("../../presets/presets.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StrategyKind {
    Outline,
    Pattern,
    Blank,
    Fixed,
    Ai,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub category: String,
    pub name: String,
    pub strategy: StrategyKind,
    #[serde(default)]
    pub fallback: Option<StrategyKind>,
    pub prompt: String,
    #[serde(default)]
    pub pattern: Option<String>,
    /// True for presets the user saved (FR-71), which they can edit and delete.
    #[serde(default)]
    pub user: bool,
}

pub fn bundled() -> Vec<Preset> {
    serde_json::from_str(BUNDLED).expect("presets/presets.json is invalid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn bundled_presets_are_valid() {
        let presets = bundled();
        assert!(!presets.is_empty());
        let mut ids = HashSet::new();
        for p in &presets {
            assert!(ids.insert(p.id.clone()), "duplicate preset id {}", p.id);
            assert!(!p.user && !p.id.starts_with("user-"), "{} uses the user preset id space", p.id);
            if p.strategy == StrategyKind::Pattern {
                let pattern = p.pattern.as_deref().unwrap_or_else(|| panic!("{} has no pattern", p.id));
                regex::Regex::new(pattern).unwrap_or_else(|e| panic!("{} has a bad pattern: {e}", p.id));
            }
        }
    }
}
