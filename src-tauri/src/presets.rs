//! Preset library (FR-70, FR-72, SRS 6.1). Presets are data: edit presets/presets.json, not this file.

use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

/// Read once. The `bundled_presets_are_valid` test checks the file, so it can't be empty in a build
/// that passed CI.
static BUNDLED: LazyLock<Vec<Preset>> =
    LazyLock::new(|| serde_json::from_str(include_str!("../../presets/presets.json")).unwrap_or_default());

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
    /// Pages in each part, for the fixed strategy (FR-21).
    #[serde(default)]
    pub every: Option<u32>,
    /// True for presets the user saved (FR-71), which they can edit and delete.
    #[serde(default)]
    pub user: bool,
}

pub fn bundled() -> Vec<Preset> {
    BUNDLED.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// A word after "number" or "no" isn't a number: a match needs a digit.
    #[test]
    fn bundled_patterns_match_numbers_not_words() {
        let pattern = |id: &str| {
            let presets = bundled();
            let preset = presets.iter().find(|p| p.id == id).unwrap();
            regex::Regex::new(preset.pattern.as_deref().unwrap()).unwrap()
        };
        let cases = [
            ("invoices-by-number", "Invoice No: INV-1001", "Your invoice number is below. Invoice note"),
            ("statements-by-account", "Account Number: **** 0098", "Account number    (see above) ****"),
            ("hr-by-employee", "Employee ID: E-204", "Employee identification and employee number here"),
            ("insurance-by-policy", "Policy No. P-77", "This policy notice and claim notification"),
            ("shipping-by-shipment", "B/L No: MAEU123", "The bill of lading shall state the b/l date"),
        ];
        for (id, number, words) in cases {
            assert!(pattern(id).is_match(number), "{id} should match {number:?}");
            assert!(!pattern(id).is_match(words), "{id} should not match {words:?}");
        }
    }

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
            if p.strategy == StrategyKind::Fixed {
                assert!(p.every.is_some_and(|n| n > 0), "{} needs every > 0", p.id);
            }
            assert_ne!(p.fallback, Some(p.strategy), "{} falls back to its own strategy", p.id);
        }
    }
}
