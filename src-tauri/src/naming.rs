//! Output file names (FR-51, FR-52).

use std::collections::HashSet;

/// Separators trimmed from around an empty `{title}` and from the ends of a name.
const SEPARATORS: &[char] = &[' ', '-', '_', '.', ','];

/// Fills a template such as "{original} - {index} - {title}" for part `index` of `total` (FR-51).
/// `{index}` is zero-padded to the width of `total` (at least 2), so names sort in order. An empty
/// title takes the separator before it along, so the name doesn't end in " - ". The result is
/// sanitized; [`dedupe`] makes it unique.
pub fn render(template: &str, index: usize, total: usize, title: &str, original: &str) -> String {
    let width = total.to_string().len().max(2);
    let title = title.trim();
    let mut name = template.to_string();
    if title.is_empty() {
        while let Some(at) = name.find("{title}") {
            let start = name[..at].trim_end_matches(SEPARATORS).len();
            name.replace_range(start..at + "{title}".len(), "");
        }
    }
    let name = name
        .replace("{index}", &format!("{index:0width$}"))
        .replace("{title}", title)
        .replace("{original}", original);
    sanitize(name.trim_matches(SEPARATORS))
}

/// The file names (without ".pdf") for parts with these `titles`, from `template` (FR-51).
pub fn part_names(template: &str, original: &str, titles: &[String]) -> Vec<String> {
    let original = strip_pdf(original);
    titles
        .iter()
        .enumerate()
        .map(|(i, title)| render(template, i + 1, titles.len(), title, original))
        .collect()
}

/// `name` without a trailing ".pdf" in any letter case.
pub fn strip_pdf(name: &str) -> &str {
    match name.len().checked_sub(4) {
        Some(at) if name.is_char_boundary(at) && name[at..].eq_ignore_ascii_case(".pdf") => &name[..at],
        _ => name,
    }
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
        assert_eq!(render("{index} - {title}", 3, 10, "Memory Hierarchy", "book"), "03 - Memory Hierarchy");
        assert_eq!(render("{original}_{index}", 1, 1, "", "book"), "book_01");
        assert_eq!(render("{title}: {original}", 1, 1, "A/B", "book"), "A_B_ book");
    }

    #[test]
    fn index_is_padded_to_the_part_count() {
        assert_eq!(render("{index}", 7, 9, "", "b"), "07");
        assert_eq!(render("{index}", 7, 120, "", "b"), "007");
        assert_eq!(render("{index}", 120, 120, "", "b"), "120");
    }

    #[test]
    fn an_empty_title_drops_its_separator() {
        let t = "{original} - {index} - {title}";
        assert_eq!(render(t, 1, 2, "", "book"), "book - 01");
        assert_eq!(render(t, 1, 2, "   ", "book"), "book - 01");
        assert_eq!(render("{title} - {index}", 2, 2, "", "book"), "02");
        assert_eq!(render("{original} - {title} - {index}", 2, 2, "", "book"), "book - 02");
        assert_eq!(render("{title}", 1, 1, "", "book"), "part");
    }

    #[test]
    fn part_names_number_every_part() {
        let titles = ["Intro".to_string(), String::new()];
        assert_eq!(
            part_names("{original} - {index} - {title}", "Book.PDF", &titles),
            ["Book - 01 - Intro", "Book - 02"]
        );
        assert!(part_names("{index}", "a.pdf", &[]).is_empty());
    }

    #[test]
    fn strips_the_pdf_extension() {
        assert_eq!(strip_pdf("a.pdf"), "a");
        assert_eq!(strip_pdf("a.PDF"), "a");
        assert_eq!(strip_pdf("a.pdf.zip"), "a.pdf.zip");
        assert_eq!(strip_pdf("pdf"), "pdf");
        assert_eq!(strip_pdf("é.pdf"), "é");
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
