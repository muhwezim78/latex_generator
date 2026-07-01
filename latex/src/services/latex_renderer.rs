//! Convert a [`Document`] AST into a complete, compilable LaTeX string.
//!
//! Three template presets are supported:
//!   - `Default`  → `article` class, sensible defaults for general use
//!   - `Ieee`     → `IEEEtran` class, two-column IEEE conference format
//!   - `Thesis`   → `report` class, single-sided thesis format with chapter support

use crate::{
    cli_structure::Template,
    models::{Document, Element, Run},
    services::escaper,
};

// ── Public entry point ────────────────────────────────────────────────────────

/// Render `doc` into a LaTeX source string using the given `template`.
pub fn render(doc: &Document, template: &Template) -> String {
    let mut out = String::with_capacity(8192);

    // 1. Preamble
    emit_preamble(&mut out, doc, template);

    // 2. Begin document
    out.push_str("\\begin{document}\n\n");

    // Maketitle — suppressed for thesis because the Word document provides its
    // own formatted title page (university logo, department, student name, etc.).
    // Emitting \maketitle would just prepend a blank "Untitled" page.
    match template {
        Template::Thesis => {} // title page comes from the Word front matter
        _ => out.push_str("\\maketitle\n\n"),
    }

    // 3. Body
    let mut seen_numbered_chapter = false;

    if template == &Template::Thesis {
        // Collect all title-page elements (everything before the first PageBreak,
        // Heading, or paragraph whose text matches a known front-matter keyword).
        let title_end = doc.elements.iter().position(|el| match el {
            Element::PageBreak => true,
            Element::Heading { .. } => true,
            Element::TocBlock => true,
            Element::Paragraph { runs } => {
                let text: String = runs.iter().map(|r| r.text.as_str()).collect();
                is_front_matter_heading(text.trim())
            }
            _ => false,
        });

        let title_end = title_end.unwrap_or(doc.elements.len());
        let (title_elems, body_elems) = doc.elements.split_at(title_end);

        // ── Render title page ────────────────────────────────────────────────
        out.push_str("\\begin{titlepage}\n  \\centering\n\n");
        emit_title_page(&mut out, title_elems);
        out.push_str("\\end{titlepage}\n\n");

        // Configure fancyhdr after the title page so it doesn't interfere
        // with the title page's own layout (avoids "Object @page already defined").
        out.push_str("\\pagestyle{fancy}\n");
        out.push_str("\\fancyhf{}\n");
        out.push_str("\\fancyhead[L]{\\small\\nouppercase{\\leftmark}}\n");
        out.push_str("\\fancyhead[R]{\\thepage}\n");
        out.push_str("\\fancyfoot[C]{}\n\n");

        // ── Render the rest of the document ─────────────────────────────────
        let mut last_was_page_break = false;
        for element in body_elems {
            emit_element(&mut out, element, template, &mut seen_numbered_chapter, last_was_page_break);
            last_was_page_break = matches!(element, Element::PageBreak);
            out.push('\n');
        }
    } else {
        let mut last_was_page_break = false;
        for element in &doc.elements {
            emit_element(&mut out, element, template, &mut seen_numbered_chapter, last_was_page_break);
            last_was_page_break = matches!(element, Element::PageBreak);
            out.push('\n');
        }
    }

    // 4. End document
    out.push_str("\\end{document}\n");
    out
}

// ── Preamble ──────────────────────────────────────────────────────────────────

