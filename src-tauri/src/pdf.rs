//! PDF engine (SRS 3): open, render thumbnails, extract text and outline, write parts.
//!
//! PDFium (via pdfium-render) renders, extracts text and reads the outline; lopdf writes parts.
//! The PDFium library ships as a Tauri resource in `pdfium/` (SRS 2.5, 5.3, NFR-12), fetched by
//! `scripts/fetch-pdfium.mjs`. The original file is never modified (FR-04).

use std::collections::{HashMap, HashSet};
use std::io::{Cursor, ErrorKind};
use std::ops::ControlFlow;
use std::path::Path;
use std::sync::OnceLock;

use image::ImageFormat;
use lopdf::{Dictionary, Document, Object, ObjectId};
use pdfium_render::prelude::{PdfDocument, PdfRenderConfig, Pdfium, PdfiumError, PdfiumInternalError};
use serde::Serialize;

use crate::error::{Result, SplitError};
use crate::strategy::{DocumentText, OutlineEntry};

/// The PDFium instance loaded at startup, or why it failed to load.
static ENGINE: OnceLock<std::result::Result<Pdfium, String>> = OnceLock::new();

/// Loads the PDFium library from `dir` (the `pdfium` resource folder). Call once at startup.
/// A failure is kept, so every later PDF call reports it to the user.
pub fn init(dir: &Path) -> Result<()> {
    ENGINE
        .get_or_init(|| load(dir))
        .as_ref()
        .map(|_| ())
        .map_err(|e| SplitError::PdfEngine(e.clone()))
}

fn load(dir: &Path) -> std::result::Result<Pdfium, String> {
    let path = Pdfium::pdfium_platform_library_name_at_path(dir);
    if !path.is_file() {
        return Err(format!(
            "the PDFium library is missing ({}). Reinstall Splitr; developers run `npm run fetch-pdfium`",
            path.display()
        ));
    }
    let bindings = Pdfium::bind_to_library(&path)
        .map_err(|e| format!("the PDFium library at {} could not be loaded ({e})", path.display()))?;
    Ok(Pdfium::new(bindings))
}

/// The loaded PDFium instance, or a user-facing error if it failed to load.
pub fn engine() -> Result<&'static Pdfium> {
    match ENGINE.get() {
        Some(Ok(pdfium)) => Ok(pdfium),
        Some(Err(e)) => Err(SplitError::PdfEngine(e.clone())),
        None => Err(SplitError::PdfEngine("the PDFium library was not loaded".into())),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfInfo {
    pub path: String,
    pub file_name: String,
    pub page_count: u32,
    pub size_bytes: u64,
}

/// Opens the PDF at `path` read-only and returns its name, page count and size (FR-01, FR-02).
/// `password` unlocks an encrypted file (FR-03). Every failure is a specific, user-facing error,
/// so the app stays usable (FR-05, NFR-08). The file is only read, never written (FR-04).
/// The loaded document is returned too, so its pages can be rendered (FR-10); the password
/// unlocks it in memory and is not kept.
pub fn open_document(path: &str, password: Option<&str>) -> Result<(PdfInfo, PdfDocument<'static>)> {
    let file_path = Path::new(path);
    let file_name = file_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());

    let metadata = std::fs::metadata(file_path).map_err(|e| io_error(&e, &file_name))?;
    if !metadata.is_file() {
        return Err(SplitError::CorruptPdf(file_name));
    }

    let password = password.filter(|p| !p.is_empty());
    let document = engine()?
        .load_pdf_from_file(file_path, password)
        .map_err(|e| load_error(e, &file_name, password.is_some()))?;

    let page_count = u32::try_from(document.pages().len()).unwrap_or(0);
    if page_count == 0 {
        return Err(SplitError::EmptyPdf(file_name));
    }

    let info = PdfInfo { path: path.to_string(), file_name, page_count, size_bytes: metadata.len() };
    Ok((info, document))
}

/// Rendered pages are never taller than this, so a very long page (a receipt, a scroll) can't
/// need a huge bitmap. Such a page renders narrower than asked, keeping its aspect ratio.
pub const MAX_RENDER_HEIGHT: u32 = 4800;

/// Renders page `page` (1-based) of `document` as a JPEG `width` pixels wide, or narrower when
/// the page would be taller than `MAX_RENDER_HEIGHT` (FR-10, FR-11).
pub fn render_page(document: &PdfDocument, page: u32, width: u32) -> Result<Vec<u8>> {
    let failed = |e: String| SplitError::RenderFailed(page, e);
    let page_count = u32::try_from(document.pages().len()).unwrap_or(0);
    if page == 0 || page > page_count {
        return Err(SplitError::PageOutOfRange(page, page_count));
    }
    let index = i32::try_from(page - 1).map_err(|e| failed(e.to_string()))?;
    let width = i32::try_from(width).map_err(|e| failed(e.to_string()))?;
    let max_height = i32::try_from(MAX_RENDER_HEIGHT).map_err(|e| failed(e.to_string()))?;
    let pdf_page = document.pages().get(index).map_err(|e| failed(e.to_string()))?;
    let config = PdfRenderConfig::new().set_target_width(width).set_maximum_height(max_height);
    let image = pdf_page
        .render_with_config(&config)
        .and_then(|bitmap| bitmap.as_image())
        .map_err(|e| failed(e.to_string()))?;
    let mut jpeg = Vec::new();
    image
        .to_rgb8()
        .write_to(&mut Cursor::new(&mut jpeg), ImageFormat::Jpeg)
        .map_err(|e| failed(e.to_string()))?;
    Ok(jpeg)
}

/// A damaged outline could link back to itself, so reading stops after this many bookmarks.
const MAX_BOOKMARKS: usize = 10_000;

