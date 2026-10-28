//! Output file names (FR-51, FR-52).
#![allow(dead_code)]

use std::collections::HashSet;

/// Fills a template such as "{original} - {index} - {title}".
pub fn render(template: &str, index: usize, title: &str, original: &str) -> String {
    let name = template
        .replace("{index}", &format!("{index:02}"))
        .replace("{title}", title)
        .replace("{original}", original);
    sanitize(&name)
}

/// Makes a name safe on Windows, macOS and Linux.
pub fn sanitize(name: &str) -> String {
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let cleaned: String = name
        .chars()
        .map(|c| if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control() { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim().trim_end_matches('.').trim();
    let mut result: String = trimmed.chars().take(200).collect();
    if result.is_empty() {
        result = "part".into();
    }
    if RESERVED.contains(&result.to_uppercase().as_str()) {
        result.push('_');
    }
    result
}

/// Adds " (2)", " (3)"... so no two parts share a name and nothing is overwritten.
pub fn dedupe(name: &str, taken: &mut HashSet<String>) -> String {
    let mut candidate = name.to_string();
    let mut n = 2;
    while !taken.insert(candidate.to_lowercase()) {
        candidate = format!("{name} ({n})");
        n += 1;
    }
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_template() {
        assert_eq!(render("{index} - {title}", 3, "Memory Hierarchy", "book"), "03 - Memory Hierarchy");
    }

    #[test]
    fn sanitizes_names() {
        assert_eq!(sanitize("a/b:c?"), "a_b_c_");
        assert_eq!(sanitize("  "), "part");
        assert_eq!(sanitize("con"), "con_");
        assert_eq!(sanitize("notes..."), "notes");
    }

    #[test]
    fn dedupes_names() {
        let mut taken = HashSet::new();
        assert_eq!(dedupe("Invoice", &mut taken), "Invoice");
        assert_eq!(dedupe("invoice", &mut taken), "invoice (2)");
        assert_eq!(dedupe("Invoice", &mut taken), "Invoice (3)");
    }
}
