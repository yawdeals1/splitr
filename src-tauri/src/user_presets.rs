//! The user's own presets (FR-71): saved custom prompts, stored as JSON in the app config folder
//! and listed after the bundled presets.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Result, SplitError};
use crate::files;
use crate::presets::{self, Preset, StrategyKind};

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

/// Every preset to show, and why the user's own are missing if they couldn't be read.
#[derive(Debug, Serialize)]
pub struct PresetList {
    pub presets: Vec<Preset>,
    pub problem: Option<String>,
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
            every: None,
            user: true,
        }
    }
}

/// The stored presets that can be used, and whether anything in the file had to be skipped.
struct ReadResult {
    presets: Vec<Stored>,
    skipped: bool,
}

/// Reads the file. A missing file is an empty list. Entries that can't be used are skipped, and
/// so is a file that isn't JSON. A file that exists but can't be read is an error, so nothing is
/// saved over it.
fn read(path: &Path) -> Result<ReadResult> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(ReadResult { presets: Vec::new(), skipped: false }),
        Err(e) => {
            return Err(SplitError::Presets(format!(
                "Splitr can't read your saved presets in {} ({e}). Close any program using the file and try again.",
                path.display()
            )))
        }
    };
    let Ok(Value::Array(entries)) = serde_json::from_str::<Value>(&text) else {
        return Ok(ReadResult { presets: Vec::new(), skipped: true });
    };
    let total = entries.len();
    let presets: Vec<Stored> = entries
        .into_iter()
        .filter_map(|v| serde_json::from_value::<Stored>(v).ok())
        .filter(|s| s.id.starts_with(ID_PREFIX))
        .collect();
    Ok(ReadResult { skipped: presets.len() != total, presets })
}

/// Reads the file before a change. If entries were skipped, the file is copied to
/// `user-presets.json.bak` first, so the write that follows can't lose them for good.
fn read_for_change(path: &Path) -> Result<Vec<Stored>> {
    let read = read(path)?;
    if read.skipped {
        fs::copy(path, path.with_extension("json.bak")).map_err(|e| {
            SplitError::Presets(format!(
                "Your saved presets file has entries Splitr can't read, and it could not be backed up ({e}), so nothing was changed."
            ))
        })?;
    }
    Ok(read.presets)
}

/// Saves the list so that a failed write keeps the old one.
fn write(path: &Path, presets: &[Stored]) -> Result<()> {
    let json = serde_json::to_string_pretty(presets)
        .map_err(|e| SplitError::Presets(format!("Could not save your presets ({e}).")))?;
    files::write_atomically(path, json.as_bytes())
        .map_err(|e| SplitError::Presets(format!("Could not save your presets to {} ({e}).", path.display())))
}

/// Held while a change reads the file and writes it back, so that two changes at once can't lose
/// one of them.
fn changing() -> MutexGuard<'static, ()> {
    static CHANGING: Mutex<()> = Mutex::new(());
    CHANGING.lock().unwrap_or_else(|e| e.into_inner())
}

/// User presets. A missing file gives an empty list, and entries that can't be used are left out.
/// A file that can't be read is an error.
pub fn load(path: &Path) -> Result<Vec<Preset>> {
    Ok(read(path)?.presets.into_iter().map(Preset::from).collect())
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
        return Err(SplitError::Presets(format!(
            "Keep the name and category to {MAX_NAME} characters or fewer."
        )));
    }
    if prompt.chars().count() > MAX_PROMPT {
        return Err(SplitError::Presets(format!("Keep the instruction to {MAX_PROMPT} characters or fewer.")));
    }

    let _changing = changing();
    let mut presets = read_for_change(path)?;
    // Names are compared in any letter case, including non-English letters, against the bundled
    // presets too, so the list never shows two names that look the same.
    let same = |a: &str, b: &str| a.to_lowercase() == b.to_lowercase();
    let duplicate_user = presets
        .iter()
        .any(|p| same(&p.name, name) && same(&p.category, category) && Some(&p.id) != input.id.as_ref());
    let duplicate_bundled = presets::bundled()
        .iter()
        .any(|p| same(&p.name, name) && same(&p.category, category));
    if duplicate_user || duplicate_bundled {
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
            let stored = Stored { id: next_id(&presets)?, ..stored };
            presets.push(stored.clone());
            stored
        }
    };
    write(path, &presets)?;
    Ok(stored.into())
}

/// Removes a user preset. Bundled presets can't be deleted.
pub fn delete(path: &Path, id: &str) -> Result<()> {
    let _changing = changing();
    let mut presets = read_for_change(path)?;
    let before = presets.len();
    presets.retain(|p| p.id != id);
    if presets.len() == before {
        return Err(SplitError::Presets("That preset no longer exists.".into()));
    }
    write(path, &presets)
}