/// Reads the text of every page and the top-level bookmarks of `document`, for the rule
/// strategies (FR-30 to FR-32). `progress` is called after each page with the pages done and the
/// total; returning `ControlFlow::Break` stops with `SplitError::ExtractCancelled` (NFR-02).
pub fn extract_text(
    document: &PdfDocument,
    mut progress: impl FnMut(u32, u32) -> ControlFlow<()>,
) -> Result<DocumentText> {
    let page_count = u32::try_from(document.pages().len()).unwrap_or(0);
    let mut pages = Vec::with_capacity(page_count as usize);
    for (index, page) in document.pages().iter().enumerate() {
        let number = index as u32 + 1;
        let text = page.text().map_err(|e| SplitError::TextFailed(number, e.to_string()))?;
        pages.push(text.all());
        if progress(number, page_count).is_break() {
            return Err(SplitError::ExtractCancelled);
        }
    }
    Ok(DocumentText { page_count, pages, outline: top_level_bookmarks(document) })
}

/// The top-level bookmarks that open a page of this document, in outline order. Bookmarks that
/// open a web address or another file are left out.
fn top_level_bookmarks(document: &PdfDocument) -> Vec<OutlineEntry> {
    let mut entries = Vec::new();
    let mut next = document.bookmarks().root();
    let mut read = 0;
    while let Some(bookmark) = next.filter(|_| read < MAX_BOOKMARKS) {
        read += 1;
        // A bookmark opens its page through `/Dest`, or through a GoTo action.
        let index = match bookmark.destination() {
            Some(dest) => dest.page_index().ok(),
            None => bookmark.action().and_then(|action| {
                action.as_local_destination_action()?.destination().ok()?.page_index().ok()
            }),
        };
        let page = index.and_then(|index| u32::try_from(index).ok()).map(|index| index + 1);
        if let Some(page) = page {
            let title = bookmark.title().unwrap_or_default().trim().to_string();
            entries.push(OutlineEntry { title, page });
        }
        next = bookmark.next_sibling();
    }
    entries
}

fn io_error(e: &std::io::Error, file_name: &str) -> SplitError {
    match e.kind() {
        ErrorKind::NotFound => SplitError::FileNotFound(file_name.to_string()),
        _ => SplitError::FileUnreadable(file_name.to_string()),
    }
}

/// Maps a PDFium load error to a message the user can act on.
fn load_error(e: PdfiumError, file_name: &str, had_password: bool) -> SplitError {
    match e {
        PdfiumError::PdfiumLibraryInternalError(internal) => match internal {
            PdfiumInternalError::PasswordError if had_password => SplitError::WrongPassword,
            PdfiumInternalError::PasswordError => SplitError::PasswordRequired,
            PdfiumInternalError::SecurityError => SplitError::UnsupportedSecurity,
            PdfiumInternalError::FormatError => SplitError::CorruptPdf(file_name.to_string()),
            PdfiumInternalError::FileError => SplitError::FileUnreadable(file_name.to_string()),
            other => SplitError::OpenFailed(file_name.to_string(), format!("{other:?}")),
        },
        PdfiumError::IoError(io) => io_error(&io, file_name),
        other => SplitError::OpenFailed(file_name.to_string(), other.to_string()),
    }
}

/// A PDF loaded with lopdf to write parts from (FR-50). Loaded once per export and read only;
/// each part is built from a copy, so the original file is never touched (FR-04).
pub struct Source {
    doc: Document,
    file_name: String,
}

/// Loads the PDF at `path` for writing parts. `password` unlocks an encrypted file (FR-03).
pub fn load_source(path: &str, password: Option<&str>) -> Result<Source> {
    let file_name = Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());
    let password = password.filter(|p| !p.is_empty());
    let loaded = match password {
        Some(p) => Document::load_with_password(path, p),
        None => Document::load(path),
    };
    let mut doc = loaded.map_err(|e| match e {
        lopdf::Error::IO(io) => io_error(&io, &file_name),
        lopdf::Error::InvalidPassword if password.is_some() => SplitError::WrongPassword,
        lopdf::Error::InvalidPassword => SplitError::PasswordRequired,
        other => SplitError::CantSplit(file_name.clone(), other.to_string()),
    })?;
    // Without the right password lopdf keeps the file encrypted instead of failing.
    if doc.is_encrypted() {
        return Err(if password.is_some() { SplitError::WrongPassword } else { SplitError::PasswordRequired });
    }
    // Loading decrypted the objects, so the parts are written without the original's protection.
    doc.encryption_state = None;
    if doc.get_pages().is_empty() {
        return Err(SplitError::EmptyPdf(file_name));
    }
    Ok(Source { doc, file_name })
}

impl Source {
    pub fn page_count(&self) -> u32 {
        u32::try_from(self.doc.get_pages().len()).unwrap_or(u32::MAX)
    }

    /// Writes pages `start..=end` (1-based) as a complete PDF to `out` (FR-50). Links and
    /// bookmarks that point inside the part are kept; ones that point to other pages are dropped
    /// (FR-54). The tagged-PDF structure tree and page labels are dropped, because they describe
    /// the whole document.
    pub fn write_part(&self, start: u32, end: u32, out: &mut impl std::io::Write) -> Result<()> {
        let mut doc = self.doc.clone();
        build_part(&mut doc, start, end)
            .map_err(|e| SplitError::CantSplit(self.file_name.clone(), e.to_string()))?;
        doc.save_to(out).map_err(|e| SplitError::ExportFailed(e.to_string()))
    }
}

/// Where a bookmark, link or open action goes.
enum Target {
    /// A page of this document.
    Page(ObjectId),
    /// A web address, another file, a script and so on: not tied to a page.
    Elsewhere,
    /// No destination, or one that can't be resolved.
    Nowhere,
}

type NamedDests = HashMap<Vec<u8>, ObjectId>;

