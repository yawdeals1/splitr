//! Output file names (FR-51, FR-52).

use std::collections::HashSet;

use serde::Serialize;

/// Separators dropped next to an empty `{title}`. Not ".", which usually belongs to the word
/// before it ("Vol. {title}").
const SEPARATORS: &[char] = &[' ', '-', '_', ','];

/// The longest name in bytes, without its extension or a " (2)" suffix. Leaves room for both
/// within the 255-byte limit of ext4 and APFS (and Windows' 255 characters).
const MAX_NAME_BYTES: usize = 200;

/// Fills a template such as "{original} - {index} - {title}" for part `index` of `total` (FR-51).
/// Tokens are replaced in one pass, so a title that contains "{index}" stays as typed. `{index}` is
/// zero-padded to the width of `total` (at least 2), so names sort in order. An empty title takes
/// the template's separator before it along (or after it, at the start), and a matching pair of
/// brackets around it, so a name doesn't end in " - " or " ()". The PDF's own name and the title
/// are never trimmed. The result is sanitized; [`export_names`] makes names unique.
pub fn render(template: &str, index: usize, total: usize, title: &str, original: &str) -> String {
    let width = total.to_string().len().max(2);
    let title = title.trim();
    let mut name = String::new();
    // Where the template text after the last filled-in token starts; only that text is trimmed.
    let mut text_start = 0;
    // True after an empty title at the start, so the separators after it go too.
    let mut skip_separators = false;
    let mut rest = template;
    loop {
        let at = rest.find('{').unwrap_or(rest.len());
        let mut text = &rest[..at];
        if skip_separators {
            text = text.trim_start_matches(SEPARATORS);
            skip_separators = text.is_empty();
        }
        name.push_str(text);
        rest = &rest[at..];
        if rest.is_empty() {
            break;
        }
        let Some(token) = ["{index}", "{title}", "{original}"].into_iter().find(|t| rest.starts_with(t)) else {
            name.push('{');
            rest = &rest[1..];
            continue;
        };
        rest = &rest[token.len()..];
        match token {
            "{index}" => name.push_str(&format!("{index:0width$}")),
            "{original}" => name.push_str(original),
            _ if !title.is_empty() => name.push_str(title),
            _ => {
                let close = match name.ends_with('(') {
                    true => Some(')'),
                    false if name.ends_with('[') => Some(']'),
                    false => None,
                };
                if let Some(close) = close.filter(|&c| rest.starts_with(c)) {
                    name.pop();
                    rest = &rest[close.len_utf8()..];
                }
                let kept = name[text_start..].trim_end_matches(SEPARATORS).len();
                name.truncate(text_start + kept);
                skip_separators = name.is_empty();
                continue;
            }
        }
        text_start = name.len();
        skip_separators = false;
    }
    sanitize(&name)
}

/// The names for one export (FR-51, FR-52), without extensions: one per part, and one for the ZIP.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct ExportNames {
    pub parts: Vec<String>,
    pub zip: String,
}

/// The names for parts with these `titles`, from `template`, and the ZIP's name (the PDF's name).
/// Part names that would repeat get " (2)", " (3)"... (FR-52), so this is exactly what goes in a
/// ZIP and what the interface previews. Export adds a number too if the folder already has a file
/// of that name, and says so.
pub fn export_names(template: &str, original: &str, titles: &[String]) -> ExportNames {
    let original = strip_pdf(original);
    let mut taken = HashSet::new();
    let parts = titles
        .iter()
        .enumerate()
        .map(|(i, title)| unique(&render(template, i + 1, titles.len(), title, original), &mut taken))
        .collect();
    ExportNames { parts, zip: sanitize(original) }
}

/// `name` without a trailing ".pdf" in any letter case.
pub fn strip_pdf(name: &str) -> &str {
    match name.len().checked_sub(4) {
        Some(at) if name.is_char_boundary(at) && name[at..].eq_ignore_ascii_case(".pdf") => &name[..at],
        _ => name,
    }
}

/// Makes a name safe on Windows, macOS and Linux: no forbidden characters, no leading dot (hidden
/// on macOS and Linux), no trailing dot or space (dropped by Windows), no reserved device name, and
/// at most [`MAX_NAME_BYTES`].
pub fn sanitize(name: &str) -> String {
    const RESERVED: [&str; 30] = [
        "CON", "PRN", "AUX", "NUL", "COM0", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
        "COM8", "COM9", "COM¹", "COM²", "COM³", "LPT0", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6",
        "LPT7", "LPT8", "LPT9", "LPT¹", "LPT²", "LPT³",
    ];
    let cleaned: String = name
        .chars()
        .map(|c| if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control() { '_' } else { c })
        .collect();
    let mut result = tidy(truncate(cleaned.trim().trim_start_matches('.'), MAX_NAME_BYTES)).to_string();
    if result.is_empty() {
        result = "part".into();
    }
    // Windows reserves these names with any extension too ("CON.txt"), so check the part before the
    // first dot, and add "_" there.
    let stem = result.find('.').unwrap_or(result.len());
    if RESERVED.contains(&result[..stem].trim_end().to_uppercase().as_str()) {
        result.insert(stem, '_');
    }
    result
}

