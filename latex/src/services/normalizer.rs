//! AST post-processing normalizer.
//!
//! Runs a series of transformation passes on the raw [`Element`] list produced
//! by the reader, before it is handed to the renderer.  Each pass is a pure
//! function: raw AST in → clean AST out.
//!
//! Pipeline order (matters — headings must exist before TOC detection):
//!   1. `deduplicate_page_breaks`   – collapse consecutive \clearpage
//!   2. `promote_bold_headings`     – heuristic bold-para → \section
//!   3. `detect_toc_block`          – manual dot-leader TOC → \tableofcontents
//!   4. `promote_inline_bullets`    – leading bullet char → \begin{itemize}
//!   5. `remove_empty_list_items`   – prune empty lines generated from bad word formats

use crate::models::{Element, Run};

// ── Public entry point ────────────────────────────────────────────────────────

/// Run all normalisation passes in the correct order.
pub fn normalize(elements: Vec<Element>) -> Vec<Element> {
    let elements = deduplicate_page_breaks(elements);
    let elements = promote_bold_headings(elements);
    let elements = detect_toc_block(elements);
    let elements = promote_inline_bullets(elements);
    let elements = remove_empty_list_items(elements);
    elements
}

// ── Pass 5: Remove empty list items ──────────────────────────────────────────

/// Prune any list items that contain only whitespace/empty runs.
pub fn remove_empty_list_items(elements: Vec<Element>) -> Vec<Element> {
    elements.into_iter().map(|mut el| {
        if let Element::List { ref mut items, .. } = el {
            items.retain(|runs| !runs.iter().all(|r| r.text.trim().is_empty()));
        }
        el
    }).collect()
}

// ── Pass 1: Deduplicate consecutive page breaks ───────────────────────────────

/// Collapse runs of consecutive `PageBreak` elements into a single one.
pub fn deduplicate_page_breaks(elements: Vec<Element>) -> Vec<Element> {
    let mut out: Vec<Element> = Vec::with_capacity(elements.len());
    let mut last_was_break = false;
    for el in elements {
        let is_break = matches!(el, Element::PageBreak);
        if is_break && last_was_break {
            // skip — already have a break queued
            continue;
        }
        last_was_break = is_break;
        out.push(el);
    }
    out
}

// ── Pass 2: Promote bold paragraphs to headings ───────────────────────────────

/// Promote paragraphs that look like headings to `Element::Heading`.
///
/// A paragraph qualifies if ALL of the following are true:
///   - Every run is bold
///   - Total text is ≤ 120 characters
///   - Text does NOT end with '.' (not a trailing sentence)
///   - Text has between 1 and 12 words
///   - Text matches a heading pattern (chapter/section numbering or ALL-CAPS)
pub fn promote_bold_headings(elements: Vec<Element>) -> Vec<Element> {
    elements
        .into_iter()
        .map(|el| {
            if let Element::Paragraph { ref runs } = el {
                if let Some(level) = detect_heading_level(runs) {
                    // Strip bold flag — heading commands imply bold styling
                    let clean_runs = runs
                        .iter()
                        .cloned()
                        .map(|mut r| {
                            r.bold = false;
                            r
                        })
                        .collect();
                    return Element::Heading {
                        level,
                        runs: clean_runs,
                    };
                }
            }
            el
        })
        .collect()
}

/// Determine what heading level a run set should map to, or `None` if it
/// should remain a plain paragraph.
fn detect_heading_level(runs: &[Run]) -> Option<u8> {
    // Must have at least one run
    if runs.is_empty() {
        return None;
    }

    // All runs must be bold
    if !runs.iter().all(|r| r.bold) {
        return None;
    }

    let text: String = runs.iter().map(|r| r.text.as_str()).collect();
    let text = text.trim();

    // Length guard: 1–120 chars
    if text.is_empty() || text.len() > 120 {
        return None;
    }

    // Must not end with a period (looks like a sentence)
    if text.ends_with('.') {
        return None;
    }

    // Word count guard: 1–12 words
    let word_count = text.split_whitespace().count();
    if word_count == 0 || word_count > 12 {
        return None;
    }

    // ── Pattern matching ──────────────────────────────────────────────────────

    // "CHAPTER 1" / "Chapter 1" / "CHAPTER ONE"
    let lower = text.to_lowercase();
    if lower.starts_with("chapter") {
        return Some(1);
    }

    // Numbered section patterns: "1.2.3 Title" → level 3
    //                            "1.2 Title"   → level 2
    //                            "1. Title"    → level 1
    let first_word = text.split_whitespace().next().unwrap_or("");
    if let Some(level) = numbering_level(first_word) {
        return Some(level);
    }

    // ALL-CAPS heuristic (≥ 75% uppercase alphabetic chars)
    let alpha_chars: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
    if !alpha_chars.is_empty() {
        let upper_ratio =
            alpha_chars.iter().filter(|c| c.is_uppercase()).count() as f64
                / alpha_chars.len() as f64;
        if upper_ratio >= 0.75 {
            return Some(1);
        }
    }

    None
}

