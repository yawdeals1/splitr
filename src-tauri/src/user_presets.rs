//! The user's own presets (FR-71): saved custom prompts, stored as JSON in the app config folder
//! and listed after the bundled presets.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Result, SplitError};
use crate::presets::{Preset, StrategyKind};

pub const FILE_NAME: &str = "user-presets.json";
const ID_PREFIX: &str = "user-";
const MAX_NAME: usize = 100;
const MAX_PROMPT: usize = 2000;

/// What the interface sends to create (no `id`) or edit (with `id`) a user preset.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetInput {
    pub id: Option<String>,
    pub category: String,
    pub name: String,
    pub prompt: String,
}

/// What is stored on disk. User presets always use the AI strategy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Stored {
    id: String,
    category: String,
    name: String,
    prompt: String,
}

impl From<Stored> for Preset {
    fn from(s: Stored) -> Self {
        Preset {
            id: s.id,
            category: s.category,
            name: s.name,
            strategy: StrategyKind::Ai,
            fallback: None,
            prompt: s.prompt,
            pattern: None,
            user: true,
        }
    }
}

fn read(path: &Path) -> Vec<Stored> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Vec<Stored>>(&text).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|s| s.id.starts_with(ID_PREFIX))
        .collect()
}

/// Writes to a temp file and renames it, so a failed write keeps the old list.
fn write(path: &Path, presets: &[Stored]) -> Result<()> {
    let fail = |e: std::io::Error| SplitError::Presets(format!("Could not save your presets: {e}"));
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(fail)?;
    }
    let json = serde_json::to_string_pretty(presets).map_err(|e| SplitError::Presets(e.to_string()))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(fail)?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        fail(e)
    })
}

/// User presets. A missing or corrupt file gives an empty list.
pub fn load(path: &Path) -> Vec<Preset> {
    read(path).into_iter().map(Preset::from).collect()
}

/// Adds a preset, or replaces the one with the same id. Returns the saved preset.
pub fn save(path: &Path, input: PresetInput) -> Result<Preset> {
    let category = input.category.trim();
    let name = input.name.trim();
    let prompt = input.prompt.trim();
    if name.is_empty() {
        return Err(SplitError::Presets("Give the preset a name.".into()));
    }
    if category.is_empty() {
        return Err(SplitError::Presets("Choose a category for the preset.".into()));
    }
    if prompt.is_empty() {
        return Err(SplitError::Presets("Write an instruction before saving it as a preset.".into()));
    }
    if name.chars().count() > MAX_NAME || category.chars().count() > MAX_NAME {
        return Err(SplitError::Presets(format!("Keep the name and category under {MAX_NAME} characters.")));
    }
    if prompt.chars().count() > MAX_PROMPT {
        return Err(SplitError::Presets(format!("Keep the instruction under {MAX_PROMPT} characters.")));
    }

    let mut presets = read(path);
    let duplicate = presets.iter().any(|p| {
        p.name.eq_ignore_ascii_case(name) && p.category.eq_ignore_ascii_case(category) && Some(&p.id) != input.id.as_ref()
    });
    if duplicate {
        return Err(SplitError::Presets(format!("You already have a preset called \"{name}\" in {category}.")));
    }

    let stored = Stored { id: String::new(), category: category.into(), name: name.into(), prompt: prompt.into() };
    let stored = match input.id {
        Some(id) => {
            let slot = presets
                .iter_mut()
                .find(|p| p.id == id)
                .ok_or_else(|| SplitError::Presets("That preset no longer exists.".into()))?;
            *slot = Stored { id, ..stored };
            slot.clone()
        }
        None => {
            let stored = Stored { id: next_id(&presets), ..stored };
            presets.push(stored.clone());
            stored
        }
    };
    write(path, &presets)?;
    Ok(stored.into())
}

/// Removes a user preset. Bundled presets can't be deleted.
pub fn delete(path: &Path, id: &str) -> Result<()> {
    let mut presets = read(path);
    let before = presets.len();
    presets.retain(|p| p.id != id);
    if presets.len() == before {
        return Err(SplitError::Presets("That preset no longer exists.".into()));
    }
    write(path, &presets)
}

fn next_id(presets: &[Stored]) -> String {
    let max = presets
        .iter()
        .filter_map(|p| p.id.strip_prefix(ID_PREFIX)?.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    format!("{ID_PREFIX}{}", max + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_file(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("splitr-user-presets-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join(FILE_NAME)
    }

    fn input(name: &str, prompt: &str) -> PresetInput {
        PresetInput { id: None, category: "My presets".into(), name: name.into(), prompt: prompt.into() }
    }

    #[test]
    fn saves_and_reloads() {
        let path = temp_file("save");
        let saved = save(&path, input(" Clients ", "Split at each new client")).unwrap();
        assert_eq!(saved.id, "user-1");
        assert_eq!(saved.name, "Clients");
        assert!(saved.user);
        assert_eq!(saved.strategy, StrategyKind::Ai);
        let loaded = load(&path);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].prompt, "Split at each new client");
        assert_eq!(save(&path, input("Other", "x")).unwrap().id, "user-2");
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn edits_and_deletes() {
        let path = temp_file("edit");
        let saved = save(&path, input("A", "one")).unwrap();
        let edited = save(&path, PresetInput { id: Some(saved.id.clone()), ..input("B", "two") }).unwrap();
        assert_eq!(edited.id, saved.id);
        let loaded = load(&path);
        assert_eq!((loaded.len(), loaded[0].name.as_str()), (1, "B"));
        delete(&path, &saved.id).unwrap();
        assert!(load(&path).is_empty());
        assert!(delete(&path, &saved.id).is_err());
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn rejects_bad_input() {
        let path = temp_file("bad");
        assert!(save(&path, input(" ", "x")).is_err());
        assert!(save(&path, input("A", "  ")).is_err());
        assert!(save(&path, PresetInput { category: "".into(), ..input("A", "x") }).is_err());
        assert!(save(&path, input("A", &"x".repeat(MAX_PROMPT + 1))).is_err());
        assert!(save(&path, PresetInput { id: Some("user-9".into()), ..input("A", "x") }).is_err());
        save(&path, input("A", "x")).unwrap();
        assert!(save(&path, input("a", "y")).is_err(), "duplicate name in the same category");
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn corrupt_file_gives_empty_list() {
        let path = temp_file("corrupt");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "[{ nope").unwrap();
        assert!(load(&path).is_empty());
        // Entries without the user prefix can't pose as bundled presets.
        fs::write(&path, r#"[{"id":"books-by-chapter","category":"c","name":"n","prompt":"p"}]"#).unwrap();
        assert!(load(&path).is_empty());
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
