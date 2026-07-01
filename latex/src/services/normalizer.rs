//! AST post-processing normalizer.
//!
//! Runs a series of transformation passes on the raw [`Element`] list produced
//! by the reader, before it is handed to the renderer.  Each pass is a pure
//! function: raw AST in → clean AST out.
//!
//! Pipeline order:
//!   1. `deduplicate_page_breaks`   – collapse consecutive \clearpage
//!   2. `detect_toc_block`          – manual dot-leader TOC/LOF/LOT → \tableofcontents etc.
//!                                    (must run BEFORE heading promotion so that
//!                                     dot-leader lines are still plain paragraphs)
//!   3. `promote_bold_headings`     – heuristic bold-para → \section
//!                                    (suppressed before the first PageBreak so
//!                                     title-page content is never promoted)
//!   4. `coalesce_runs`             – merge adjacent runs with identical formatting
//!   5. `normalize_whitespace`      – collapse multiple consecutive spaces in run text
//!   6. `promote_inline_bullets`    – leading bullet char → \begin{itemize}
//!   7. `remove_empty_list_items`   – prune empty lines generated from bad word formats

use crate::models::{Element, Run};

// ── Public entry point ────────────────────────────────────────────────────────

/// Run all normalisation passes in the correct order.
pub fn normalize(elements: Vec<Element>) -> Vec<Element> {
    let elements = deduplicate_page_breaks(elements);
    let elements = detect_toc_block(elements);      // must precede heading promotion
    let elements = fold_captions(elements);         // fold caption paras into Image/Table
    let elements = promote_bold_headings(elements);
    let elements = coalesce_runs(elements);
    let elements = normalize_whitespace(elements);
    let elements = remove_stray_page_numbers(elements);
    let elements = promote_inline_bullets(elements);
    let elements = remove_empty_list_items(elements);
    elements
}

// ── Pass 1b: Fold caption paragraphs into adjacent Image / Table elements ────