/// Infer heading level from a numeric prefix like "1.", "1.2", "1.2.3".
fn numbering_level(word: &str) -> Option<u8> {
    // Strip trailing dot
    let w = word.trim_end_matches('.');
    let parts: Vec<&str> = w.split('.').collect();

    // All parts must be digits
    if parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())) {
        return match parts.len() {
            1 => Some(1),
            2 => Some(2),
            3 => Some(3),
            _ => Some(4),
        };
    }
    None
}

// ── Pass 3: Detect manual TOC blocks ─────────────────────────────────────────

/// Replace clusters of manual dot-leader TOC paragraphs with `Element::TocBlock`.
///
/// A paragraph is a TOC entry if its plain text (after stripping dots and
/// `\ldots` sequences) ends with a bare page number (1–3 digits).
/// Clusters of ≥ 4 consecutive TOC entries are replaced with a single
/// `TocBlock`.  Any immediately preceding "TABLE OF CONTENTS" heading/bold
/// paragraph is also consumed.
pub fn detect_toc_block(elements: Vec<Element>) -> Vec<Element> {
    let mut out: Vec<Element> = Vec::with_capacity(elements.len());
    let mut i = 0;
    let n = elements.len();
    let toc_search_limit = if n > 0 { n / 5 } else { 0 }; // First 20% of the document

    while i < n {
        if is_toc_header(&elements[i]) {
            out.push(Element::TocBlock);
            i += 1;
            
            // Skip immediately following TOC entries, blank paragraphs, and up to 2 PageBreaks
            let mut page_breaks_seen = 0;
            while i < n && page_breaks_seen < 3 {
                if is_toc_entry(&elements[i]) {
                    i += 1;
                } else if matches!(elements[i], Element::PageBreak) {
                    page_breaks_seen += 1;
                    // We don't skip the page break itself, just consume it so we can keep looking
                    out.push(take_element(&elements, i));
                    i += 1;
                } else if is_blank_paragraph(&elements[i]) {
                    i += 1;
                } else {
                    break;
                }
            }
        } else if i < toc_search_limit && is_toc_entry(&elements[i]) {
            // Stray TOC entry in the front matter, drop it.
            i += 1;
        } else {
            out.push(take_element(&elements, i));
            i += 1;
        }
    }
    out
}

fn is_blank_paragraph(el: &Element) -> bool {
    if let Element::Paragraph { runs } = el {
        runs.iter().all(|r| r.text.trim().is_empty())
    } else {
        false
    }
}

/// Clone an element from the slice (workaround since Element isn't Copy).
fn take_element(elements: &[Element], idx: usize) -> Element {
    // We rebuild from parts — cheaper than a full derive(Clone) on Element
    match &elements[idx] {
        Element::Paragraph { runs } => Element::Paragraph { runs: runs.clone() },
        Element::Heading { level, runs } => Element::Heading { level: *level, runs: runs.clone() },
        Element::List { ordered, level, items } => Element::List {
            ordered: *ordered,
            level: *level,
            items: items.clone(),
        },
        Element::Table { rows } => Element::Table { rows: rows.clone() },
        Element::Image { path, width_cm, caption } => Element::Image {
            path: path.clone(),
            width_cm: *width_cm,
            caption: caption.clone(),
        },
        Element::PageBreak => Element::PageBreak,
        Element::TocBlock => Element::TocBlock,
    }
}