/// Turns `doc` (a copy of the whole source) into a document with only pages `start..=end`.
fn build_part(doc: &mut Document, start: u32, end: u32) -> lopdf::Result<()> {
    let pages = doc.get_pages();
    let kept: Vec<ObjectId> = (start..=end).filter_map(|n| pages.get(&n).copied()).collect();
    let kept_set: HashSet<ObjectId> = kept.iter().copied().collect();
    let removed: HashSet<ObjectId> = pages.values().copied().filter(|id| !kept_set.contains(id)).collect();
    let named = named_destinations(doc);

    flatten_page_tree(doc, &kept)?;
    drop_outside_links(doc, &kept, &kept_set, &named);
    rebuild_outline(doc, &kept_set, &named)?;

    let open_action = doc.catalog()?.get(b"OpenAction").ok().map(|a| dest_or_action(doc, a, &named));
    let catalog = doc.catalog_mut()?;
    // The structure tree, page labels and article threads describe every page of the original.
    for key in [b"StructTreeRoot".as_slice(), b"MarkInfo", b"PageLabels", b"Threads"] {
        catalog.remove(key);
    }
    let open_outside = match open_action {
        Some(Target::Page(id)) => !kept_set.contains(&id),
        Some(Target::Nowhere) => true,
        Some(Target::Elsewhere) | None => false,
    };
    if open_outside {
        catalog.remove(b"OpenAction");
    }

    // Anything else still pointing at a removed page (named destinations, form fields) now points
    // nowhere, so the removed pages and everything only they use are pruned.
    doc.traverse_objects(|object| {
        if let Object::Reference(id) = object
            && removed.contains(id)
        {
            *object = Object::Null;
        }
    });
    doc.prune_objects();
    Ok(())
}

/// Makes the kept pages the only children of the root page tree node. Attributes that pages
/// inherit from their ancestors are copied onto each page first.
fn flatten_page_tree(doc: &mut Document, kept: &[ObjectId]) -> lopdf::Result<()> {
    const INHERITED: [&[u8]; 4] = [b"Resources", b"MediaBox", b"CropBox", b"Rotate"];
    let root_id = doc.catalog()?.get(b"Pages")?.as_reference()?;
    for &page_id in kept {
        let mut inherited: Vec<(&[u8], Object)> = Vec::new();
        let mut seen = HashSet::new();
        let mut parent = doc.get_dictionary(page_id)?.get(b"Parent").and_then(Object::as_reference).ok();
        while let Some(id) = parent.filter(|id| seen.insert(*id)) {
            let Ok(node) = doc.get_dictionary(id) else { break };
            for key in INHERITED {
                if let Ok(value) = node.get(key) {
                    inherited.push((key, value.clone()));
                }
            }
            parent = node.get(b"Parent").and_then(Object::as_reference).ok();
        }
        let page = doc.get_dictionary_mut(page_id)?;
        // Nearest ancestor first, so the first value found for a key wins.
        for (key, value) in inherited {
            if !page.has(key) {
                page.set(key, value);
            }
        }
        page.set("Parent", root_id);
    }
    let root = doc.get_dictionary_mut(root_id)?;
    root.set("Kids", kept.iter().map(|&id| Object::Reference(id)).collect::<Vec<_>>());
    root.set("Count", kept.len() as i64);
    root.remove(b"Parent");
    Ok(())
}

/// Removes link annotations on the kept pages that jump to a page outside the part.
fn drop_outside_links(doc: &mut Document, kept: &[ObjectId], kept_set: &HashSet<ObjectId>, named: &NamedDests) {
    for &page_id in kept {
        let Some(annots) = doc
            .get_dictionary(page_id)
            .ok()
            .and_then(|page| page.get(b"Annots").ok())
            .and_then(|a| doc.dereference(a).ok())
            .and_then(|(_, a)| a.as_array().ok())
        else {
            continue;
        };
        let remaining: Vec<Object> = annots
            .iter()
            .filter(|annot| {
                let Some(dict) = doc.dereference(annot).ok().and_then(|(_, a)| a.as_dict().ok()) else {
                    return true;
                };
                let is_link = dict.get(b"Subtype").and_then(Object::as_name).is_ok_and(|s| s == b"Link");
                !(is_link && matches!(target(doc, dict, named), Target::Page(id) if !kept_set.contains(&id)))
            })
            .cloned()
            .collect();
        if remaining.len() != annots.len()
            && let Ok(page) = doc.get_dictionary_mut(page_id)
        {
            page.set("Annots", remaining);
        }
    }
}

/// One bookmark and the bookmarks under it.
struct Bookmark {
    id: ObjectId,
    open: bool,
    children: Vec<Bookmark>,
}

/// Keeps the bookmarks that point inside the part, and their headings (FR-54). A heading whose own
/// page is outside the part but has bookmarks inside it now opens its first kept child's page.
fn rebuild_outline(doc: &mut Document, kept: &HashSet<ObjectId>, named: &NamedDests) -> lopdf::Result<()> {
    let Ok(outlines_id) = doc.catalog()?.get(b"Outlines").and_then(Object::as_reference) else {
        return Ok(());
    };
    let first = doc.get_dictionary(outlines_id).ok().and_then(|o| o.get(b"First").and_then(Object::as_reference).ok());
    let mut seen = HashSet::new();
    let tree = read_bookmarks(doc, first, &mut seen, 0);
    let tree = keep_bookmarks(doc, tree, kept, named);
    if tree.is_empty() {
        doc.catalog_mut()?.remove(b"Outlines");
        return Ok(());
    }
    let visible = link_bookmarks(doc, outlines_id, &tree);
    if let Ok(outlines) = doc.get_dictionary_mut(outlines_id) {
        outlines.set("Count", visible);
    }
    Ok(())
}

fn read_bookmarks(doc: &Document, mut next: Option<ObjectId>, seen: &mut HashSet<ObjectId>, depth: usize) -> Vec<Bookmark> {
    let mut items = Vec::new();
    // `seen` and the depth limit guard against looped or absurdly deep outlines in damaged files.
    while let Some(id) = next.filter(|id| depth < 64 && seen.insert(*id)) {
        let Ok(item) = doc.get_dictionary(id) else { break };
        let first = item.get(b"First").and_then(Object::as_reference).ok();
        let open = item.get(b"Count").and_then(Object::as_i64).is_ok_and(|c| c > 0);
        next = item.get(b"Next").and_then(Object::as_reference).ok();
        let children = read_bookmarks(doc, first, seen, depth + 1);
        items.push(Bookmark { id, open, children });
    }
    items
}

