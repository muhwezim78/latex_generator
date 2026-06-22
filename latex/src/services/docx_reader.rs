//! Parse a `.docx` file into a [`Document`] AST.
//!
//! A `.docx` is a ZIP archive.  This reader:
//!   1. Opens the ZIP and reads `word/document.xml`.
//!   2. Optionally reads `word/styles.xml` to resolve style names.
//!   3. Stream-parses the XML with `quick-xml`.
//!   4. Copies `word/media/*` images to `<output_dir>/media/`.
//!   5. Returns a fully populated [`Document`].

use std::{
    collections::HashMap,
    io::{Cursor, Read},
    path::Path,
};

use quick_xml::{events::Event, Reader};
use zip::ZipArchive;

use crate::models::{Cell, DocError, Document, Element, Run};

use crate::services::normalizer;

// ── Public entry point ────────────────────────────────────────────────────────

/// Parse `docx_path` into a `Document` and copy media into `output_dir/media/`.
pub fn parse(docx_path: &Path, output_dir: &Path) -> Result<Document, DocError> {
    // Read the entire archive into memory so we can iterate it multiple times.
    let bytes = std::fs::read(docx_path)?;
    let cursor = Cursor::new(bytes.as_slice());
    let mut archive = ZipArchive::new(cursor)?;

    // Extract media files first.
    extract_media(&mut archive, output_dir)?;

    // Read relationships for resolving images.
    let rel_map = read_relationships(&mut archive);

    // Read styles (best-effort — errors are non-fatal).
    let style_map = read_styles(&mut archive).unwrap_or_default();

    // Read and parse the main document body.
    let xml = read_entry(&mut archive, "word/document.xml")?;
    let mut doc = parse_document_xml(&xml, &style_map, &rel_map)?;

    // Run AST normalisation passes (dedup page breaks, heading promotion, TOC
    // detection, bullet promotion).
    doc.elements = normalizer::normalize(doc.elements);

    Ok(doc)
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Read a named ZIP entry into a UTF-8 string.
fn read_entry(archive: &mut ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<String, DocError> {
    let mut entry = archive
        .by_name(name)
        .map_err(|_| DocError::Missing(name.to_owned()))?;
    let mut buf = String::new();
    entry.read_to_string(&mut buf)?;
    Ok(buf)
}

/// Copy `word/media/*` to `<output_dir>/media/`.
fn extract_media(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    output_dir: &Path,
) -> Result<(), DocError> {
    let media_dir = output_dir.join("media");
    let names: Vec<String> = (0..archive.len())
        .filter_map(|i| {
            archive.by_index(i).ok().and_then(|f| {
                let n = f.name().to_owned();
                if n.starts_with("word/media/") { Some(n) } else { None }
            })
        })
        .collect();

    if names.is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(&media_dir)?;

    for name in names {
        let mut entry = archive.by_name(&name).map_err(DocError::Zip)?;
        let file_name = Path::new(&name)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".to_owned());
        let dest = media_dir.join(&file_name);
        let mut data = Vec::new();
        entry.read_to_end(&mut data)?;
        std::fs::write(dest, data)?;
    }
    Ok(())
}

/// Build a map from style ID → style name from `word/styles.xml`.
/// Returns an empty map if the entry is missing or unparseable.
fn read_styles(archive: &mut ZipArchive<Cursor<&[u8]>>) -> Result<HashMap<String, String>, DocError> {
    let xml = match archive.by_name("word/styles.xml") {
        Ok(mut e) => {
            let mut s = String::new();
            e.read_to_string(&mut s)?;
            s
        }
        Err(_) => return Ok(HashMap::new()),
    };

    let mut map = HashMap::new();
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);

    let mut current_id = String::new();
    let mut current_name = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let local = local_name(&e.name());
                match local.as_str() {
                    "style" => {
                        current_id.clear();
                        current_name.clear();
                        for attr in e.attributes().flatten() {
                            if local_name(&attr.key) == "styleId" {
                                current_id = String::from_utf8_lossy(&attr.value).into_owned();
                            }
                        }
                    }
                    "name" => {
                        for attr in e.attributes().flatten() {
                            if local_name(&attr.key) == "val" {
                                current_name = String::from_utf8_lossy(&attr.value).into_owned();
                            }
                        }
                        if !current_id.is_empty() && !current_name.is_empty() {
                            map.insert(current_id.clone(), current_name.clone());
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    Ok(map)
}

// ── document.xml parser ───────────────────────────────────────────────────────

/// Main parser state machine over `word/document.xml`.
fn parse_document_xml(
    xml: &str,
    style_map: &HashMap<String, String>,
    rel_map: &HashMap<String, String>,
) -> Result<Document, DocError> {
    let mut doc = Document::default();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    // Parser state
    let mut elements: Vec<Element> = Vec::new();

    // Current paragraph
    let mut para_runs: Vec<Run> = Vec::new();
    let mut para_style: Option<String> = None;

    // List tracking
    let mut para_has_numpr = false;
    let list_ordered = false;
    let mut list_level: u8 = 0;
    let mut list_items: Vec<Vec<Run>> = Vec::new();

    // Table tracking
    let mut _in_table = false;
    let mut table_rows: Vec<Vec<Cell>> = Vec::new();
    let mut current_row: Vec<Cell> = Vec::new();
    let mut current_cell_runs: Vec<Run> = Vec::new();
    let mut in_cell = false;

    // Run tracking
    let mut current_run = Run::default();
    let mut in_run = false;
    let mut in_text = false;

    // Relationship tracking (for hyperlinks — simplified)
    let mut _in_hyperlink = false;
    let mut hyperlink_url: Option<String> = None;

    // Image tracking
    let mut pending_images: Vec<ImageData> = Vec::new();
    let mut current_drawing = ImageData::default();
    let mut in_drawing = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let local = local_name(&e.name());
                match local.as_str() {
                    // ── Table ──────────────────────────────────────────────
                    "tbl" => {
                        _in_table = true;
                        table_rows.clear();
                    }
                    "tr" => {
                        current_row.clear();
                    }
                    "tc" => {
                        in_cell = true;
                        current_cell_runs.clear();
                    }
                    // ── Paragraph ──────────────────────────────────────────
                    "p" => {
                        para_runs.clear();
                        para_style = None;
                        para_has_numpr = false;
                    }
                    // ── Paragraph properties ───────────────────────────────
                    "pStyle" => {
                        for attr in e.attributes().flatten() {
                            if local_name(&attr.key) == "val" {
                                let style_id =
                                    String::from_utf8_lossy(&attr.value).into_owned();
                                let resolved = style_map
                                    .get(&style_id)
                                    .cloned()
                                    .unwrap_or(style_id);
                                para_style = Some(resolved);
                            }
                        }
                    }
                    // ── Numbered list ──────────────────────────────────────
                    "numPr" => {
                        para_has_numpr = true;
                    }
                    "ilvl" => {
                        for attr in e.attributes().flatten() {
                            if local_name(&attr.key) == "val"
                                && let Ok(n) = String::from_utf8_lossy(&attr.value).parse::<u8>() {
                                    list_level = n;
                                }
                        }
                    }
                    // ── Run ────────────────────────────────────────────────
                    "r" => {
                        in_run = true;
                        current_run = Run::default();
                        if let Some(ref url) = hyperlink_url {
                            current_run.hyperlink = Some(url.clone());
                        }
                    }
                    // ── Run properties ─────────────────────────────────────
                    "b" => {
                        if in_run {
                            current_run.bold = is_truthy(&e);
                        }
                    }
                    "i" => {
                        if in_run {
                            current_run.italic = is_truthy(&e);
                        }
                    }
                    "u" => {
                        if in_run {
                            current_run.underline = is_truthy(&e);
                        }
                    }
                    "vertAlign" => {
                        if in_run {
                            for attr in e.attributes().flatten() {
                                if local_name(&attr.key) == "val" {
                                    match &*String::from_utf8_lossy(&attr.value) {
                                        "superscript" => current_run.superscript = true,
                                        "subscript" => current_run.subscript = true,
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                    "rFonts" => {
                        // Detect monospace fonts (Courier, Consolas, etc.)
                        for attr in e.attributes().flatten() {
                            let val = String::from_utf8_lossy(&attr.value).to_lowercase();
                            if val.contains("courier") || val.contains("consolas")
                                || val.contains("mono") || val.contains("typewriter")
                            {
                                current_run.mono = true;
                            }
                        }
                    }
                    // ── Text ───────────────────────────────────────────────
                    "t" => {
                        in_text = true;
                    }
                    // ── Hyperlink ──────────────────────────────────────────
                    "hyperlink" => {
                        _in_hyperlink = true;
                        // The URL lives in a relationship — store placeholder;
                        // a full implementation would pre-read word/_rels/.
                        for attr in e.attributes().flatten() {
                            if local_name(&attr.key) == "id" {
                                let rid = String::from_utf8_lossy(&attr.value).into_owned();
                                hyperlink_url = Some(
                                    rel_map
                                        .get(&rid)
                                        .cloned()
                                        .unwrap_or_else(|| format!("rel:{}", rid)),
                                );
                            }
                        }
                    }
                    // ── Images & Drawings ──────────────────────────────────
                    "drawing" => {
                        in_drawing = true;
                        current_drawing = ImageData::default();
                    }
                    "extent" => {
                        if in_drawing {
                            for attr in e.attributes().flatten() {
                                if local_name(&attr.key) == "cx"
                                    && let Ok(emu) = String::from_utf8_lossy(&attr.value).parse::<u64>() {
                                        current_drawing.width_cm = Some(emu_to_cm(emu));
                                    }
                            }
                        }
                    }
                    "docPr" => {
                        if in_drawing {
                            let mut descr = None::<String>;
                            let mut title = None::<String>;
                            let mut name = None::<String>;
                            for attr in e.attributes().flatten() {
                                match local_name(&attr.key).as_str() {
                                    "descr" => descr = Some(String::from_utf8_lossy(&attr.value).into_owned()),
                                    "title" => title = Some(String::from_utf8_lossy(&attr.value).into_owned()),
                                    "name" => name = Some(String::from_utf8_lossy(&attr.value).into_owned()),
                                    _ => {}
                                }
                            }
                            current_drawing.caption = descr
                                .filter(|s| !s.is_empty())
                                .or_else(|| title.filter(|s| !s.is_empty()))
                                .or_else(|| name.filter(|s| !s.is_empty()));
                        }
                    }
                    "blip"
                        if in_drawing => {
                            for attr in e.attributes().flatten() {
                                if local_name(&attr.key) == "embed" {
                                    let rid = String::from_utf8_lossy(&attr.value).into_owned();
                                    let path = rel_map
                                        .get(&rid)
                                        .cloned()
                                        .unwrap_or_else(|| format!("media/{rid}")); // graceful fallback
                                    current_drawing.path = Some(path);
                                }
                            }
                        }
                    _ => {}
                }
            }

            Ok(Event::Empty(e)) => {
                let local = local_name(&e.name());
                match local.as_str() {
                    "br" => {
                        // Page break or line break
                        for attr in e.attributes().flatten() {
                            if local_name(&attr.key) == "type" && &*String::from_utf8_lossy(&attr.value) == "page" {
                                elements.push(Element::PageBreak);
                            }
                        }
                    }
                    "tab" => {
                        if in_run {
                            current_run.text.push(' ');
                        }
                    }
                    "b" => {
                        if in_run {
                            current_run.bold = is_truthy(&e);
                        }
                    }
                    "i" => {
                        if in_run {
                            current_run.italic = is_truthy(&e);
                        }
                    }
                    "u" => {
                        if in_run {
                            current_run.underline = is_truthy(&e);
                        }
                    }
                    "extent" => {
                        if in_drawing {
                            for attr in e.attributes().flatten() {
                                if local_name(&attr.key) == "cx"
                                    && let Ok(emu) = String::from_utf8_lossy(&attr.value).parse::<u64>() {
                                        current_drawing.width_cm = Some(emu_to_cm(emu));
                                    }
                            }
                        }
                    }
                    "docPr" => {
                        if in_drawing {
                            let mut descr = None::<String>;
                            let mut title = None::<String>;
                            let mut name = None::<String>;
                            for attr in e.attributes().flatten() {
                                match local_name(&attr.key).as_str() {
                                    "descr" => descr = Some(String::from_utf8_lossy(&attr.value).into_owned()),
                                    "title" => title = Some(String::from_utf8_lossy(&attr.value).into_owned()),
                                    "name" => name = Some(String::from_utf8_lossy(&attr.value).into_owned()),
                                    _ => {}
                                }
                            }
                            current_drawing.caption = descr
                                .filter(|s| !s.is_empty())
                                .or_else(|| title.filter(|s| !s.is_empty()))
                                .or_else(|| name.filter(|s| !s.is_empty()));
                        }
                    }
                    "blip"
                        if in_drawing => {
                            for attr in e.attributes().flatten() {
                                if local_name(&attr.key) == "embed" {
                                    let rid = String::from_utf8_lossy(&attr.value).into_owned();
                                    let path = rel_map
                                        .get(&rid)
                                        .cloned()
                                        .unwrap_or_else(|| format!("media/{rid}")); // graceful fallback
                                    current_drawing.path = Some(path);
                                }
                            }
                        }
                    _ => {}
                }
            }

            Ok(Event::Text(e)) => {
                if in_text && in_run {
                    let text = e.unescape().unwrap_or_default().into_owned();
                    current_run.text.push_str(&text);
                }
            }

            Ok(Event::End(e)) => {
                let local = local_name(&e.name());
                match local.as_str() {
                    // ── Finish text span ───────────────────────────────────
                    "t" => {
                        in_text = false;
                    }
                    // ── Finish run ─────────────────────────────────────────
                    "r" => {
                        in_run = false;
                        if !current_run.text.is_empty() {
                            let run = current_run.clone();
                            if in_cell {
                                current_cell_runs.push(run);
                            } else {
                                para_runs.push(run);
                            }
                        }
                        current_run = Run::default();
                    }
                    // ── Finish hyperlink ───────────────────────────────────
                    "hyperlink" => {
                        _in_hyperlink = false;
                        hyperlink_url = None;
                    }
                    // ── Finish cell ────────────────────────────────────────
                    "tc" => {
                        in_cell = false;
                        current_row.push(Cell {
                            runs: std::mem::take(&mut current_cell_runs),
                        });
                    }
                    // ── Finish table row ───────────────────────────────────
                    "tr" => {
                        if !current_row.is_empty() {
                            table_rows.push(std::mem::take(&mut current_row));
                        }
                    }
                    // ── Finish table ───────────────────────────────────────
                    "tbl" => {
                        _in_table = false;
                        if !table_rows.is_empty() {
                            elements.push(Element::Table {
                                rows: std::mem::take(&mut table_rows),
                            });
                        }
                    }
                    // ── Finish drawing ─────────────────────────────────────
                    "drawing" => {
                        if in_drawing {
                            in_drawing = false;
                            if let Some(path) = current_drawing.path.take() {
                                pending_images.push(ImageData {
                                    path: Some(path),
                                    width_cm: current_drawing.width_cm.take(),
                                    caption: current_drawing.caption.take(),
                                });
                            }
                            current_drawing = ImageData::default();
                        }
                    }
                    // ── Finish paragraph ───────────────────────────────────
                    "p" => {
                        // Handle inline images
                        for img in pending_images.drain(..) {
                            if let Some(path) = img.path {
                                elements.push(Element::Image {
                                    path,
                                    width_cm: img.width_cm,
                                    caption: img.caption,
                                });
                            }
                        }

                        let runs = std::mem::take(&mut para_runs);
                        let style = para_style.take();

                        // Determine heading level
                        let heading_level = style.as_deref().and_then(heading_level_for_style);

                        // If it's a heading, it breaks any list.
                        // If it's NOT a heading, and it DOES NOT have numPr, it breaks any list.
                        let is_list_item = para_has_numpr && heading_level.is_none();

                        if !is_list_item && !list_items.is_empty() {
                            flush_list(
                                &mut elements,
                                &mut list_items,
                                list_ordered,
                                list_level,
                            );
                        }

                        if let Some(level) = heading_level {
                            // Set document title from first H1
                            if level == 1 && doc.title.is_none() {
                                let title_text: String =
                                    runs.iter().map(|r| r.text.as_str()).collect();
                                if !title_text.is_empty() {
                                    doc.title = Some(title_text);
                                }
                            }
                            elements.push(Element::Heading { level, runs });
                        } else if is_list_item {
                            list_items.push(runs);
                        } else if in_cell {
                            if !runs.is_empty() {
                                if !current_cell_runs.is_empty() {
                                    current_cell_runs.push(Run { text: " ".to_string(), ..Default::default() });
                                }
                                current_cell_runs.extend(runs);
                            }
                        } else {
                            if !runs.is_empty() || !elements.is_empty() {
                                let mut text_str: String = runs.iter().map(|r| r.text.as_str()).collect();
                                let upper = text_str.trim().to_uppercase();
                                if doc.title.is_none() && upper.starts_with("TITLE:") {
                                    // Extract title and do not emit as paragraph
                                    let mut title_text = text_str.trim()[6..].trim().to_string();
                                    if !title_text.is_empty() {
                                        doc.title = Some(title_text);
                                    }
                                } else {
                                    elements.push(Element::Paragraph { runs });
                                }
                            }
                        }
                    }
                    // ── Finish numPr (list item) ───────────────────────────
                    "numPr" => {
                        // handled at paragraph start
                    }
                    _ => {}
                }
            }

            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(DocError::Xml(e.to_string()));
            }
            _ => {}
        }
    }

    // Flush any trailing list
    if !list_items.is_empty() {
        flush_list(&mut elements, &mut list_items, list_ordered, list_level);
    }

    doc.elements = elements;
    Ok(doc)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Map a resolved Word style name to a heading level (1–4), or `None`.
fn heading_level_for_style(name: &str) -> Option<u8> {
    let lower = name.to_lowercase();
    // Match "heading 1" .. "heading 4" and common variants.
    if lower.contains("heading 1") || lower.contains("heading1") || lower == "h1" || lower == "title" {
        Some(1)
    } else if lower.contains("heading 2") || lower.contains("heading2") || lower == "h2" || lower == "subtitle" {
        Some(2)
    } else if lower.contains("heading 3") || lower.contains("heading3") || lower == "h3" {
        Some(3)
    } else if lower.contains("heading 4") || lower.contains("heading4") || lower == "h4" {
        Some(4)
    } else {
        None
    }
}

/// Helper to parse toggle properties like `<w:b w:val="0"/>`.
fn is_truthy(e: &quick_xml::events::BytesStart) -> bool {
    for attr in e.attributes().flatten() {
        if local_name(&attr.key) == "val" {
            let val = String::from_utf8_lossy(&attr.value).to_lowercase();
            return !matches!(val.as_str(), "0" | "false" | "off");
        }
    }
    true // If <w:b/> exists but has no w:val, it implies true.
}

/// Flush accumulated list items into an `Element::List` and reset the buffer.
fn flush_list(
    elements: &mut Vec<Element>,
    items: &mut Vec<Vec<Run>>,
    ordered: bool,
    level: u8,
) {
    if !items.is_empty() {
        elements.push(Element::List {
            ordered,
            level,
            items: std::mem::take(items),
        });
    }
}

/// Strip the XML namespace prefix (e.g. `w:p` → `p`).
fn local_name(name: &quick_xml::name::QName) -> String {
    let bytes = name.local_name().into_inner();
    String::from_utf8_lossy(bytes).into_owned()
}

// ── Image Helpers ─────────────────────────────────────────────────────────────

#[derive(Default)]
struct ImageData {
    path: Option<String>,
    width_cm: Option<f64>,
    caption: Option<String>,
}

fn emu_to_cm(emu: u64) -> f64 {
    emu as f64 * 2.54 / 914_400.0
}

fn normalize_media_target(target: &str) -> String {
    // 1. Replace Windows backslashes
    let t = target.replace('\\', "/");
    // 2. Strip leading slash
    let t = t.trim_start_matches('/');
    // 3. Strip "word/" prefix if present
    let t = t.strip_prefix("word/").unwrap_or(t);
    // 4. Strip "../" prefix
    let t = t.trim_start_matches("../");
    // 5. Ensure "media/" prefix
    if t.starts_with("media/") {
        t.to_owned()
    } else {
        format!("media/{t}")
    }
}

fn read_relationships(archive: &mut ZipArchive<Cursor<&[u8]>>) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let xml = match archive.by_name("word/_rels/document.xml.rels") {
        Ok(mut e) => {
            let mut s = String::new();
            if e.read_to_string(&mut s).is_err() {
                return map;
            }
            s
        }
        Err(_) => return map,
    };

    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                if local_name(&e.name()) == "Relationship" {
                    let mut id = None;
                    let mut is_image = false;
                    let mut is_hyperlink = false;
                    let mut target = None;

                    for attr in e.attributes().flatten() {
                        let key = local_name(&attr.key);
                        let val = String::from_utf8_lossy(&attr.value);
                        match key.as_str() {
                            "Id" => id = Some(val.into_owned()),
                            "Type" => {
                                if val.ends_with("/image") {
                                    is_image = true;
                                } else if val.ends_with("/hyperlink") {
                                    is_hyperlink = true;
                                }
                            }
                            "Target" => target = Some(val.into_owned()),
                            _ => {}
                        }
                    }

                    if let (Some(id), Some(target)) = (id, target) {
                        if is_image {
                            map.insert(id, normalize_media_target(&target));
                        } else if is_hyperlink {
                            map.insert(id, target);
                        }
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_normalization() {
        assert_eq!(normalize_media_target("media/image1.png"),       "media/image1.png");
        assert_eq!(normalize_media_target(r"media\image1.png"),      "media/image1.png");
        assert_eq!(normalize_media_target("/word/media/image1.png"), "media/image1.png");
        assert_eq!(normalize_media_target("../media/image1.png"),    "media/image1.png");
        assert_eq!(normalize_media_target("image1.png"),             "media/image1.png");
    }

    #[test]
    fn test_emu_conversion() {
        assert!((emu_to_cm(914_400) - 2.54).abs() < 0.001);    // 1 inch
        assert!((emu_to_cm(3_240_000) - 9.00).abs() < 0.01);   // ~3.5in
        assert!((emu_to_cm(0) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_is_truthy_toggles() {
        use quick_xml::events::BytesStart;
        
        // No w:val implies true
        let e1 = BytesStart::new("b");
        assert!(is_truthy(&e1));

        // w:val="1" implies true
        let mut e2 = BytesStart::new("b");
        e2.push_attribute(("w:val", "1"));
        assert!(is_truthy(&e2));

        // w:val="0" implies false
        let mut e3 = BytesStart::new("b");
        e3.push_attribute(("w:val", "0"));
        assert!(!is_truthy(&e3));

        // w:val="false" implies false
        let mut e4 = BytesStart::new("b");
        e4.push_attribute(("w:val", "false"));
        assert!(!is_truthy(&e4));

        // w:val="True" implies true
        let mut e5 = BytesStart::new("b");
        e5.push_attribute(("w:val", "True"));
        assert!(is_truthy(&e5));
    }
}