/// Fold caption paragraphs into the Image / Table elements they describe.
///
/// **Figure captions** — a paragraph starting with "Figure N", "Fig. N", or
/// "Plate N" that appears *after* an Image (with up to 4 intervening blank
/// paragraphs) is folded into `Image::caption`.  The alt-text Word stores in
/// `docPr` (e.g. "Picture 849") is discarded in all cases because it is not a
/// human-readable caption.
///
/// **Table captions** — a paragraph starting with "Table N" or "Tab. N" that
/// appears *before* a Table (up to 3 elements back, skipping blank and
/// description paragraphs) is folded into `Table::caption`.
///
/// Both types of consumed elements are removed from the stream so they do not
/// appear as standalone paragraphs in the LaTeX output.
///
/// Must run before `promote_bold_headings` so that caption paragraphs are still
/// plain `Element::Paragraph` nodes.
pub fn fold_captions(elements: Vec<Element>) -> Vec<Element> {
    let n = elements.len();
    use std::collections::{HashMap, HashSet};

    // ── Pass 1: find caption associations and mark consumed indices ──────────
    let mut consumed: HashSet<usize> = HashSet::new();
    let mut table_cap: HashMap<usize, String> = HashMap::new(); // table_idx → caption
    let mut image_cap: HashMap<usize, String> = HashMap::new(); // image_idx → caption

    for i in 0..n {
        match &elements[i] {
            // ── Table: scan backwards for a caption paragraph ────────────────
            Element::Table { .. } => {
                if i == 0 { continue; }
                let mut j = i as isize - 1;
                let mut non_blank_skipped = 0usize;
                while j >= 0 {
                    let idx = j as usize;
                    if consumed.contains(&idx) { j -= 1; continue; }
                    match &elements[idx] {
                        Element::Paragraph { runs } => {
                            let text: String = runs.iter().map(|r| r.text.as_str()).collect();
                            let t = text.trim();
                            if t.is_empty() { j -= 1; continue; } // blank — skip
                            if looks_like_table_caption(t) {
                                table_cap.insert(i, extract_table_caption(t).to_owned());
                                consumed.insert(idx);
                                break;
                            }
                            // Non-caption paragraph (e.g. "The table below shows…")
                            non_blank_skipped += 1;
                            if non_blank_skipped >= 3 { break; } // don't look too far back
                            j -= 1;
                        }
                        // Section boundaries: stop scanning
                        Element::Heading { .. }
                        | Element::PageBreak
                        | Element::Table { .. }
                        | Element::Image { .. }
                        | Element::List { .. }
                        | Element::TocBlock
                        | Element::LofBlock
                        | Element::LotBlock => break,
                    }
                }
            }

            // ── Image: scan forward for a caption paragraph ──────────────────
            Element::Image { .. } => {
                let mut j = i + 1;
                let mut blanks_skipped = 0usize;
                while j < n {
                    match &elements[j] {
                        Element::Paragraph { runs } => {
                            let text: String = runs.iter().map(|r| r.text.as_str()).collect();
                            let t = text.trim();
                            if t.is_empty() {
                                blanks_skipped += 1;
                                if blanks_skipped > 4 { break; } // give up after 4 blanks
                                j += 1;
                                continue;
                            }
                            if looks_like_figure_caption(t) {
                                image_cap.insert(i, extract_figure_caption(t).to_owned());
                                // Consume all blank paragraphs between image and caption
                                for k in (i + 1)..=j {
                                    consumed.insert(k);
                                }
                                break;
                            }
                            break; // non-blank non-caption paragraph — stop
                        }
                        _ => break, // non-paragraph element — stop
                    }
                }
            }

            _ => {}
        }
    }

    // ── Pass 2: build output, applying collected captions ────────────────────
    let mut out: Vec<Element> = Vec::with_capacity(n);
    for i in 0..n {
        if consumed.contains(&i) {
            continue;
        }
        match &elements[i] {
            Element::Table { rows, .. } => {
                out.push(Element::Table {
                    rows: rows.clone(),
                    caption: table_cap.remove(&i),
                });
            }
            Element::Image { path, width_cm, .. } => {
                out.push(Element::Image {
                    path: path.clone(),
                    width_cm: *width_cm,
                    // Always use the explicit caption paragraph; discard alt-text
                    caption: image_cap.remove(&i),
                });
            }
            _ => out.push(elements[i].clone()),
        }
    }
    out
}

/// Return `true` when `text` looks like a table caption label.
///
/// Matches: "Table 3:", "TABLE 3.1:", "Tab. 3 —", etc.
/// Requires that the word "table"/"tab." be immediately followed by a digit.
fn looks_like_table_caption(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let rest = if lower.starts_with("table") {
        &lower["table".len()..]
    } else if lower.starts_with("tab.") {
        &lower["tab.".len()..]
    } else {
        return false;
    };
    rest.trim_start().starts_with(|c: char| c.is_ascii_digit())
}

/// Return `true` when `text` looks like a figure caption label.
///
/// Matches: "Figure 5:", "FIGURE 5:", "Fig. 2:", "Plate 1 —", etc.
fn looks_like_figure_caption(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let rest = if lower.starts_with("figure") {
        &lower["figure".len()..]
    } else if lower.starts_with("fig.") {
        &lower["fig.".len()..]
    } else if lower.starts_with("plate") {
        &lower["plate".len()..]
    } else {
        return false;
    };
    rest.trim_start().starts_with(|c: char| c.is_ascii_digit())
}