/// Adds " (2)", " (3)"... so no two names in `taken` match, ignoring letter case. The name is
/// shortened if needed, so it stays within [`MAX_NAME_BYTES`] and `sanitize` leaves it unchanged.
pub fn dedupe(name: &str, taken: &mut HashSet<String>) -> String {
    let mut candidate = name.to_string();
    let mut n = 2;
    while !taken.insert(candidate.to_lowercase()) {
        let suffix = format!(" ({n})");
        candidate = format!("{}{suffix}", tidy(truncate(name, MAX_NAME_BYTES - suffix.len())));
        n += 1;
    }
    candidate
}

/// A safe name for `name` that isn't in `taken` yet, which it is added to (FR-52).
pub fn unique(name: &str, taken: &mut HashSet<String>) -> String {
    dedupe(&sanitize(name), taken)
}

/// `text` cut to at most `max` bytes, on a character boundary.
fn truncate(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// `text` without trailing spaces and dots, which Windows drops from file names.
fn tidy(text: &str) -> &str {
    text.trim_end_matches([' ', '.'])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_template() {
        assert_eq!(render("{index} - {title}", 3, 10, "Memory Hierarchy", "book"), "03 - Memory Hierarchy");
        assert_eq!(render("{original}_{index}", 1, 1, "", "book"), "book_01");
        assert_eq!(render("{title}: {original}", 1, 1, "A/B", "book"), "A_B_ book");
        assert_eq!(render("{original} {unknown} {", 1, 1, "", "book"), "book {unknown} {");
        assert_eq!(render("{original}", 1, 1, "", "_draft-"), "_draft-");
    }

    #[test]
    fn tokens_inside_a_title_stay_as_typed() {
        assert_eq!(render("{title} - {index}", 1, 1, "{index} and {original}", "book"), "{index} and {original} - 01");
        assert_eq!(render("{original}", 1, 1, "", "{title}"), "{title}");
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
        assert_eq!(render("{original} ({title})", 1, 1, "", "book"), "book");
        assert_eq!(render("{original} [{title}] {index}", 1, 1, "", "book"), "book 01");
        assert_eq!(render("{original} ({title})", 1, 1, "Intro", "book"), "book (Intro)");
        assert_eq!(render("Vol. {title} - {index}", 1, 1, "", "book"), "Vol. - 01");
        // Only the template's separators go, never the end of the PDF's name.
        assert_eq!(render("{original}_{title}", 1, 1, "", "draft_"), "draft_");
        assert_eq!(render("{original} - {index}", 1, 1, "", "_index"), "_index - 01");
        // At the start, the separators after the title go.
        assert_eq!(render("{title}{index} - {original}", 1, 1, "", "book"), "01 - book");
        // Only a matching pair of brackets is removed.
        assert_eq!(render("{original} [{title})", 1, 1, "", "book"), "book [)");
    }

    #[test]
    fn export_names_number_every_part() {
        let titles = ["Intro".to_string(), String::new()];
        assert_eq!(
            export_names("{original} - {index} - {title}", "Book.PDF", &titles),
            ExportNames { parts: vec!["Book - 01 - Intro".into(), "Book - 02".into()], zip: "Book".into() }
        );
        let same = ["Intro".to_string(), "intro".to_string(), "Intro".to_string()];
        assert_eq!(export_names("{title}", "a.pdf", &same).parts, ["Intro", "intro (2)", "Intro (3)"]);
        assert!(export_names("{index}", "a.pdf", &[]).parts.is_empty());
        // The ZIP is named after the PDF, with ".pdf" removed once and made safe.
        assert_eq!(export_names("{index}", "scan.pdf.pdf", &[]).zip, "scan.pdf");
        assert_eq!(export_names("{index}", "a:b.pdf", &[]).zip, "a_b");
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
        assert_eq!(sanitize("CON.x"), "CON_.x");
        assert_eq!(sanitize("lpt1 .notes"), "lpt1 _.notes");
        assert_eq!(sanitize("console.x"), "console.x");
        assert_eq!(sanitize("notes..."), "notes");
        assert_eq!(sanitize("COM0"), "COM0_");
        assert_eq!(sanitize("lpt³.x"), "lpt³_.x");
        // A leading dot hides a file on macOS and Linux.
        assert_eq!(sanitize(".notes"), "notes");
        assert_eq!(sanitize("..."), "part");
    }

    #[test]
    fn long_names_fit_every_file_system() {
        // 3-byte characters: 120 of them are 360 bytes, over the 255-byte limit of ext4 and APFS.
        let long = "語".repeat(120);
        let name = sanitize(&long);
        assert!(name.len() <= MAX_NAME_BYTES && name.chars().all(|c| c == '語'), "{name}");
        // A cut never leaves a trailing space or dot, which Windows drops.
        let spaced = format!("{} .x", "a".repeat(MAX_NAME_BYTES - 2));
        assert_eq!(sanitize(&spaced), "a".repeat(MAX_NAME_BYTES - 2));
        // A numbered name stays within the limit, and sanitizing it again changes nothing.
        let mut taken = HashSet::new();
        let first = unique(&long, &mut taken);
        let second = unique(&long, &mut taken);
        assert!(second.ends_with(" (2)") && second.len() <= MAX_NAME_BYTES, "{second}");
        assert_ne!(first, second);
        assert_eq!(sanitize(&second), second);
    }

    #[test]
    fn dedupes_names() {
        let mut taken = HashSet::new();
        assert_eq!(dedupe("Invoice", &mut taken), "Invoice");
        assert_eq!(dedupe("invoice", &mut taken), "invoice (2)");
        assert_eq!(dedupe("Invoice", &mut taken), "Invoice (3)");
    }
}