fn emit_preamble(out: &mut String, doc: &Document, template: &Template) {
    match template {
        Template::Default => {
            out.push_str("\\documentclass[12pt,a4paper]{article}\n");
            out.push_str("\\usepackage[T1]{fontenc}\n");
            out.push_str("\\usepackage[utf8]{inputenc}\n");
            out.push_str("\\usepackage[margin=2.5cm]{geometry}\n");
            out.push_str("\\usepackage{graphicx}\n");
            out.push_str("\\graphicspath{{./}}\n");
            out.push_str("\\usepackage{float}\n");
            out.push_str("\\usepackage{array}\n");
            out.push_str("\\usepackage{booktabs}\n");
            out.push_str("\\usepackage[hyphens]{url}\n");
            out.push_str("\\usepackage{hyperref}\n");
            out.push_str("\\usepackage{parskip}\n");
            out.push_str("\\usepackage{microtype}\n");
            out.push_str("\\usepackage[normalem]{ulem}\n");
            out.push_str("\\usepackage{textcomp}\n");
            out.push_str("\\usepackage{amsmath}\n");
            out.push_str("\\usepackage{amssymb}\n");
        }
        Template::Ieee => {
            out.push_str("\\documentclass[conference]{IEEEtran}\n");
            out.push_str("\\usepackage[T1]{fontenc}\n");
            out.push_str("\\usepackage[utf8]{inputenc}\n");
            out.push_str("\\usepackage{graphicx}\n");
            out.push_str("\\graphicspath{{./}}\n");
            out.push_str("\\usepackage{float}\n");
            out.push_str("\\usepackage{array}\n");
            out.push_str("\\usepackage[normalem]{ulem}\n");
            out.push_str("\\usepackage{textcomp}\n");
            out.push_str("\\usepackage{amsmath}\n");
            out.push_str("\\usepackage{amssymb}\n");
            out.push_str("\\usepackage{cite}\n");
        }
        Template::Thesis => {
            out.push_str("\\documentclass[12pt,a4paper,oneside]{report}\n");
            out.push_str("\\usepackage[T1]{fontenc}\n");
            out.push_str("\\usepackage[utf8]{inputenc}\n");
            out.push_str("\\usepackage[margin=3cm]{geometry}\n");
            out.push_str("\\usepackage{graphicx}\n");
            out.push_str("\\graphicspath{{./}}\n");
            out.push_str("\\usepackage{float}\n");
            out.push_str("\\usepackage{array}\n");
            out.push_str("\\usepackage{booktabs}\n");
            out.push_str("\\usepackage[hyphens]{url}\n");
            out.push_str("\\usepackage{hyperref}\n");
            out.push_str("\\usepackage{natbib}\n");
            out.push_str("\\usepackage{fancyhdr}\n");
            out.push_str("\\usepackage{microtype}\n");
            out.push_str("\\usepackage[normalem]{ulem}\n");
            out.push_str("\\usepackage{textcomp}\n");
            out.push_str("\\usepackage{amsmath}\n");
            out.push_str("\\usepackage{amssymb}\n");
            // Paragraph spacing — no indent, 6pt vertical gap between paragraphs
            out.push_str("\\setlength{\\parindent}{0pt}\n");
            out.push_str("\\setlength{\\parskip}{6pt}\n");
            // Rename the built-in "List of Figures/Tables" headings to match
            // academic thesis conventions (all-caps, same style as the document).
            // This prevents \listoffigures/\listoftables from emitting a second
            // lower-case heading after our own \chapter*{} call.
            out.push_str("\\renewcommand{\\listfigurename}{LIST OF FIGURES}\n");
            out.push_str("\\renewcommand{\\listtablename}{LIST OF TABLES}\n");
        }
    }
    out.push('\n');

    // Global line-break relaxation — prevents overfull \hbox on long words/URLs.
    // Thesis uses a larger value + \sloppy because academic docs often contain
    // long URLs, identifiers and technical terms that don't break naturally.
    match template {
        Template::Thesis => {
            out.push_str("\\setlength{\\emergencystretch}{5em}\n");
            out.push_str("\\sloppy\n");
        }
        _ => {
            out.push_str("\\setlength{\\emergencystretch}{3em}\n");
        }
    }

    // Title / author block — only needed for non-thesis templates.
    // Thesis documents generate their own title page from the Word source.
    match template {
        Template::Thesis => {}
        _ => {
            let title = doc
                .title
                .as_deref()
                .map(escaper::escape)
                .unwrap_or_else(|| "Untitled".to_owned());
            out.push_str(&format!("\\title{{{title}}}\n"));
            out.push_str("\\author{}\n");
            out.push_str("\\date{\\today}\n");
        }
    }
    out.push('\n');
}