/// A new id that no preset has used. It is at least the current time in milliseconds, so the id
/// of a deleted preset isn't given to a new one, and anything that still refers to it (such as a
/// last-used preset) can't point at a different prompt.
fn next_id(presets: &[Stored]) -> Result<String> {
    let max = presets
        .iter()
        .filter_map(|p| p.id.strip_prefix(ID_PREFIX)?.parse::<u64>().ok())
        .max()
        .unwrap_or(0);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
    let next = max
        .checked_add(1)
        .ok_or_else(|| SplitError::Presets("Your saved presets file has an id Splitr can't continue from.".into()))?;
    Ok(format!("{ID_PREFIX}{}", next.max(now)))
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
    fn changes_at_the_same_time_are_all_kept() {
        let path = temp_file("user-presets-overlap.json");
        let threads: Vec<_> = (0..8)
            .map(|n| {
                let path = path.clone();
                std::thread::spawn(move || save(&path, input(&format!("Preset {n}"), "Split it")).unwrap())
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(load(&path).unwrap().len(), 8);
    }

    #[test]
    fn saves_and_reloads() {
        let path = temp_file("save");
        let saved = save(&path, input(" Clients ", "Split at each new client")).unwrap();
        assert!(saved.id.starts_with(ID_PREFIX));
        assert_eq!(saved.name, "Clients");
        assert!(saved.user);
        assert_eq!(saved.strategy, StrategyKind::Ai);
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].prompt, "Split at each new client");
        assert_ne!(save(&path, input("Other", "x")).unwrap().id, saved.id);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn edits_and_deletes() {
        let path = temp_file("edit");
        let saved = save(&path, input("A", "one")).unwrap();
        let edited = save(&path, PresetInput { id: Some(saved.id.clone()), ..input("B", "two") }).unwrap();
        assert_eq!(edited.id, saved.id);
        let loaded = load(&path).unwrap();
        assert_eq!((loaded.len(), loaded[0].name.as_str()), (1, "B"));
        delete(&path, &saved.id).unwrap();
        assert!(load(&path).unwrap().is_empty());
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
        assert!(save(&path, input(&"n".repeat(MAX_NAME), &"x".repeat(MAX_PROMPT))).is_ok());
        assert!(save(&path, PresetInput { id: Some("user-9".into()), ..input("A", "x") }).is_err());
        save(&path, input("A", "x")).unwrap();
        assert!(save(&path, input("a", "y")).is_err(), "duplicate name in the same category");
        save(&path, input("Élèves", "x")).unwrap();
        assert!(save(&path, input("élèves", "y")).is_err(), "duplicate in non-English letters");
        let bundled = &presets::bundled()[0];
        let clash = PresetInput { category: bundled.category.clone(), ..input(&bundled.name, "x") };
        assert!(save(&path, clash).is_err(), "same name as a bundled preset");
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn bad_entries_are_skipped_and_backed_up() {
        let path = temp_file("corrupt");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "[{ nope").unwrap();
        assert!(load(&path).unwrap().is_empty());
        // Entries without the user prefix can't pose as bundled presets.
        fs::write(&path, r#"[{"id":"books-by-chapter","category":"c","name":"n","prompt":"p"}]"#).unwrap();
        assert!(load(&path).unwrap().is_empty());

        let file = r#"[{"id":"user-1","category":"c","name":"Good","prompt":"p"}, {"id":"user-2","name":"no prompt"}]"#;
        fs::write(&path, file).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!((loaded.len(), loaded[0].name.as_str()), (1, "Good"));
        save(&path, input("New", "x")).unwrap();
        assert_eq!(fs::read_to_string(path.with_extension("json.bak")).unwrap(), file);
        assert_eq!(load(&path).unwrap().len(), 2, "the good entry is kept");
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn unreadable_file_is_not_overwritten() {
        // A folder where the file should be can't be read as text.
        let path = temp_file("unreadable");
        fs::create_dir_all(&path).unwrap();
        assert!(load(&path).is_err());
        assert!(save(&path, input("A", "x")).is_err());
        assert!(path.is_dir());
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn ids_are_not_reused_and_do_not_overflow() {
        let stored = |id: &str| Stored { id: id.into(), category: "c".into(), name: id.into(), prompt: "p".into() };
        let after_delete = next_id(&[stored("user-1")]).unwrap();
        assert_ne!(after_delete, "user-2", "ids follow the clock, not just the highest id");
        assert!(next_id(&[stored(&format!("user-{}", u64::MAX))]).is_err());
        assert_eq!(next_id(&[stored("user-99999999999999")]).unwrap(), "user-100000000000000");
    }
}
