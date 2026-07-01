//! `validate` subcommand handler.
//!
//! Parses a .docx file and prints a human-readable summary of the document
//! structure without writing any output files.

use anyhow::Context;

use crate::{
    cli_structure::ValidateArgs,
    models::Element,
    services::docx_reader,
};

pub fn run(args: ValidateArgs) -> anyhow::Result<()> {
    let input = &args.input;
    anyhow::ensure!(
        input.exists(),
        "Input file does not exist: {}",
        input.display()
    );

    // Use a process-specific temp dir for media (discarded after validation).
    let tmp = std::env::temp_dir().join(format!("docx2tex_validate_{}", std::process::id()));
    std::fs::create_dir_all(&tmp)?;

    println!("Validating {}…\n", input.display());

    let parse_result = docx_reader::parse(input, &tmp)
        .with_context(|| format!("Failed to parse {}", input.display()));
    if parse_result.is_err() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    let doc = parse_result?;

    // ── Summary counts ────────────────────────────────────────────────────────
    let mut headings = 0u32;
    let mut paragraphs = 0u32;
    let mut lists = 0u32;
    let mut tables = 0u32;
    let mut images = 0u32;
    let mut page_breaks = 0u32;
    let mut toc_blocks = 0u32;

    for el in &doc.elements {
        match el {
            Element::Heading { .. }   => headings += 1,
            Element::Paragraph { .. } => paragraphs += 1,
            Element::List { .. }      => lists += 1,
            Element::Table { .. }     => tables += 1,
            Element::Image { .. }     => images += 1,
            Element::PageBreak        => page_breaks += 1,
            Element::TocBlock | Element::LofBlock | Element::LotBlock => toc_blocks += 1,
        }
    }

    println!("Document Summary");
    println!("================");
    if let Some(ref title) = doc.title {
        println!("  Title      : {title}");
    }
    println!("  Headings   : {headings}");
    println!("  Paragraphs : {paragraphs}");
    println!("  Lists      : {lists}");
    println!("  Tables     : {tables}");
    println!("  Images     : {images}");
    println!("  Page breaks: {page_breaks}");
    if toc_blocks > 0 {
        println!("  TOC blocks : {toc_blocks}");
    }
    println!();

    // ── Element walkthrough ───────────────────────────────────────────────────
    println!("Structure");
    println!("---------");
    for (i, el) in doc.elements.iter().enumerate() {
        let idx = i + 1;
        match el {
            Element::Heading { level, runs } => {
                let text: String = runs.iter().map(|r| r.text.as_str()).collect();
                println!("  [{idx:>3}] H{level}: \"{text}\"");
            }
            Element::Paragraph { runs } => {
                let preview: String = runs.iter().map(|r| r.text.as_str()).collect();
                let preview = truncate(&preview, 60);
                println!("  [{idx:>3}] Paragraph: \"{preview}\"");
            }
            Element::List { ordered, items } => {
                let kind = if *ordered { "ordered" } else { "unordered" };
                let max_level = items.iter().map(|(lvl, _)| *lvl).max().unwrap_or(0);
                println!(
                    "  [{idx:>3}] List ({kind}, max depth {max_level}): {} item(s)",
                    items.len()
                );
            }
            Element::Table { rows, caption } => {
                let cols = rows.first().map(|r| r.len()).unwrap_or(0);
                if let Some(cap) = caption {
                    let preview = &cap[..cap.len().min(50)];
                    println!("  [{idx:>3}] Table: {} row(s) × {cols} col(s) — \"{preview}\"", rows.len());
                } else {
                    println!("  [{idx:>3}] Table: {} row(s) × {cols} col(s)", rows.len());
                }
            }
            Element::Image { path, .. } => {
                println!("  [{idx:>3}] Image: {path}");
            }
            Element::PageBreak => {
                println!("  [{idx:>3}] PageBreak");
            }
            Element::TocBlock => {
                println!("  [{idx:>3}] TableOfContents (auto-generated)");
            }
            Element::LofBlock => {
                println!("  [{idx:>3}] ListOfFigures (auto-generated)");
            }
            Element::LotBlock => {
                println!("  [{idx:>3}] ListOfTables (auto-generated)");
            }
        }
    }

    // Clean up temp dir (best-effort)
    let _ = std::fs::remove_dir_all(&tmp);

    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}
