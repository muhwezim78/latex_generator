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

    // Maketitle
    out.push_str("\\maketitle\n\n");

    // 3. Body
    for element in &doc.elements {
        emit_element(&mut out, element, template);
        out.push('\n');
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
            out.push_str("\\usepackage{booktabs}\n");
            out.push_str("\\usepackage{hyperref}\n");
            out.push_str("\\usepackage{parskip}\n");
            out.push_str("\\usepackage{microtype}\n");
            out.push_str("\\usepackage[normalem]{ulem}\n");
        }
        Template::Ieee => {
            out.push_str("\\documentclass[conference]{IEEEtran}\n");
            out.push_str("\\usepackage[T1]{fontenc}\n");
            out.push_str("\\usepackage[utf8]{inputenc}\n");
            out.push_str("\\usepackage{graphicx}\n");
            out.push_str("\\usepackage{amsmath}\n");
            out.push_str("\\usepackage{amssymb}\n");
            out.push_str("\\usepackage{cite}\n");
            out.push_str("\\usepackage{hyperref}\n");
            out.push_str("\\usepackage[normalem]{ulem}\n");
        }
        Template::Thesis => {
            out.push_str("\\documentclass[12pt,a4paper,oneside]{report}\n");
            out.push_str("\\usepackage[T1]{fontenc}\n");
            out.push_str("\\usepackage[utf8]{inputenc}\n");
            out.push_str("\\usepackage[margin=3cm]{geometry}\n");
            out.push_str("\\usepackage{graphicx}\n");
            out.push_str("\\usepackage{booktabs}\n");
            out.push_str("\\usepackage{hyperref}\n");
            out.push_str("\\usepackage{natbib}\n");
            out.push_str("\\usepackage{fancyhdr}\n");
            out.push_str("\\usepackage{microtype}\n");
            out.push_str("\\pagestyle{fancy}\n");
            out.push_str("\\usepackage[normalem]{ulem}\n");
        }
    }
    out.push('\n');

    // Title / author block
    let title = doc
        .title
        .as_deref()
        .map(escaper::escape)
        .unwrap_or_else(|| "Untitled".to_owned());
    out.push_str(&format!("\\title{{{title}}}\n"));
    out.push_str("\\author{}\n");
    out.push_str("\\date{\\today}\n");
    out.push('\n');
}

// ── Element rendering ─────────────────────────────────────────────────────────

fn emit_element(out: &mut String, element: &Element, template: &Template) {
    match element {
        Element::Heading { level, runs } => {
            let cmd = heading_command(*level, template);
            out.push_str(&format!("\\{cmd}{{"));
            emit_runs(out, runs);
            out.push_str("}\n");
        }

        Element::Paragraph { runs } => {
            if runs.is_empty() {
                out.push('\n');
            } else {
                emit_runs(out, runs);
                out.push_str("\n\n");
            }
        }

        Element::List { ordered, items, .. } => {
            let env = if *ordered { "enumerate" } else { "itemize" };
            out.push_str(&format!("\\begin{{{env}}}\n"));
            for item in items {
                out.push_str("  \\item ");
                emit_runs(out, item);
                out.push('\n');
            }
            out.push_str(&format!("\\end{{{env}}}\n"));
        }

        Element::Table { rows } => {
            emit_table(out, rows, template);
        }

        Element::Image { path, width_cm, caption } => {
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
    }
}

// ── Template-aware text width caps ─────────────────────────────────────────────

fn max_text_width_cm(template: &Template) -> f64 {
    match template {
        Template::Default => 15.5,  // A4, 2.5cm margins each side
        Template::Ieee    => 8.89,  // IEEEtran single-column width (~3.5in)
        Template::Thesis  => 15.0,  // A4, 3cm margins each side
    }
}

// ── Table rendering ───────────────────────────────────────────────────────────

fn emit_table(out: &mut String, rows: &[Vec<crate::models::Cell>], template: &Template) {
    if rows.is_empty() {
        return;
    }
    let col_count = rows.iter().map(|r| r.len()).max().unwrap_or(1);
    let col_spec = "l ".repeat(col_count);

    out.push_str("\\begin{table}[htbp]\n");
    out.push_str("  \\centering\n");
    out.push_str(&format!("  \\begin{{tabular}}{{{}}}\n", col_spec.trim()));

    let use_booktabs = matches!(template, Template::Default | Template::Thesis);

    if use_booktabs {
        out.push_str("    \\toprule\n");
    } else {
        out.push_str("    \\hline\n");
    }

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
            if use_booktabs {
                out.push_str("    \\midrule\n");
            } else {
                out.push_str("    \\hline\n");
            }
        }
    }

    if use_booktabs {
        out.push_str("    \\bottomrule\n");
    } else {
        out.push_str("    \\hline\n");
    }

    out.push_str("  \\end{tabular}\n");
    out.push_str("\\end{table}\n");
}

// ── Run rendering ─────────────────────────────────────────────────────────────

fn emit_runs(out: &mut String, runs: &[Run]) {
    for run in runs {
        let text = escaper::escape(&run.text);

        // Wrap in formatting commands, innermost first.
        let mut s = if run.mono {
            format!("\\texttt{{{text}}}")
        } else {
            text
        };

        if run.bold {
            s = format!("\\textbf{{{s}}}");
        }
        if run.italic {
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

// ── Heading commands ──────────────────────────────────────────────────────────

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