fn keep_bookmarks(doc: &mut Document, items: Vec<Bookmark>, kept: &HashSet<ObjectId>, named: &NamedDests) -> Vec<Bookmark> {
    let mut result = Vec::new();
    for mut item in items {
        item.children = keep_bookmarks(doc, std::mem::take(&mut item.children), kept, named);
        let Ok(dict) = doc.get_dictionary(item.id) else { continue };
        let keep_self = match target(doc, dict, named) {
            Target::Page(id) => kept.contains(&id),
            Target::Elsewhere => true,
            Target::Nowhere => false,
        };
        if keep_self {
            result.push(item);
        } else if !item.children.is_empty() {
            let first_page = item.children.iter().find_map(|child| match doc.get_dictionary(child.id) {
                Ok(child) => match target(doc, child, named) {
                    Target::Page(id) => Some(id),
                    _ => None,
                },
                Err(_) => None,
            });
            if let Ok(dict) = doc.get_dictionary_mut(item.id) {
                dict.remove(b"A");
                match first_page {
                    Some(page) => dict.set("Dest", vec![Object::Reference(page), "Fit".into()]),
                    None => {
                        dict.remove(b"Dest");
                    }
                }
            }
            result.push(item);
        }
    }
    result
}

/// Rewrites the First, Last, Next, Prev, Parent and Count links of `items` under `parent`, and
/// returns how many of them a viewer shows (the parent's Count).
fn link_bookmarks(doc: &mut Document, parent: ObjectId, items: &[Bookmark]) -> i64 {
    let mut visible = 0;
    for (i, item) in items.iter().enumerate() {
        let below = link_bookmarks(doc, item.id, &item.children);
        let Ok(dict) = doc.get_dictionary_mut(item.id) else { continue };
        dict.set("Parent", parent);
        match i.checked_sub(1).and_then(|p| items.get(p)) {
            Some(prev) => dict.set("Prev", prev.id),
            None => {
                dict.remove(b"Prev");
            }
        }
        match items.get(i + 1) {
            Some(next) => dict.set("Next", next.id),
            None => {
                dict.remove(b"Next");
            }
        }
        if item.children.is_empty() {
            for key in [b"First".as_slice(), b"Last", b"Count"] {
                dict.remove(key);
            }
        } else {
            dict.set("Count", if item.open { below } else { -below });
        }
        visible += 1 + if item.open { below } else { 0 };
    }
    if let (Some(first), Some(last)) = (items.first(), items.last())
        && let Ok(dict) = doc.get_dictionary_mut(parent)
    {
        dict.set("First", first.id);
        dict.set("Last", last.id);
    }
    visible
}

/// The target of a bookmark or link annotation: its `/Dest`, or else its `/A` action.
fn target(doc: &Document, item: &Dictionary, named: &NamedDests) -> Target {
    if let Ok(dest) = item.get(b"Dest") {
        return dest_or_action(doc, dest, named);
    }
    match item.get(b"A") {
        Ok(action) => dest_or_action(doc, action, named),
        Err(_) => Target::Nowhere,
    }
}

/// Resolves a destination (an array or a name) or an action dictionary.
fn dest_or_action(doc: &Document, object: &Object, named: &NamedDests) -> Target {
    let Ok((_, object)) = doc.dereference(object) else { return Target::Nowhere };
    match object {
        Object::Array(dest) => match dest.first() {
            Some(Object::Reference(id)) => Target::Page(*id),
            _ => Target::Nowhere,
        },
        Object::Name(name) | Object::String(name, _) => named.get(name).map_or(Target::Nowhere, |&id| Target::Page(id)),
        Object::Dictionary(action) => {
            let is_goto = action.get(b"S").and_then(Object::as_name).is_ok_and(|s| s == b"GoTo");
            match action.get(b"D") {
                Ok(dest) if is_goto => dest_or_action(doc, dest, named),
                _ if is_goto => Target::Nowhere,
                _ => Target::Elsewhere,
            }
        }
        _ => Target::Nowhere,
    }
}