/// Strip the leading "Table N:" / "Table N." label from a table caption,
/// returning only the descriptive part.
///
/// Examples:
///   "TABLE 3: Development Tools"  → "Development Tools"
///   "Table 5.1. Results"          → "Results"
fn extract_table_caption(text: &str) -> &str {
    let lower = text.to_ascii_lowercase();
    let after_word = if lower.starts_with("table") {
        &text["table".len()..]
    } else if lower.starts_with("tab.") {
        &text["tab.".len()..]
    } else {
        return text;
    };
    // Trim leading whitespace first, then digits+dots (e.g. " 3.1"), then separator
    let after_ws  = after_word.trim_start();
    let after_num = after_ws.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.');
    let after_sep = after_num.trim_start_matches(|c: char| {
        matches!(c, ':' | '.' | '-' | '\u{2013}' | '\u{2014}' | ' ')
    });
    if after_sep.is_empty() { text } else { after_sep }
}

/// Strip the leading "Figure N:" / "Fig. N:" label from a figure caption.
///
/// Examples:
///   "Figure 5: Postman Login"   → "Postman Login"
///   "Figure 5.2: Failed Login"  → "Failed Login"
///   "Fig. 2. Architecture"      → "Architecture"
fn extract_figure_caption(text: &str) -> &str {
    let lower = text.to_ascii_lowercase();
    let after_word = if lower.starts_with("figure") {
        &text["figure".len()..]
    } else if lower.starts_with("fig.") {
        &text["fig.".len()..]
    } else if lower.starts_with("plate") {
        &text["plate".len()..]
    } else {
        return text;
    };
    // Trim whitespace, then digits+dots+dashes (e.g. " 5.2"), then separator
    let after_ws  = after_word.trim_start();
    let after_num = after_ws.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == '-');
    let after_sep = after_num.trim_start_matches(|c: char| {
        matches!(c, ':' | '.' | '-' | '\u{2013}' | '\u{2014}' | ' ')
    });
    if after_sep.is_empty() { text } else { after_sep }
}

// ── Pass 5: Normalize whitespace ─────────────────────────────────────────────

/// Collapse runs of 2+ consecutive spaces/tabs to a single space in every
/// non-monospace run.  Word documents often carry alignment spaces (manual
/// centering, tab stops) as raw space characters; these produce ugly gaps in
/// the LaTeX output.
pub fn normalize_whitespace(elements: Vec<Element>) -> Vec<Element> {
    fn fix_runs(runs: Vec<Run>) -> Vec<Run> {
        runs.into_iter().map(|mut r| {
            if !r.mono {
                r.text = collapse_spaces(&r.text);
            }
            r
        }).collect()
    }

    elements.into_iter().map(|el| match el {
        Element::Paragraph { runs } => Element::Paragraph { runs: fix_runs(runs) },
        Element::Heading { level, runs } => Element::Heading { level, runs: fix_runs(runs) },
        Element::List { ordered, items } => Element::List {
            ordered,
            items: items.into_iter().map(|(lvl, runs)| (lvl, fix_runs(runs))).collect(),
        },
        other => other,
    }).collect()
}