// ── Element rendering ─────────────────────────────────────────────────────────

fn emit_element(out: &mut String, element: &Element, template: &Template, seen_numbered_chapter: &mut bool, prev_was_page_break: bool) {
    match element {
        Element::Heading { level, runs } => {
            // Strip Word's embedded numbering prefix (e.g. "CHAPTER 1:", "1.1 ")
            // so LaTeX's own auto-numbering doesn't double up.
            let stripped = strip_runs_prefix(runs);
            let runs = &stripped;

            match (template, level) {
                // ── Thesis: level 1 is always \chapter{} ─────────────────────
                (Template::Thesis, 1) => {
                    *seen_numbered_chapter = true;
                    out.push_str("\\chapter{");
                    emit_runs(out, runs);
                    out.push_str("}\n");
                }

                // ── Thesis: level 2 — front matter → \chapter*{} + TOC entry;
                //                     body section    → \section{}
                (Template::Thesis, 2) => {
                    let text: String = runs.iter().map(|r| r.text.as_str()).collect();
                    let text = text.trim();

                    // A heading is treated as "front matter" if it either matches
                    // a known keyword OR appears before the first numbered chapter.
                    if is_front_matter_heading(text) || !*seen_numbered_chapter {
                        let escaped = escaper::escape(text);
                        out.push_str(&format!("\\chapter*{{{escaped}}}\n"));
                        // \chapter*{} does not add to the TOC automatically —
                        // emit \addcontentsline so readers can find these sections.
                        out.push_str(&format!(
                            "\\addcontentsline{{toc}}{{chapter}}{{{escaped}}}\n"
                        ));
                    } else {
                        out.push_str("\\section{");
                        emit_runs(out, runs);
                        out.push_str("}\n");
                    }
                }

                // ── Thesis: deeper levels ────────────────────────────────────
                (Template::Thesis, 3) => {
                    out.push_str("\\subsection{");
                    emit_runs(out, runs);
                    out.push_str("}\n");
                }
                (Template::Thesis, _) => {
                    out.push_str("\\subsubsection{");
                    emit_runs(out, runs);
                    out.push_str("}\n");
                }

                // ── Non-thesis templates: use heading_command() as before ────
                _ => {
                    let cmd = heading_command(*level, template);
                    out.push_str(&format!("\\{cmd}{{"));
                    emit_runs(out, runs);
                    out.push_str("}\n");
                }
            }
        }

        Element::Paragraph { runs } => {
            if runs.is_empty() {
                out.push('\n');
            } else {
                // In thesis mode, a paragraph whose entire text exactly matches
                // a known front-matter keyword (e.g. "Declaration of Authorship",
                // "Approval", "Dedication") should be treated as \chapter*{}.
                // Word documents often format these as bold paragraphs rather
                // than with a proper Heading style.
                if template == &Template::Thesis {
                    let text: String = runs.iter().map(|r| r.text.as_str()).collect();
                    if is_front_matter_heading(text.trim()) {
                        let escaped = escaper::escape(text.trim());
                        // Only emit \clearpage if the preceding element was NOT
                        // already a PageBreak (which emits its own \clearpage).
                        if !prev_was_page_break {
                            out.push_str("\\clearpage\n");
                        }
                        out.push_str(&format!("\\chapter*{{{escaped}}}\n"));
                        out.push_str(&format!(
                            "\\addcontentsline{{toc}}{{chapter}}{{{escaped}}}\n"
                        ));
                        return;
                    }
                }
                emit_runs(out, runs);
                out.push_str("\n\n");
            }
        }

        Element::List { ordered, items } => {
            emit_nested_list(out, *ordered, items);
        }

        Element::Table { rows, caption } => {
            emit_table(out, rows, template, caption.as_deref());
        }

        Element::Image { path, width_cm, caption } => {
            // Skip image formats that pdfLaTeX/tectonic cannot process.
            // EMF/WMF are Windows vector formats with no BoundingBox info;
            // SVG requires a special package; TIFF/BMP have poor support.
            // Emitting a comment keeps the document compilable.
            let ext = std::path::Path::new(path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if matches!(ext.as_str(), "emf" | "wmf" | "svg" | "tiff" | "tif" | "bmp") {
                out.push_str(&format!(
                    "% [image skipped: '{}' is not supported by pdfLaTeX — convert to PNG/PDF first]\n",
                    path
                ));
                return;
            }

            let max_w = max_text_width_cm(template);
            let width_str = match width_cm {
                Some(w) if *w <= max_w => format!("{:.2}cm", w),
                Some(_) => "\\linewidth".to_owned(),   // oversized → full width
                None    => "\\linewidth".to_owned(),   // unknown → full width
            };

            out.push_str("\\begin{figure}[htbp]\n");
            out.push_str("  \\centering\n");
            out.push_str(&format!("  \\includegraphics[width={width_str}]{{{path}}}\n"));
            if let Some(cap) = caption {
                out.push_str(&format!("  \\caption{{{}}}\n", escaper::escape(cap)));
            }
            out.push_str("\\end{figure}\n");
        }

        Element::PageBreak => {
            out.push_str("\\clearpage\n");
        }

        Element::TocBlock => {
            out.push_str("\\tableofcontents\n");
            out.push_str("\\clearpage\n");
        }

        Element::LofBlock => {
            // \phantomsection creates the hyperref anchor at the right spot so
            // the TOC hyperlink jumps to the correct page.
            // \listoffigures itself emits \chapter*{\listfigurename} — we renamed
            // that to "LIST OF FIGURES" in the preamble, so no second heading here.
            out.push_str("\\phantomsection\n");
            out.push_str("\\addcontentsline{toc}{chapter}{LIST OF FIGURES}\n");
            out.push_str("\\listoffigures\n");
            out.push_str("\\clearpage\n");
        }

        Element::LotBlock => {
            out.push_str("\\phantomsection\n");
            out.push_str("\\addcontentsline{toc}{chapter}{LIST OF TABLES}\n");
            out.push_str("\\listoftables\n");
            out.push_str("\\clearpage\n");
        }
    }
}

// ── Title page rendering ──────────────────────────────────────────────────────

/// Render the elements that belong to the thesis title page.
///
/// Heuristics applied (in priority order):
/// 1. Image → university logo, rendered small and centred.
/// 2. ALL-CAPS text ≤ 3 words that is an institution keyword → `\large \textbf`.
/// 3. ALL-CAPS text > 3 words → `\Large \textbf` (the main document title).
/// 4. Short ALL-CAPS text (≤ 3 words, not institution) → `\normalsize \textbf`.
/// 5. Text containing "Supervisor", "Registration", etc. → `\normalsize` line.
/// 6. Otherwise → `\normalsize \textit` (italicised subtitle/submission line).
///
/// A `\rule` pair is inserted around the first long ALL-CAPS heading (the title).
fn emit_title_page(out: &mut String, elements: &[Element]) {
    let mut title_emitted = false;

    for element in elements {
        match element {
            Element::Image { path, width_cm, .. } => {
                let ext = std::path::Path::new(path)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if matches!(ext.as_str(), "emf" | "wmf" | "svg" | "tiff" | "tif" | "bmp") {
                    continue; // skip unconvertible formats
                }
                let w = width_cm.map(|w| w.min(4.5)).unwrap_or(4.0);
                out.push_str(&format!(
                    "  \\includegraphics[width={w:.2}cm]{{{path}}}\\\\[0.8cm]\n\n"
                ));
            }

            Element::Paragraph { runs } => {
                // Collect the plain text for classification
                let raw_text: String = runs.iter().map(|r| r.text.as_str()).collect();
                let text = raw_text.trim();
                let text = &normalize_caps_spaces(text);
                let text = text.as_str();
                if text.is_empty() {
                    continue;
                }

                let is_all_caps = !text.chars().any(|c| c.is_lowercase())
                    && text.chars().any(|c| c.is_uppercase());
                let word_count = text.split_whitespace().count();
                let lower = text.to_lowercase();

                let is_institution = lower.contains("university")
                    || lower.contains("department")
                    || lower.contains("faculty")
                    || lower.contains("college")
                    || lower.contains("school of");

                let is_meta = lower.starts_with("registration")
                    || lower.starts_with("supervisor")
                    || lower.starts_with("by")
                    || lower.starts_with("submitted")
                    || lower.starts_with("june ")
                    || lower.starts_with("july ")
                    || lower.starts_with("august ")
                    || lower.starts_with("september ")
                    || lower.starts_with("october ")
                    || lower.starts_with("november ")
                    || lower.starts_with("december ")
                    || lower.starts_with("january ")
                    || lower.starts_with("february ")
                    || lower.starts_with("march ")
                    || lower.starts_with("april ")
                    || lower.starts_with("may 2");

                if is_all_caps && word_count > 3 && is_institution {
                    // Long institution name — bold, normalsize
                    out.push_str("{\\normalsize \\textbf{");
                    out.push_str(&escaper::escape(text));
                    out.push_str("}}\\\\[0.3cm]\n\n");
                } else if is_all_caps && !is_institution && word_count > 3 && !title_emitted {
                    // Main document title — wrap in \\rule + \Large \textbf
                    title_emitted = true;
                    out.push_str("  \\rule{\\linewidth}{0.5pt}\\\\[0.4cm]\n");
                    out.push_str("  {\\Large \\textbf{");
                    emit_runs_stripped(out, runs, true, false); // already in \textbf
                    out.push_str("}}\\\\[0.4cm]\n");
                    out.push_str("  \\rule{\\linewidth}{0.5pt}\\\\[1.2cm]\n\n");
                } else if is_all_caps && !is_institution && word_count > 3 {
                    // Subtitle / continued title
                    out.push_str("  {\\large \\textbf{");
                    emit_runs_stripped(out, runs, true, false); // already in \textbf
                    out.push_str("}}\\\\[0.5cm]\n\n");
                } else if is_all_caps && is_institution {
                    // Short institution name (e.g. "KYAMBOGO UNIVERSITY")
                    out.push_str("  {\\large \\textbf{");
                    out.push_str(&escaper::escape(text));
                    out.push_str("}}\\\\[0.3cm]\n\n");
                } else if is_all_caps {
                    // Short ALL-CAPS (author name, degree programme name, etc.)
                    out.push_str("  {\\large \\textbf{");
                    emit_runs_stripped(out, runs, true, false); // already in \textbf
                    out.push_str("}}\\\\[0.4cm]\n\n");
                } else if is_meta {
                    // Meta lines: date, supervisor, registration number, "By"
                    out.push_str("  {\\normalsize ");
                    emit_runs(out, runs);
                    out.push_str("}\\\\[0.5cm]\n\n");
                } else {
                    // Anything else: submission statement, italicised
                    out.push_str("  {\\normalsize \\textit{");
                    emit_runs_stripped(out, runs, false, true); // already in \textit
                    out.push_str("}}\\\\[1.0cm]\n\n");
                }
            }

            // Ignore page breaks and other elements inside the title page
            _ => {}
        }
    }
}

// ── Template-aware text width caps ─────────────────────────────────────────────

/// Fix ALL-CAPS text where Word merged two words without a space (e.g.
/// "KYAMBOGOUNIVERSITY" → "KYAMBOGO UNIVERSITY").
/// Tries known institution boundary words as split points.
fn normalize_caps_spaces(text: &str) -> String {
    // Only process if text has no spaces and is ALL-CAPS
    if text.contains(' ') || text.chars().any(|c| c.is_lowercase()) {
        return text.to_owned();
    }
    const BOUNDARIES: &[&str] = &[
        "UNIVERSITY",
        "POLYTECHNIC",
        "INSTITUTE",
        "COLLEGE",
        "FACULTY",
        "DEPARTMENT",
        "SCHOOL",
        "ENGINEERING",
        "TECHNOLOGY",
        "SCIENCES",
        "MANAGEMENT",
    ];
    let mut result = text.to_owned();
    for boundary in BOUNDARIES {
        if let Some(pos) = result.find(boundary) {
            if pos > 0 {
                result.insert(pos, ' ');
                // Only fix the first boundary to avoid cascading insertions on short text
                break;
            }
        }
    }
    result
}



fn max_text_width_cm(template: &Template) -> f64 {
    match template {
        Template::Default => 15.5,  // A4, 2.5cm margins each side
        Template::Ieee    => 8.89,  // IEEEtran single-column width (~3.5in)
        Template::Thesis  => 15.0,  // A4, 3cm margins each side
    }
}

// ── List rendering ────────────────────────────────────────────────────────────

/// Render a list with proper nesting based on per-item indent levels.
///
/// Consecutive items at the same level stay in the same environment.
/// A deeper level opens a new nested `\begin{env}`, a shallower level
/// closes back to the appropriate depth.
fn emit_nested_list(out: &mut String, ordered: bool, items: &[(u8, Vec<Run>)]) {
    if items.is_empty() {
        return;
    }
    let env = if ordered { "enumerate" } else { "itemize" };
    let mut depth: i32 = -1;

    for (level, runs) in items {
        let target = *level as i32;

        // Open new environments as needed
        while depth < target {
            let indent = "  ".repeat((depth + 1).max(0) as usize);
            out.push_str(&format!("{indent}\\begin{{{env}}}\n"));
            depth += 1;
        }

        // Close environments as needed
        while depth > target {
            let indent = "  ".repeat(depth.max(0) as usize);
            out.push_str(&format!("{indent}\\end{{{env}}}\n"));
            depth -= 1;
        }

        let indent = "  ".repeat((depth + 1).max(0) as usize);
        out.push_str(&format!("{indent}\\item "));
        emit_runs(out, runs);
        out.push('\n');
    }

    // Close all open environments
    while depth >= 0 {
        let indent = "  ".repeat(depth.max(0) as usize);
        out.push_str(&format!("{indent}\\end{{{env}}}\n"));
        depth -= 1;
    }
}

// ── Table rendering ───────────────────────────────────────────────────────────

/// Compute proportional column widths based on average cell text length.
///
/// Short label columns (e.g. "S/N") get less space; long content columns
/// (e.g. "Purpose" / "Remarks") get more.  Each width is clamped to
/// [1.5cm, 8.0cm] and the total is normalised to `total_width`.
fn compute_col_widths(
    rows: &[Vec<crate::models::Cell>],
    col_count: usize,
    total_width: f64,
) -> Vec<f64> {
    // Sum of character lengths per column across all rows
    let mut col_sums = vec![0usize; col_count];
    let mut col_counts = vec![0usize; col_count];

    for row in rows {
        for (i, cell) in row.iter().enumerate().take(col_count) {
            let len: usize = cell.runs.iter().map(|r| r.text.len()).sum();
            col_sums[i] += len;
            col_counts[i] += 1;
        }
    }

    // Average text length per column (at least 1 to avoid div-by-zero)
    let avgs: Vec<f64> = col_sums
        .iter()
        .zip(&col_counts)
        .map(|(&s, &n)| if n > 0 { s as f64 / n as f64 } else { 1.0 }.max(1.0))
        .collect();

    let total_avg: f64 = avgs.iter().sum();

    // Proportional widths clamped to [1.5, 8.0]
    let raw: Vec<f64> = avgs
        .iter()
        .map(|&a| ((a / total_avg) * total_width).clamp(1.5, 8.0))
        .collect();

    // Re-scale so columns sum exactly to total_width
    let raw_sum: f64 = raw.iter().sum();
    let scale = total_width / raw_sum;
    raw.into_iter().map(|w| (w * scale).clamp(1.5, 8.0)).collect()
}

fn emit_table(out: &mut String, rows: &[Vec<crate::models::Cell>], template: &Template, caption: Option<&str>) {
    if rows.is_empty() {
        return;
    }
    let col_count = rows.iter().map(|r| r.len()).max().unwrap_or(1);
    let text_width = max_text_width_cm(template);
    let col_widths = compute_col_widths(rows, col_count, text_width);
    let col_spec: String = col_widths
        .iter()
        .map(|w| format!(">{{\\raggedright\\arraybackslash}}p{{{:.2}cm}}|", w))
        .collect::<String>();

    out.push_str("\\begin{table}[htbp]\n");
    out.push_str("  \\centering\n");
    // Caption above the tabular (standard convention for tables)
    if let Some(cap) = caption {
        out.push_str(&format!("  \\caption{{{}}}\n", escaper::escape(cap)));
    }
    out.push_str("  {\\small\n");
    out.push_str(&format!("  \\begin{{tabular}}{{|{}}}\n", col_spec));

    out.push_str("    \\hline\n");

    for (row_idx, row) in rows.iter().enumerate() {
        out.push_str("    ");
        let cells: Vec<String> = row
            .iter()
            .map(|cell| {
                let mut s = String::new();
                emit_runs(&mut s, &cell.runs);
                s
            })
            .collect();
        out.push_str(&cells.join(" & "));
        out.push_str(" \\\\\n");

        // Header rule after first row
        if row_idx == 0 {
            out.push_str("    \\hline\n");
        }
    }

    out.push_str("    \\hline\n");
    out.push_str("  \\end{tabular}\n");
    out.push_str("  }\n"); // close \small
    out.push_str("\\end{table}\n");
}

// ── Run rendering ─────────────────────────────────────────────────────────────

fn emit_runs(out: &mut String, runs: &[Run]) {
    emit_runs_stripped(out, runs, false, false);
}

/// Like `emit_runs` but suppresses bold/italic wrappers when the call site
/// is already inside a `\textbf{}` or `\textit{}` environment (avoids
/// `\textbf{\textbf{...}}` double-nesting from Word's run-level formatting).
fn emit_runs_stripped(out: &mut String, runs: &[Run], strip_bold: bool, strip_italic: bool) {
    for run in runs {
        let text = escaper::escape(&run.text);

        // Wrap in formatting commands, innermost first.
        let mut s = if run.mono {
            format!("\\texttt{{{text}}}")
        } else {
            text
        };

        if run.bold && !strip_bold {
            s = format!("\\textbf{{{s}}}");
        }
        if run.italic && !strip_italic {
            s = format!("\\textit{{{s}}}");
        }
        if run.underline {
            s = format!("\\uline{{{s}}}");
        }
        if run.superscript {
            s = format!("\\textsuperscript{{{s}}}");
        }
        if run.subscript {
            s = format!("\\textsubscript{{{s}}}");
        }
        if let Some(ref url) = run.hyperlink {
            let escaped_url = escaper::escape_url(url);
            s = format!("\\href{{{escaped_url}}}{{{s}}}");
        }

        out.push_str(&s);
    }
}

// ── Front-matter keyword detection ───────────────────────────────────────────

/// Returns true if `text` matches a well-known academic front/back-matter
/// section title that should be typeset as `\chapter*{}` in thesis mode
/// (so it still starts a new page) rather than `\section{}`.
///
/// Matching is case-insensitive and trimmed.
fn is_front_matter_heading(text: &str) -> bool {
    const FRONT_MATTER: &[&str] = &[
        // Standard front matter
        "ABSTRACT",
        "DECLARATION",
        "DECLARATION OF AUTHORSHIP",
        "DECLARATION OF ORIGINALITY",
        "APPROVAL",
        "APPROVAL PAGE",
        "CERTIFICATION",
        "DEDICATION",
        "ACKNOWLEDGEMENTS",
        "ACKNOWLEDGMENTS",
        "ACKNOWLEDGEMENT",   // singular — common in African/Commonwealth theses
        "PREFACE",
        "FOREWORD",
        // Table of contents (if it survives normalisation as a heading)
        "TABLE OF CONTENTS",
        "CONTENTS",
        // Lists
        "NOMENCLATURE",
        "ACRONYMS",
        "LIST OF ACRONYMS",
        "ABBREVIATIONS",
        "LIST OF ABBREVIATIONS",
        "LIST OF FIGURES",
        "LIST OF TABLES",
        "LIST OF SYMBOLS",
        "LIST OF PLATES",
        "URL LINKS",
        // Back matter
        "REFERENCES",
        "BIBLIOGRAPHY",
        "APPENDIX",
        "APPENDICES",
    ];
    let upper = text.trim().to_uppercase();
    FRONT_MATTER.iter().any(|&kw| upper == kw)
}

// ── Heading number prefix stripping ──────────────────────────────────────────

/// Strip leading numbering prefixes that Word embeds in heading text so that
/// LaTeX's own auto-numbering doesn't double up.
///
/// Examples stripped:
///   "CHAPTER 1: INTRODUCTION"  → "INTRODUCTION"
///   "Chapter 2 - Methods"      → "Methods"
///   "1. Introduction"          → "Introduction"
///   "1.1 Background"           → "Background"
///   "1.2.3 Sub-section"        → "Sub-section"
///
/// Returns the trimmed suffix.  If no prefix is recognised the trimmed
/// original is returned unchanged.
fn strip_heading_number_prefix(text: &str) -> &str {
    let text = text.trim();
    let lower = text.to_ascii_lowercase();

    // "CHAPTER N[separator]..." — separator is one of : . - – (en-dash) — (em-dash)
    if lower.starts_with("chapter") {
        let after_word = text["chapter".len()..].trim_start();
        let after_num  = after_word.trim_start_matches(|c: char| c.is_ascii_digit());
        let after_sep  = after_num.trim_start_matches(|c: char| {
            matches!(c, ':' | '.' | '-' | '\u{2013}' | '\u{2014}' | ' ')
        });
        if !after_sep.is_empty() {
            return after_sep;
        }
        // "CHAPTER 1" with no further title — keep as-is so the chapter isn't blank
        return text;
    }

    // "N.", "N.N ", "N.N.N " etc. — all dot-separated digit groups
    let first_word = text.split_whitespace().next().unwrap_or("");
    let w = first_word.trim_end_matches('.');
    if !w.is_empty()
        && w.split('.').all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
    {
        let rest = text[first_word.len()..].trim_start();
        if !rest.is_empty() {
            return rest;
        }
    }

    text
}

/// Apply `strip_heading_number_prefix` to a run slice, stripping the
/// appropriate number of bytes from the leading runs.
fn strip_runs_prefix(runs: &[Run]) -> Vec<Run> {
    let combined: String = runs.iter().map(|r| r.text.as_str()).collect();
    let stripped = strip_heading_number_prefix(&combined);
    // Byte offset of the stripped slice within `combined`
    let skip = stripped.as_ptr() as usize - combined.as_ptr() as usize;

    if skip == 0 {
        return runs.to_vec();
    }

    let mut result = runs.to_vec();
    let mut remaining = skip;
    for run in &mut result {
        if remaining == 0 {
            break;
        }
        let run_len = run.text.len();
        if remaining >= run_len {
            remaining -= run_len;
            run.text.clear();
        } else {
            run.text = run.text[remaining..].to_owned();
            remaining = 0;
        }
    }
    result.retain(|r| !r.text.is_empty());
    result
}

// ── Heading commands (non-thesis templates) ───────────────────────────────────

fn heading_command(level: u8, template: &Template) -> &'static str {
    match template {
        Template::Thesis => match level {
            1 => "chapter",
            2 => "section",
            3 => "subsection",
            _ => "subsubsection",
        },
        _ => match level {
            1 => "section",
            2 => "subsection",
            3 => "subsubsection",
            _ => "paragraph",
        },
    }
}