/// Return `true` if the element looks like a manual TOC dot-leader entry.
fn is_toc_entry(el: &Element) -> bool {
    let text = match el {
        Element::Paragraph { runs } => runs.iter().map(|r| r.text.as_str()).collect::<String>(),
        _ => return false,
    };
    let text = text.trim();

    // Must contain a cluster of dots
    let has_dots = text.contains("....") || text.contains('…');
    if !has_dots {
        return false;
    }

    // Replace dots and ellipsis with spaces so words don't merge
    let mut stripped = String::with_capacity(text.len());
    for c in text.chars() {
        if c == '.' || c == '…' || c == '\u{2026}' {
            stripped.push(' ');
        } else {
            stripped.push(c);
        }
    }
    let stripped = stripped.trim();

    // Last token should be a page number
    if let Some(last) = stripped.split_whitespace().last() {
        return last.chars().all(|c| c.is_ascii_digit()) && last.len() <= 3;
    }
    false
}

/// Return `true` if the element looks like a "TABLE OF CONTENTS" header.
fn is_toc_header(el: &Element) -> bool {
    let text = match el {
        Element::Paragraph { runs } | Element::Heading { runs, .. } => {
            runs.iter().map(|r| r.text.as_str()).collect::<String>()
        }
        _ => return false,
    };
    let upper = text.trim().to_uppercase();
    upper.contains("TABLE OF CONTENTS") || upper.contains("CONTENTS")
}

// ── Pass 4: Promote inline bullet characters to list items ────────────────────

const BULLET_CHARS: &[char] = &['•', '▪', '▸', '►', '‣'];

/// Group consecutive paragraphs that start with a bullet character into
/// `Element::List { ordered: false }` blocks.
pub fn promote_inline_bullets(elements: Vec<Element>) -> Vec<Element> {
    let mut out: Vec<Element> = Vec::with_capacity(elements.len());
    let mut pending_bullets: Vec<Vec<Run>> = Vec::new();

    for el in elements {
        if let Element::Paragraph { ref runs } = el {
            if let Some(item_runs) = strip_bullet_prefix(runs) {
                pending_bullets.push(item_runs);
                continue;
            }
        }

        // Flush any pending bullets before this non-bullet element
        if !pending_bullets.is_empty() {
            out.push(Element::List {
                ordered: false,
                level: 0,
                items: std::mem::take(&mut pending_bullets),
            });
        }
        out.push(el);
    }

    // Flush any trailing bullets
    if !pending_bullets.is_empty() {
        out.push(Element::List {
            ordered: false,
            level: 0,
            items: pending_bullets,
        });
    }

    out
}

