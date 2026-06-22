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

    // Use a temp dir for media (discarded after validation).
    let tmp = std::env::temp_dir().join("docx2tex_validate");
    std::fs::create_dir_all(&tmp)?;

    println!("Validating {}…\n", input.display());

    let doc = docx_reader::parse(input, &tmp)
        .with_context(|| format!("Failed to parse {}", input.display()))?;

    // ── Summary counts ────────────────────────────────────────────────────────
    let mut headings = 0u32;
    let mut paragraphs = 0u32;
    let mut lists = 0u32;
    let mut tables = 0u32;
    let mut images = 0u32;
    let mut page_breaks = 0u32;

    for el in &doc.elements {
        match el {
            Element::Heading { .. }   => headings += 1,
            Element::Paragraph { .. } => paragraphs += 1,
            Element::List { .. }      => lists += 1,
            Element::Table { .. }     => tables += 1,
            Element::Image { .. }     => images += 1,
            Element::PageBreak        => page_breaks += 1,
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
            Element::List { ordered, items, level } => {
                let kind = if *ordered { "ordered" } else { "unordered" };
                println!(
                    "  [{idx:>3}] List ({kind}, level {level}): {} item(s)",
                    items.len()
                );
            }
            Element::Table { rows } => {
                let cols = rows.first().map(|r| r.len()).unwrap_or(0);
                println!("  [{idx:>3}] Table: {} row(s) × {cols} col(s)", rows.len());
            }
            Element::Image { path, .. } => {
                println!("  [{idx:>3}] Image: {path}");
            }
            Element::PageBreak => {
                println!("  [{idx:>3}] PageBreak");
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