/// Named destinations from the catalog's `/Dests` dictionary and `/Names /Dests` tree, mapped to
/// the page each one opens.
fn named_destinations(doc: &Document) -> NamedDests {
    fn page_of(doc: &Document, value: &Object) -> Option<ObjectId> {
        let (_, value) = doc.dereference(value).ok()?;
        let dest = match value {
            Object::Dictionary(d) => doc.dereference(d.get(b"D").ok()?).ok()?.1,
            other => other,
        };
        dest.as_array().ok()?.first()?.as_reference().ok()
    }
    fn walk(doc: &Document, node: &Object, out: &mut NamedDests, depth: usize) {
        let Some(node) = doc.dereference(node).ok().and_then(|(_, n)| n.as_dict().ok()) else { return };
        if let Ok(names) = node.get(b"Names").and_then(Object::as_array) {
            for pair in names.chunks(2) {
                if let [Object::String(key, _), value] = pair
                    && let Some(page) = page_of(doc, value)
                {
                    out.insert(key.clone(), page);
                }
            }
        }
        if depth < 32
            && let Ok(kids) = node.get(b"Kids").and_then(Object::as_array)
        {
            for kid in kids {
                walk(doc, kid, out, depth + 1);
            }
        }
    }

    let mut out = NamedDests::new();
    let Ok(catalog) = doc.catalog() else { return out };
    if let Some(dests) = catalog.get(b"Dests").ok().and_then(|d| doc.dereference(d).ok()).and_then(|(_, d)| d.as_dict().ok()) {
        for (key, value) in dests.iter() {
            if let Some(page) = page_of(doc, value) {
                out.insert(key.clone(), page);
            }
        }
    }
    if let Some(tree) = catalog
        .get(b"Names")
        .ok()
        .and_then(|n| doc.dereference(n).ok())
        .and_then(|(_, n)| n.as_dict().ok())
        .and_then(|n| n.get(b"Dests").ok())
    {
        walk(doc, tree, &mut out, 0);
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use lopdf::{Object, Stream, dictionary};
    use std::path::PathBuf;

    #[test]
    fn missing_library_gives_a_clear_error() {
        let err = load(Path::new("does-not-exist")).unwrap_err();
        assert!(err.contains("PDFium library is missing"), "{err}");
        assert!(err.contains("npm run fetch-pdfium"), "{err}");
    }

    /// Needs `npm run fetch-pdfium` first; CI runs it before `cargo test`.
    #[test]
    fn loads_the_bundled_library() {
        start_engine();
        let doc = engine().unwrap().create_new_pdf().unwrap();
        assert_eq!(doc.pages().len(), 0);
    }

    fn open(path: &str, password: Option<&str>) -> Result<PdfInfo> {
        open_document(path, password).map(|(info, _)| info)
    }

    pub(crate) fn start_engine() {
        init(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("pdfium")).unwrap();
    }

    /// A PDF with `pages` text pages. The trailer `/ID` is there because encryption needs it.
    pub(crate) fn document(pages: usize) -> Document {
        let mut doc = Document::with_version("1.5");
        doc.trailer.set(
            "ID",
            Object::Array(vec![
                Object::string_literal(b"0123456789abcdef"),
                Object::string_literal(b"fedcba9876543210"),
            ]),
        );
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
        });
        let kids: Vec<Object> = (1..=pages)
            .map(|n| {
                let content = format!("BT /F1 12 Tf 100 700 Td (Page {n}) Tj ET");
                let contents_id = doc.add_object(Stream::new(dictionary! {}, content.into_bytes()));
                doc.add_object(dictionary! {
                    "Type" => "Page",
                    "Parent" => pages_id,
                    "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
                    "Resources" => dictionary! { "Font" => dictionary! { "F1" => font_id } },
                    "Contents" => contents_id,
                })
                .into()
            })
            .collect();
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => pages as i64 }),
        );
        let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog_id);
        doc
    }

    pub(crate) fn encrypted(pages: usize, user_password: &str) -> Document {
        let mut doc = document(pages);
        let state = lopdf::EncryptionState::try_from(lopdf::EncryptionVersion::V2 {
            document: &doc,
            owner_password: "owner-secret",
            user_password,
            key_length: 128,
            permissions: lopdf::Permissions::all(),
        })
        .unwrap();
        doc.encrypt(&state).unwrap();
        doc
    }

    pub(crate) fn save(dir: &tempfile::TempDir, name: &str, mut doc: Document) -> String {
        let path = dir.path().join(name);
        doc.save(&path).unwrap();
        path.to_string_lossy().into_owned()
    }

    fn write(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> String {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn opens_a_normal_pdf() {
        start_engine();
        let dir = tempfile::tempdir().unwrap();
        let path = save(&dir, "report.pdf", document(3));

        let info = open(&path, None).unwrap();
        assert_eq!(info.file_name, "report.pdf");
        assert_eq!(info.page_count, 3);
        assert_eq!(info.size_bytes, std::fs::metadata(&path).unwrap().len());
        assert_eq!(info.path, path);
    }

    #[test]
    fn opening_leaves_the_original_byte_for_byte_unchanged() {
        start_engine();
        let dir = tempfile::tempdir().unwrap();
        let path = save(&dir, "original.pdf", document(2));
        let locked = save(&dir, "locked.pdf", encrypted(2, "secret"));
        let before = (std::fs::read(&path).unwrap(), std::fs::read(&locked).unwrap());

        open(&path, None).unwrap();
        open(&locked, Some("secret")).unwrap();
        let _ = open(&locked, Some("wrong"));

        assert_eq!(before, (std::fs::read(&path).unwrap(), std::fs::read(&locked).unwrap()));
    }

    #[test]
    fn encrypted_pdf_asks_for_a_password_then_opens_with_it() {
        start_engine();
        let dir = tempfile::tempdir().unwrap();
        let path = save(&dir, "locked.pdf", encrypted(4, "secret"));

        assert!(matches!(open(&path, None), Err(SplitError::PasswordRequired)));
        assert!(matches!(open(&path, Some("")), Err(SplitError::PasswordRequired)));
        assert!(matches!(open(&path, Some("wrong")), Err(SplitError::WrongPassword)));
        assert_eq!(open(&path, Some("secret")).unwrap().page_count, 4);
    }

    #[test]
    fn encrypted_pdf_without_a_user_password_opens_directly() {
        start_engine();
        let dir = tempfile::tempdir().unwrap();
        let path = save(&dir, "restricted.pdf", encrypted(2, ""));
        assert_eq!(open(&path, None).unwrap().page_count, 2);
    }

    #[test]
    fn corrupt_and_non_pdf_files_give_clear_errors() {
        start_engine();
        let dir = tempfile::tempdir().unwrap();
        let mut bytes = Vec::new();
        document(3).save_to(&mut bytes).unwrap();
        let truncated = write(&dir, "truncated.pdf", &bytes[..bytes.len() / 3]);
        let text = write(&dir, "notes.pdf", b"just some text, not a PDF");
        let empty = write(&dir, "empty.pdf", b"");

        for path in [&truncated, &text, &empty] {
            let err = open(path, None).unwrap_err();
            assert!(matches!(err, SplitError::CorruptPdf(_)), "{path}: {err}");
            assert!(err.to_string().ends_with("is damaged or is not a PDF"), "{err}");
        }
    }

    #[test]
    fn missing_file_and_folder_give_clear_errors() {
        start_engine();
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("gone.pdf");
        let err = open(&missing.to_string_lossy(), None).unwrap_err();
        assert_eq!(err.to_string(), "The file gone.pdf was not found. It may have been moved or deleted");

        let folder = open(&dir.path().to_string_lossy(), None).unwrap_err();
        assert!(matches!(folder, SplitError::CorruptPdf(_)), "{folder}");
    }

    #[test]
    fn renders_a_page_as_a_jpeg_of_the_requested_width() {
        start_engine();
        let dir = tempfile::tempdir().unwrap();
        let path = save(&dir, "report.pdf", document(2));
        let (_, doc) = open_document(&path, None).unwrap();

        let jpeg = render_page(&doc, 2, 200).unwrap();
        let image = image::load_from_memory_with_format(&jpeg, ImageFormat::Jpeg).unwrap();
        assert_eq!(image.width(), 200);
        // US Letter is 612 x 792 points, so the height keeps the page's aspect ratio.
        assert_eq!(image.height(), 259);
    }

    #[test]
    fn rendering_a_missing_page_gives_a_clear_error() {
        start_engine();
        let dir = tempfile::tempdir().unwrap();
        let (_, doc) = open_document(&save(&dir, "a.pdf", document(1)), None).unwrap();
        for page in [0, 5] {
            let err = render_page(&doc, page, 100).unwrap_err();
            assert!(matches!(err, SplitError::PageOutOfRange(p, 1) if p == page), "{err}");
        }
    }

    #[test]
    fn a_very_tall_page_renders_no_taller_than_the_limit() {
        start_engine();
        let dir = tempfile::tempdir().unwrap();
        let mut doc = document(1);
        // A 612 x 30,000 point page, like a long receipt.
        let page_id = doc.page_iter().next().unwrap();
        doc.get_object_mut(page_id)
            .and_then(Object::as_dict_mut)
            .unwrap()
            .set("MediaBox", vec![0.into(), 0.into(), 612.into(), 30_000.into()]);
        let (_, doc) = open_document(&save(&dir, "receipt.pdf", doc), None).unwrap();

        let jpeg = render_page(&doc, 1, 2400).unwrap();
        let image = image::load_from_memory_with_format(&jpeg, ImageFormat::Jpeg).unwrap();
        assert!(image.height() <= MAX_RENDER_HEIGHT, "{}", image.height());
        assert!(image.width() < 2400, "{}", image.width());
    }

    /// The interface shows a password prompt by matching these messages.
    #[test]
    fn password_messages_match_the_frontend() {
        let types = include_str!("../../src/types.ts");
        for err in [SplitError::PasswordRequired, SplitError::WrongPassword] {
            assert!(types.contains(&format!("\"{err}\"")), "src/types.ts is missing \"{err}\"");
        }
    }

    /// `document(4)` with bookmarks "Chapter 1" (page 1) > "Section 1.1" (page 2) and
    /// "Chapter 2" (page 3, closed) > "Section 2.1" (page 4, a named destination), and on page 1
    /// links to page 2, to page 4 (by name) and to a web address.
    pub(crate) fn outlined() -> Document {
        let mut doc = document(4);
        let pages = doc.get_pages();
        let page = |n: u32| Object::Reference(pages[&n]);
        let outlines_id = doc.new_object_id();
        let ids: Vec<ObjectId> = (0..4).map(|_| doc.new_object_id()).collect();
        let item = |title: &str, parent: ObjectId, dest: Object| {
            dictionary! { "Title" => Object::string_literal(title), "Parent" => parent, "Dest" => dest }
        };
        let mut items = [
            item("Chapter 1", outlines_id, vec![page(1), "Fit".into()].into()),
            item("Section 1.1", ids[0], vec![page(2), "Fit".into()].into()),
            item("Chapter 2", outlines_id, vec![page(3), "Fit".into()].into()),
            item("Section 2.1", ids[2], Object::string_literal("sec-2.1")),
        ];
        items[0].set("Next", ids[2]);
        items[0].set("First", ids[1]);
        items[0].set("Last", ids[1]);
        items[0].set("Count", 1);
        items[2].set("Prev", ids[0]);
        items[2].set("First", ids[3]);
        items[2].set("Last", ids[3]);
        items[2].set("Count", -1);
        for (id, dict) in ids.iter().zip(items) {
            doc.objects.insert(*id, Object::Dictionary(dict));
        }
        doc.objects.insert(
            outlines_id,
            Object::Dictionary(dictionary! { "Type" => "Outlines", "First" => ids[0], "Last" => ids[2], "Count" => 3 }),
        );
        let link = |action: Object| {
            Object::Dictionary(dictionary! {
                "Type" => "Annot", "Subtype" => "Link",
                "Rect" => vec![0.into(), 0.into(), 50.into(), 50.into()], "A" => action,
            })
        };
        let annots = vec![
            doc.add_object(link(dictionary! { "S" => "GoTo", "D" => vec![page(2), "Fit".into()] }.into())).into(),
            doc.add_object(link(dictionary! { "S" => "GoTo", "D" => Object::string_literal("sec-2.1") }.into())).into(),
            doc.add_object(link(dictionary! { "S" => "URI", "URI" => Object::string_literal("https://example.com") }.into())).into(),
        ];
        doc.get_dictionary_mut(pages[&1]).unwrap().set("Annots", annots);
        let dests = doc.add_object(dictionary! {
            "Names" => vec![Object::string_literal("sec-2.1"), vec![page(4), "Fit".into()].into()],
        });
        let catalog = doc.catalog_mut().unwrap();
        catalog.set("Outlines", outlines_id);
        catalog.set("Names", dictionary! { "Dests" => dests });
        catalog.set("StructTreeRoot", dictionary! { "Type" => "StructTreeRoot" });
        doc
    }

    fn part(source: &Source, start: u32, end: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        source.write_part(start, end, &mut bytes).unwrap();
        bytes
    }

    fn source(doc: Document) -> Source {
        Source { doc, file_name: "test.pdf".into() }
    }

    /// The text of each page, read by PDFium: a second, independent reader of the written file.
    fn page_texts(bytes: &[u8]) -> Vec<String> {
        start_engine();
        let doc = engine().unwrap().load_pdf_from_byte_slice(bytes, None).unwrap();
        doc.pages().iter().map(|p| p.text().unwrap().all().trim().to_string()).collect()
    }

    /// Bookmark titles as (depth, title, page number in the part, Count).
    fn bookmarks(bytes: &[u8]) -> Vec<(usize, String, Option<u32>, Option<i64>)> {
        fn walk(doc: &Document, first: Option<ObjectId>, depth: usize, out: &mut Vec<(usize, String, Option<u32>, Option<i64>)>) {
            let numbers: HashMap<ObjectId, u32> = doc.get_pages().into_iter().map(|(n, id)| (id, n)).collect();
            let mut next = first;
            while let Some(id) = next {
                let item = doc.get_dictionary(id).unwrap();
                let title = String::from_utf8_lossy(item.get(b"Title").unwrap().as_str().unwrap()).into_owned();
                let page = item
                    .get(b"Dest")
                    .ok()
                    .and_then(|d| d.as_array().ok())
                    .and_then(|d| d[0].as_reference().ok())
                    .and_then(|p| numbers.get(&p).copied());
                out.push((depth, title, page, item.get(b"Count").and_then(Object::as_i64).ok()));
                assert!(item.get(b"Parent").unwrap().as_reference().is_ok());
                walk(doc, item.get(b"First").and_then(Object::as_reference).ok(), depth + 1, out);
                next = item.get(b"Next").and_then(Object::as_reference).ok();
            }
        }
        let doc = Document::load_mem(bytes).unwrap();
        let mut out = Vec::new();
        if let Ok(outlines) = doc.catalog().unwrap().get(b"Outlines").and_then(Object::as_reference) {
            let first = doc.get_dictionary(outlines).unwrap().get(b"First").and_then(Object::as_reference).ok();
            walk(&doc, first, 0, &mut out);
        }
        out
    }

    #[test]
    fn a_part_has_exactly_its_pages() {
        let src = source(document(5));
        assert_eq!(page_texts(&part(&src, 2, 4)), ["Page 2", "Page 3", "Page 4"]);
        assert_eq!(page_texts(&part(&src, 5, 5)), ["Page 5"]);
        assert_eq!(page_texts(&part(&src, 1, 5)).len(), 5);
    }

    #[test]
    fn content_of_other_pages_is_not_in_the_part() {
        let bytes = part(&source(outlined()), 1, 2);
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("(Page 1)") && text.contains("(Page 2)"));
        assert!(!text.contains("(Page 3)") && !text.contains("(Page 4)"), "removed pages were kept");
        assert!(!text.contains("StructTreeRoot"));
    }

    #[test]
    fn bookmarks_inside_the_part_are_kept() {
        let src = source(outlined());
        let b = |d, t: &str, p, c| (d, t.to_string(), p, c);
        assert_eq!(
            bookmarks(&part(&src, 1, 4)),
            [b(0, "Chapter 1", Some(1), Some(1)), b(1, "Section 1.1", Some(2), None), b(0, "Chapter 2", Some(3), Some(-1)), b(1, "Section 2.1", None, None)]
        );
        assert_eq!(bookmarks(&part(&src, 1, 2)), [b(0, "Chapter 1", Some(1), Some(1)), b(1, "Section 1.1", Some(2), None)]);
        assert_eq!(bookmarks(&part(&src, 3, 3)), [b(0, "Chapter 2", Some(1), None)]);
    }

    #[test]
    fn a_heading_outside_the_part_opens_its_first_kept_child() {
        let bytes = part(&source(outlined()), 2, 3);
        let b = |d, t: &str, p, c| (d, t.to_string(), p, c);
        assert_eq!(bookmarks(&bytes), [b(0, "Chapter 1", Some(1), Some(1)), b(1, "Section 1.1", Some(1), None), b(0, "Chapter 2", Some(2), None)]);
    }

    #[test]
    fn a_part_without_bookmarked_pages_has_no_outline() {
        let mut doc = outlined();
        let pages = doc.get_pages();
        // Point "Section 2.1" at page 3 directly, so page 4 has no bookmark.
        let id = doc.get_dictionary(doc.catalog().unwrap().get(b"Outlines").unwrap().as_reference().unwrap()).unwrap();
        let chapter2 = id.get(b"Last").unwrap().as_reference().unwrap();
        let section = doc.get_dictionary(chapter2).unwrap().get(b"First").unwrap().as_reference().unwrap();
        doc.get_dictionary_mut(section).unwrap().set("Dest", vec![Object::Reference(pages[&3]), "Fit".into()]);
        let bytes = part(&source(doc), 4, 4);
        let doc = Document::load_mem(&bytes).unwrap();
        assert!(doc.catalog().unwrap().get(b"Outlines").is_err());
    }

    #[test]
    fn links_to_pages_outside_the_part_are_dropped() {
        let src = source(outlined());
        let links = |bytes: &[u8]| {
            let doc = Document::load_mem(bytes).unwrap();
            let first = doc.get_pages()[&1];
            doc.get_page_annotations(first).unwrap().len()
        };
        assert_eq!(links(&part(&src, 1, 4)), 3);
        assert_eq!(links(&part(&src, 1, 2)), 2, "the link to page 4 should be gone");
        assert_eq!(links(&part(&src, 1, 1)), 1, "only the web link should be left");
    }

    #[test]
    fn inherited_page_attributes_survive_flattening() {
        let mut doc = document(3);
        let pages = doc.get_pages();
        let root = doc.catalog().unwrap().get(b"Pages").unwrap().as_reference().unwrap();
        // Move pages 2 and 3 under a middle node that holds their MediaBox and Resources.
        let resources = doc.get_dictionary(pages[&2]).unwrap().get(b"Resources").unwrap().clone();
        let middle = doc.add_object(dictionary! {
            "Type" => "Pages", "Parent" => root, "Count" => 2,
            "Kids" => vec![pages[&2].into(), pages[&3].into()],
            "MediaBox" => vec![0.into(), 0.into(), 300.into(), 792.into()], "Resources" => resources,
        });
        for n in [2, 3] {
            let page = doc.get_dictionary_mut(pages[&n]).unwrap();
            page.remove(b"MediaBox");
            page.remove(b"Resources");
            page.set("Parent", middle);
        }
        doc.get_dictionary_mut(root).unwrap().set("Kids", vec![pages[&1].into(), middle.into()]);

        let bytes = part(&source(doc), 3, 3);
        assert_eq!(page_texts(&bytes), ["Page 3"]);
        let doc = engine().unwrap().load_pdf_from_byte_slice(&bytes, None).unwrap();
        assert_eq!(doc.pages().get(0).unwrap().width().value, 300.0);
    }

    #[test]
    fn an_encrypted_source_needs_its_password_and_gives_open_parts() {
        let dir = tempfile::tempdir().unwrap();
        let path = save(&dir, "locked.pdf", encrypted(3, "secret"));

        assert!(matches!(load_source(&path, None), Err(SplitError::PasswordRequired)));
        assert!(matches!(load_source(&path, Some("wrong")), Err(SplitError::WrongPassword)));
        let src = load_source(&path, Some("secret")).unwrap();
        assert_eq!(src.page_count(), 3);
        assert_eq!(page_texts(&part(&src, 2, 3)), ["Page 2", "Page 3"]);
    }

    #[test]
    fn missing_and_corrupt_sources_give_clear_errors() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("gone.pdf").to_string_lossy().into_owned();
        assert!(matches!(load_source(&missing, None), Err(SplitError::FileNotFound(_))));
        let junk = write(&dir, "junk.pdf", b"not a pdf");
        let err = load_source(&junk, None).err().unwrap();
        assert!(matches!(err, SplitError::CantSplit(..)), "{err}");
    }

    fn extract(doc: Document) -> DocumentText {
        start_engine();
        let dir = tempfile::tempdir().unwrap();
        let (_, doc) = open_document(&save(&dir, "a.pdf", doc), None).unwrap();
        extract_text(&doc, |_, _| ControlFlow::Continue(())).unwrap()
    }

    fn entry(title: &str, page: u32) -> OutlineEntry {
        OutlineEntry { title: title.into(), page }
    }

    #[test]
    fn extracts_the_text_of_every_page() {
        let text = extract(document(3));
        assert_eq!(text.page_count, 3);
        let pages: Vec<&str> = text.pages.iter().map(|p| p.trim()).collect();
        assert_eq!(pages, ["Page 1", "Page 2", "Page 3"]);
        assert!(text.outline.is_empty());
    }

    #[test]
    fn extracts_only_the_top_level_bookmarks() {
        assert_eq!(extract(outlined()).outline, [entry("Chapter 1", 1), entry("Chapter 2", 3)]);
    }

    #[test]
    fn bookmarks_with_actions_and_named_destinations_find_their_page() {
        let mut doc = outlined();
        let pages = doc.get_pages();
        let outlines = doc.get_dictionary(doc.catalog().unwrap().get(b"Outlines").unwrap().as_reference().unwrap()).unwrap();
        let (chapter1, chapter2) = (
            outlines.get(b"First").unwrap().as_reference().unwrap(),
            outlines.get(b"Last").unwrap().as_reference().unwrap(),
        );
        let item = doc.get_dictionary_mut(chapter1).unwrap();
        item.remove(b"Dest");
        item.set("A", dictionary! { "S" => "GoTo", "D" => vec![Object::Reference(pages[&2]), "Fit".into()] });
        doc.get_dictionary_mut(chapter2).unwrap().set("Dest", Object::string_literal("sec-2.1"));

        assert_eq!(extract(doc).outline, [entry("Chapter 1", 2), entry("Chapter 2", 4)]);
    }

    #[test]
    fn bookmarks_that_open_no_page_are_left_out() {
        let mut doc = outlined();
        let outlines = doc.get_dictionary(doc.catalog().unwrap().get(b"Outlines").unwrap().as_reference().unwrap()).unwrap();
        let chapter1 = outlines.get(b"First").unwrap().as_reference().unwrap();
        let item = doc.get_dictionary_mut(chapter1).unwrap();
        item.remove(b"Dest");
        item.set("A", dictionary! { "S" => "URI", "URI" => Object::string_literal("https://example.com") });

        assert_eq!(extract(doc).outline, [entry("Chapter 2", 3)]);
    }

    #[test]
    fn extraction_reports_progress_and_can_be_cancelled() {
        start_engine();
        let dir = tempfile::tempdir().unwrap();
        let (_, doc) = open_document(&save(&dir, "a.pdf", document(5)), None).unwrap();

        let mut seen = Vec::new();
        let result = extract_text(&doc, |done, total| {
            seen.push((done, total));
            if done == 2 { ControlFlow::Break(()) } else { ControlFlow::Continue(()) }
        });
        assert!(matches!(result, Err(SplitError::ExtractCancelled)));
        assert_eq!(seen, [(1, 5), (2, 5)]);
    }

    /// NFR-01: a 300-page file extracts well inside the 15 s budget.
    #[test]
    fn a_300_page_file_extracts_well_inside_the_budget() {
        let mut doc = document(300);
        // Fill each page with 50 lines, like a page of a report.
        for (n, page_id) in doc.get_pages() {
            let lines: String = (0..50).map(|l| format!("0 -14 Td (Line {l} of page {n}, some ordinary report text.) Tj ")).collect();
            let content = format!("BT /F1 10 Tf 50 750 Td {lines}ET");
            let contents_id = doc.add_object(Stream::new(dictionary! {}, content.into_bytes()));
            doc.get_dictionary_mut(page_id).unwrap().set("Contents", contents_id);
        }
        let started = std::time::Instant::now();
        let text = extract(doc);
        let took = started.elapsed();
        assert_eq!(text.pages.len(), 300);
        assert!(text.pages[299].contains("Line 49 of page 300"), "{}", text.pages[299]);
        assert!(took < std::time::Duration::from_secs(5), "took {took:?}");
    }
}