/// Replace any run of 2+ consecutive ASCII whitespace characters with a single space.
fn collapse_spaces(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for ch in s.chars() {
        if ch == ' ' || ch == '\t' {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out
}

// ── Pass 6b: Remove stray page-number paragraphs ─────────────────────────────

/// Drop paragraphs that contain only 1–3 ASCII digits — these are almost always
/// page-number field codes that leaked out of Word's footer or cross-reference
/// stream (e.g. a lone "6" sitting between two section headings).
/// Legitimate prose paragraphs never consist solely of a short number.
pub fn remove_stray_page_numbers(elements: Vec<Element>) -> Vec<Element> {
    elements
        .into_iter()
        .filter(|el| {
            if let Element::Paragraph { runs } = el {
                let text: String = runs.iter().map(|r| r.text.as_str()).collect();
                let t = text.trim();
                // 1–3 chars, all ASCII digits, non-empty → stray page number
                if !t.is_empty() && t.len() <= 3 && t.chars().all(|c| c.is_ascii_digit()) {
                    return false;
                }
            }
            true
        })
        .collect()
}

// ── Pass 7: Remove empty list items ──────────────────────────────────────────

/// Prune any list items that contain only whitespace/empty runs.
pub fn remove_empty_list_items(elements: Vec<Element>) -> Vec<Element> {
    elements.into_iter().map(|mut el| {
        if let Element::List { ref mut items, .. } = el {
            items.retain(|(_, runs)| !runs.iter().all(|r| r.text.trim().is_empty()));
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

// ── Pass 3: Promote bold paragraphs to headings ───────────────────────────────

/// Promote paragraphs that look like headings to `Element::Heading`.
///
/// A paragraph qualifies if ALL of the following are true:
///   - Every run is bold
///   - Total text is ≤ 120 characters
///   - Text does NOT end with '.' (not a trailing sentence)
///   - Text has between 1 and 12 words
///   - Text matches a heading pattern (chapter/section numbering or ALL-CAPS)
///
/// Additionally, heading promotion is suppressed entirely until after the first
/// `Element::PageBreak` in the document.  This prevents title-page content
/// (university name, student name, registration number, etc.) from being
/// incorrectly promoted to `\chapter` or `\section`.
pub fn promote_bold_headings(elements: Vec<Element>) -> Vec<Element> {
    let mut seen_page_break = false;
    elements
        .into_iter()
        .map(|el| {
            // Track the first page break — promotion is only allowed after it.
            if matches!(el, Element::PageBreak) {
                seen_page_break = true;
                return el;
            }

            // Suppress promotion on the title page (before the first \clearpage).
            if !seen_page_break {
                return el;
            }

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

    // ── Pattern matching ────────────────────────────────────────────────────────

    // Guard: "CHAPTER 1", "Chapter 1", "CHAPTER 1: Title", "Chapter 2 - Title".
    // This MUST be checked before the ALL-CAPS heuristic below, otherwise a
    // heading like "CHAPTER 1: INTRODUCTION" (100% uppercase) would be demoted
    // to \section instead of staying as \chapter.
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

    // ALL-CAPS heuristic (≥ 75% uppercase alphabetic chars).
    // Returns level 2 (\section) — unnumbered ALL-CAPS headings like OBJECTIVES,
    // LIMITATIONS, RESULTS that appear inside chapter bodies should be sections,
    // not chapters.  Front-matter ALL-CAPS headings (ABSTRACT, DECLARATION, etc.)
    // are handled separately in the renderer via a keyword list + \chapter*{}.
    let alpha_chars: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
    if !alpha_chars.is_empty() {
        let upper_ratio =
            alpha_chars.iter().filter(|c| c.is_uppercase()).count() as f64
                / alpha_chars.len() as f64;
        if upper_ratio >= 0.75 {
            return Some(2);
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

// ── Pass 2: Detect manual TOC blocks ─────────────────────────────────────────

/// Replace clusters of manual dot-leader TOC paragraphs with `Element::TocBlock`.
///
/// A paragraph is a TOC entry if its plain text (after stripping dots and
/// `\ldots` sequences) ends with a bare page number (1–3 digits).
///
/// Two cases are handled:
///   1. **TOC header + entries**: A "TABLE OF CONTENTS" heading followed by dot-leader
///      lines is replaced with a single `TocBlock`.
///   2. **Stray entries (anywhere)**: Dot-leader lines that appear anywhere in the
///      document without a TOC header (e.g. a mid-document mini-TOC repeated in body
///      chapters) are simply dropped — they have no meaning in LaTeX output.
pub fn detect_toc_block(elements: Vec<Element>) -> Vec<Element> {
    let mut out: Vec<Element> = Vec::with_capacity(elements.len());
    let mut i = 0;
    let n = elements.len();

    while i < n {
        if is_toc_header(&elements[i]) {
            out.push(Element::TocBlock);

            // Scan forward from the TOC header, dropping all TOC material.
            //
            // For each element encountered:
            //   ─ Blank paragraph, dot-leader TOC entry, or page break: skip
            //   ─ Heading-like paragraph (bold, chapter/ALL-CAPS pattern):
            //       look ahead (skipping blanks + page breaks) to see what follows:
            //       • followed by real content → this IS the first body heading; stop
            //       • followed by more headings/TOC → still inside TOC; skip it
            //   ─ Real content paragraph: body starts here; stop
            let mut j = i + 1;

            while j < n {
                let el = &elements[j];

                // Always skip these inside the TOC region
                if is_blank_paragraph(el) || is_toc_entry(el) || matches!(el, Element::PageBreak) {
                    j += 1;
                    continue;
                }

                // A LOF or LOT header ends the TOC scan — they are separate sections
                // that must be handled by their own branches below.
                if is_lof_header(el) || is_lot_header(el) {
                    break;
                }

                // Is this a heading-like or known TOC chapter-title paragraph?
                let heading_like = if let Element::Paragraph { runs } = el {
                    is_toc_chapter_title(runs) || detect_heading_level(runs).is_some()
                } else {
                    false
                };

                if heading_like {
                    // Look ahead past blanks and page breaks
                    let next_idx = next_real_element(j + 1, &elements);

                    let followed_by_content = next_idx
                        .map(|k| is_content_paragraph(&elements[k]))
                        .unwrap_or(false);

                    if followed_by_content {
                        // This heading is the real first body heading — stop here
                        break;
                    } else {
                        // Still inside TOC material — skip this heading
                        j += 1;
                        continue;
                    }
                }

                // Non-heading, non-blank, non-TOC-entry — body starts here
                break;
            }

            i = j;

        } else if is_lof_header(&elements[i]) {
            // Replace manual LOF header + dot-leader entries with LofBlock.
            // The dot-leader entries have wrong page numbers in LaTeX anyway.
            out.push(Element::LofBlock);
            let mut j = i + 1;
            while j < n && (is_toc_entry(&elements[j]) || is_blank_paragraph(&elements[j]) || matches!(elements[j], Element::PageBreak)) {
                j += 1;
            }
            i = j;

        } else if is_lot_header(&elements[i]) {
            // Replace manual LOT header + dot-leader entries with LotBlock.
            out.push(Element::LotBlock);
            let mut j = i + 1;
            while j < n && (is_toc_entry(&elements[j]) || is_blank_paragraph(&elements[j]) || matches!(elements[j], Element::PageBreak)) {
                j += 1;
            }
            i = j;

        } else {
            out.push(elements[i].clone());
            i += 1;
        }
    }
    out
}

/// Skip over blank paragraphs and page breaks; return the index of the
/// next element that is neither, or `None` if the slice is exhausted.
fn next_real_element(start: usize, elements: &[Element]) -> Option<usize> {
    let mut k = start;
    while k < elements.len() {
        let el = &elements[k];
        if is_blank_paragraph(el) || matches!(el, Element::PageBreak) {
            k += 1;
        } else {
            return Some(k);
        }
    }
    None
}

/// Returns true when the runs spell out a bare "Chapter N" or "Appendix X"
/// title line — the kind of bold but undotted TOC chapter-title entry
/// that Word generates for top-level headings in the TOC.
///
/// These are NOT dot-leader entries, so `is_toc_entry()` misses them;
/// this helper provides the extra safety layer recommended in the review.
fn is_toc_chapter_title(runs: &[Run]) -> bool {
    let text: String = runs.iter().map(|r| r.text.as_str()).collect();
    let lower = text.trim().to_lowercase();
    lower.starts_with("chapter ") || lower.starts_with("appendix ")
}

/// A paragraph is considered "real content" if it's not blank, not a TOC entry, 
/// and wouldn't be promoted to a heading.
fn is_content_paragraph(el: &Element) -> bool {
    if let Element::Paragraph { runs } = el {
        if is_blank_paragraph(el) {
            return false;
        }
        if is_toc_entry(el) {
            return false;
        }
        if detect_heading_level(runs).is_some() {
            return false;
        }
        true
    } else {
        false
    }
}

fn is_blank_paragraph(el: &Element) -> bool {
    if let Element::Paragraph { runs } = el {
        runs.iter().all(|r| r.text.trim().is_empty())
    } else {
        false
    }
}


/// Return `true` if the element looks like a manual TOC dot-leader entry.
///
/// Handles both `Element::Paragraph` and `Element::Heading` — the latter can
/// appear if a previous pass already promoted a dot-leader line to a heading
/// (belt-and-suspenders, now that detect_toc_block runs before promotion).
fn is_toc_entry(el: &Element) -> bool {
    let text = match el {
        Element::Paragraph { runs } | Element::Heading { runs, .. } => {
            runs.iter().map(|r| r.text.as_str()).collect::<String>()
        }
        _ => return false,
    };
    let text = text.trim();

    // Must contain a cluster of dots (4+ consecutive is the minimum threshold)
    let has_dots = text.contains("....") || text.contains('…');
    if !has_dots {
        return false;
    }

    // If the dot sequence is very long (≥ 10 consecutive dots) the line is
    // overwhelmingly a dot-leader regardless of whether the page number was
    // preserved.  Word sometimes puts the page number in a separate run that
    // gets trimmed away, leaving trailing punctuation instead of a digit.
    let long_dot_run = text.contains("..........")
        || text.contains("…………")
        || text.matches("\\ldots").count() >= 5;
    if long_dot_run {
        return true;
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
    let upper = element_text_upper(el);
    upper.contains("TABLE OF CONTENTS") || upper == "CONTENTS"
}

/// Return `true` if the element looks like a "LIST OF FIGURES" header.
fn is_lof_header(el: &Element) -> bool {
    let upper = element_text_upper(el);
    upper.contains("LIST OF FIGURES") || upper.contains("LIST OF FIGURE")
}

/// Return `true` if the element looks like a "LIST OF TABLES" header.
fn is_lot_header(el: &Element) -> bool {
    let upper = element_text_upper(el);
    upper.contains("LIST OF TABLES") || upper.contains("LIST OF TABLE")
}

fn element_text_upper(el: &Element) -> String {
    match el {
        Element::Paragraph { runs } | Element::Heading { runs, .. } => {
            runs.iter().map(|r| r.text.as_str()).collect::<String>().trim().to_uppercase()
        }
        _ => String::new(),
    }
}

// ── Pass 4: Coalesce adjacent runs with identical formatting ─────────────────

/// Merge consecutive runs within each `Paragraph` or `Heading` that share
/// identical formatting properties into a single run.  This eliminates the
/// character-by-character `\textbf{K}\textbf{Y}\textbf{AM}…` fragmentation
/// that Word sometimes produces, resulting in cleaner `.tex` source.
pub fn coalesce_runs(elements: Vec<Element>) -> Vec<Element> {
    elements
        .into_iter()
        .map(|el| match el {
            Element::Paragraph { runs } => Element::Paragraph {
                runs: merge_runs(runs),
            },
            Element::Heading { level, runs } => Element::Heading {
                level,
                runs: merge_runs(runs),
            },
            Element::List { ordered, items } => Element::List {
                ordered,
                items: items.into_iter().map(|(lvl, runs)| (lvl, merge_runs(runs))).collect(),
            },
            other => other,
        })
        .collect()
}

/// Merge consecutive runs that share all formatting properties.
fn merge_runs(runs: Vec<Run>) -> Vec<Run> {
    let mut out: Vec<Run> = Vec::with_capacity(runs.len());
    for run in runs {
        if let Some(last) = out.last_mut() {
            if last.bold == run.bold
                && last.italic == run.italic
                && last.underline == run.underline
                && last.mono == run.mono
                && last.superscript == run.superscript
                && last.subscript == run.subscript
                && last.hyperlink == run.hyperlink
            {
                last.text.push_str(&run.text);
                continue;
            }
        }
        out.push(run);
    }
    out
}

// ── Pass 5: Promote inline bullet characters to list items ────────────────────

const BULLET_CHARS: &[char] = &['•', '▪', '▸', '►', '‣'];

/// Group consecutive paragraphs that start with a bullet character into
/// `Element::List { ordered: false }` blocks.
pub fn promote_inline_bullets(elements: Vec<Element>) -> Vec<Element> {
    let mut out: Vec<Element> = Vec::with_capacity(elements.len());
    let mut pending_bullets: Vec<(u8, Vec<Run>)> = Vec::new();

    for el in elements {
        if let Element::Paragraph { ref runs } = el {
            if let Some(item_runs) = strip_bullet_prefix(runs) {
                pending_bullets.push((0, item_runs));
                continue;
            }
        }

        // Flush any pending bullets before this non-bullet element
        if !pending_bullets.is_empty() {
            out.push(Element::List {
                ordered: false,
                items: std::mem::take(&mut pending_bullets),
            });
        }
        out.push(el);
    }

    // Flush any trailing bullets
    if !pending_bullets.is_empty() {
        out.push(Element::List {
            ordered: false,
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
        let input = vec![Element::PageBreak, para(vec![bold_run("CHAPTER 1: INTRODUCTION")])];
        let out = promote_bold_headings(input);
        assert!(matches!(out[1], Element::Heading { level: 1, .. }));
    }

    #[test]
    fn test_subsection_numbered() {
        let input = vec![Element::PageBreak, para(vec![bold_run("1.2 Background of the study")])];
        let out = promote_bold_headings(input);
        assert!(matches!(out[1], Element::Heading { level: 2, .. }));
    }

    #[test]
    fn test_subsubsection_numbered() {
        let input = vec![Element::PageBreak, para(vec![bold_run("1.2.3 Specific sub-point")])];
        let out = promote_bold_headings(input);
        assert!(matches!(out[1], Element::Heading { level: 3, .. }));
    }

    #[test]
    fn test_allcaps_promoted() {
        // ALL-CAPS bold headings after a page break → \section (level 2)
        let input = vec![Element::PageBreak, para(vec![bold_run("ABSTRACT")])];
        let out = promote_bold_headings(input);
        assert!(matches!(out[1], Element::Heading { level: 2, .. }));
    }

    #[test]
    fn test_long_sentence_not_promoted() {
        let text = "This is a very long bold sentence that should not become a heading because it ends with a period.";
        let input = vec![Element::PageBreak, para(vec![bold_run(text)])];
        let out = promote_bold_headings(input);
        assert!(matches!(out[1], Element::Paragraph { .. }));
    }

    #[test]
    fn test_non_bold_not_promoted() {
        let input = vec![Element::PageBreak, para(vec![plain_run("Chapter 1")])];
        let out = promote_bold_headings(input);
        assert!(matches!(out[1], Element::Paragraph { .. }));
    }

    #[test]
    fn test_bold_stripped_after_promotion() {
        let input = vec![Element::PageBreak, para(vec![bold_run("CHAPTER 2")])];
        let out = promote_bold_headings(input);
        if let Element::Heading { runs, .. } = &out[1] {
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
    fn test_stray_entries_kept_without_header() {
        let toc_entry = |text: &str| para(vec![plain_run(text)]);
        let input = vec![
            toc_entry("Intro....... 1"),
            toc_entry("Background.. 5"),
            toc_entry("Results..... 10"),
        ];
        // No TOC header → dot-leader lines are kept as-is (don't silently drop content)
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
            let (_, runs) = &items[0];
            let text: String = runs.iter().map(|r| r.text.as_str()).collect();
            assert_eq!(text, "Hello world");
        }
    }
}
