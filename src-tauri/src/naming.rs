//! Output file names (FR-51, FR-52).

use std::collections::HashSet;

/// Characters trimmed from around an empty `{title}` and from the ends of a name. Not ".", which
/// usually belongs to the word before it ("Vol. {title}").
const SEPARATORS: &[char] = &[' ', '-', '_', ','];

/// Fills a template such as "{original} - {index} - {title}" for part `index` of `total` (FR-51).
/// Tokens are replaced in one pass, so a title that contains "{index}" stays as typed. `{index}` is
/// zero-padded to the width of `total` (at least 2), so names sort in order. An empty title takes
/// the separator before it along, and the brackets around it, so a name doesn't end in " - " or
/// " ()". The result is sanitized; [`part_names`] makes names unique.
pub fn render(template: &str, index: usize, total: usize, title: &str, original: &str) -> String {
    let width = total.to_string().len().max(2);
    let title = title.trim();
    let mut name = String::new();
    let mut rest = template;
    while let Some(at) = rest.find('{') {
        name.push_str(&rest[..at]);
        rest = &rest[at..];
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
                // Drop an opening bracket right before the empty title, its closing partner after
                // it, and the separators before them.
                if name.ends_with(['(', '[']) {
                    name.pop();
                    if rest.starts_with([')', ']']) {
                        rest = &rest[1..];
                    }
                }
                name.truncate(name.trim_end_matches(SEPARATORS).len());
            }
        }
    }
    name.push_str(rest);
    sanitize(name.trim_matches(SEPARATORS))
}

/// The file names (without ".pdf") for parts with these `titles`, from `template` (FR-51). Names
/// that would repeat get " (2)", " (3)"... (FR-52), so this is exactly what goes in a ZIP and what
/// the interface previews. Export adds a number too if the folder already has a file of that name.
pub fn part_names(template: &str, original: &str, titles: &[String]) -> Vec<String> {
    let original = strip_pdf(original);
    let mut taken = HashSet::new();
    titles
        .iter()
        .enumerate()
        .map(|(i, title)| dedupe(&render(template, i + 1, titles.len(), title, original), &mut taken))
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
    // Windows reserves these names with any extension too ("CON.txt"), so check the part before the
    // first dot, and add "_" there.
    let stem = result.find('.').unwrap_or(result.len());
    if RESERVED.contains(&result[..stem].trim_end().to_uppercase().as_str()) {
        result.insert(stem, '_');
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
        assert_eq!(render("{original} {unknown} {", 1, 1, "", "book"), "book {unknown} {");
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
    }

    #[test]
    fn part_names_number_every_part() {
        let titles = ["Intro".to_string(), String::new()];
        assert_eq!(
            part_names("{original} - {index} - {title}", "Book.PDF", &titles),
            ["Book - 01 - Intro", "Book - 02"]
        );
        let same = ["Intro".to_string(), "intro".to_string(), "Intro".to_string()];
        assert_eq!(part_names("{title}", "a.pdf", &same), ["Intro", "intro (2)", "Intro (3)"]);
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
        assert_eq!(sanitize("CON.x"), "CON_.x");
        assert_eq!(sanitize("lpt1 .notes"), "lpt1 _.notes");
        assert_eq!(sanitize("console.x"), "console.x");
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