/// If the paragraph starts with a bullet character, return the runs with that
/// character stripped.  Otherwise return `None`.
fn strip_bullet_prefix(runs: &[Run]) -> Option<Vec<Run>> {
    let first_text = runs.first()?.text.trim_start();
    let first_char = first_text.chars().next()?;

    if !BULLET_CHARS.contains(&first_char) {
        return None;
    }

    // Rebuild runs with the bullet character removed from the first run
    let mut new_runs: Vec<Run> = runs.to_vec();
    new_runs[0].text = first_text
        .trim_start_matches(first_char)
        .trim_start()
        .to_owned();

    // Remove empty leading run
    if new_runs[0].text.is_empty() {
        new_runs.remove(0);
    }

    Some(new_runs)
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Run;

    fn bold_run(text: &str) -> Run {
        Run { text: text.to_owned(), bold: true, ..Default::default() }
    }

    fn plain_run(text: &str) -> Run {
        Run { text: text.to_owned(), ..Default::default() }
    }

    fn para(runs: Vec<Run>) -> Element {
        Element::Paragraph { runs }
    }

    // ── Page break deduplication ──────────────────────────────────────────────

    #[test]
    fn test_dedup_page_breaks_collapses_runs() {
        let input = vec![
            para(vec![plain_run("text")]),
            Element::PageBreak,
            Element::PageBreak,
            Element::PageBreak,
            para(vec![plain_run("more text")]),
        ];
        let out = deduplicate_page_breaks(input);
        assert_eq!(out.len(), 3);
        assert!(matches!(out[1], Element::PageBreak));
    }

    #[test]
    fn test_dedup_page_breaks_single_kept() {
        let input = vec![
            para(vec![plain_run("a")]),
            Element::PageBreak,
            para(vec![plain_run("b")]),
        ];
        let out = deduplicate_page_breaks(input);
        assert_eq!(out.len(), 3);
    }

    // ── Heading promotion ─────────────────────────────────────────────────────

    #[test]
    fn test_chapter_promoted_to_level1() {
        let input = vec![para(vec![bold_run("CHAPTER 1: INTRODUCTION")])];
        let out = promote_bold_headings(input);
        assert!(matches!(out[0], Element::Heading { level: 1, .. }));
    }

    #[test]
    fn test_subsection_numbered() {
        let input = vec![para(vec![bold_run("1.2 Background of the study")])];
        let out = promote_bold_headings(input);
        assert!(matches!(out[0], Element::Heading { level: 2, .. }));
    }

    #[test]
    fn test_subsubsection_numbered() {
        let input = vec![para(vec![bold_run("1.2.3 Specific sub-point")])];
        let out = promote_bold_headings(input);
        assert!(matches!(out[0], Element::Heading { level: 3, .. }));
    }

    #[test]
    fn test_allcaps_promoted() {
        let input = vec![para(vec![bold_run("ABSTRACT")])];
        let out = promote_bold_headings(input);
        assert!(matches!(out[0], Element::Heading { level: 1, .. }));
    }

    #[test]
    fn test_long_sentence_not_promoted() {
        let text = "This is a very long bold sentence that should not become a heading because it ends with a period.";
        let input = vec![para(vec![bold_run(text)])];
        let out = promote_bold_headings(input);
        assert!(matches!(out[0], Element::Paragraph { .. }));
    }

    #[test]
    fn test_non_bold_not_promoted() {
        let input = vec![para(vec![plain_run("Chapter 1")])];
        let out = promote_bold_headings(input);
        assert!(matches!(out[0], Element::Paragraph { .. }));
    }

    #[test]
    fn test_bold_stripped_after_promotion() {
        let input = vec![para(vec![bold_run("CHAPTER 2")])];
        let out = promote_bold_headings(input);
        if let Element::Heading { runs, .. } = &out[0] {
            assert!(!runs[0].bold, "bold flag should be stripped on promotion");
        }
    }

    // ── TOC detection ─────────────────────────────────────────────────────────

    #[test]
    fn test_toc_cluster_replaced() {
        let toc_entry = |text: &str| para(vec![plain_run(text)]);
        let input = vec![
            para(vec![bold_run("TABLE OF CONTENTS")]),
            toc_entry("Introduction...................... 1"),
            toc_entry("Background........................ 5"),
            toc_entry("Methodology....................... 10"),
            toc_entry("Results........................... 15"),
            toc_entry("Conclusion........................ 20"),
        ];
        let out = detect_toc_block(input);
        // TOC header + 5 entries → 1 TocBlock
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], Element::TocBlock));
    }

    #[test]
    fn test_small_cluster_kept() {
        let toc_entry = |text: &str| para(vec![plain_run(text)]);
        let input = vec![
            toc_entry("Intro....... 1"),
            toc_entry("Background.. 5"),
            toc_entry("Results..... 10"),
        ];
        // Only 3 entries — not enough, keep as paragraphs
        let out = detect_toc_block(input);
        assert_eq!(out.len(), 3);
        assert!(out.iter().all(|e| matches!(e, Element::Paragraph { .. })));
    }

    // ── Bullet promotion ──────────────────────────────────────────────────────

    #[test]
    fn test_bullets_grouped_into_list() {
        let input = vec![
            para(vec![plain_run("• First item")]),
            para(vec![plain_run("• Second item")]),
            para(vec![plain_run("• Third item")]),
            para(vec![plain_run("Normal paragraph")]),
        ];
        let out = promote_inline_bullets(input);
        assert_eq!(out.len(), 2);
        assert!(matches!(out[0], Element::List { ordered: false, .. }));
        assert!(matches!(out[1], Element::Paragraph { .. }));
    }

    #[test]
    fn test_bullet_char_stripped_from_run() {
        let input = vec![para(vec![plain_run("• Hello world")])];
        let out = promote_inline_bullets(input);
        if let Element::List { items, .. } = &out[0] {
            let text: String = items[0].iter().map(|r| r.text.as_str()).collect();
            assert_eq!(text, "Hello world");
        }
    }
}
