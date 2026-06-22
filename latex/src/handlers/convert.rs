//! `convert` subcommand handler.
//!
//! Orchestrates the full pipeline:
//!   1. Resolve output directory
//!   2. Parse the .docx into a Document AST
//!   3. Render the AST to LaTeX
//!   4. Write the .tex file (and copy media/)
//!   5. Optionally compile to PDF

use anyhow::Context;


use crate::{
    cli_structure::ConvertArgs,
    generated_pdfs,
    services::{docx_reader, latex_renderer},
};


pub fn run(args: ConvertArgs) -> anyhow::Result<()> {
    // ── 1. Resolve paths ──────────────────────────────────────────────────────
    let input = &args.input;
    anyhow::ensure!(
        input.exists(),
        "Input file does not exist: {}",
        input.display()
    );
    anyhow::ensure!(
        input.extension().and_then(|e| e.to_str()) == Some("docx"),
        "Input file must have a .docx extension, got: {}",
        input.display()
    );

    let output_dir = match &args.output {
        Some(dir) => dir.clone(),
        None => input
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf(),
    };
    std::fs::create_dir_all(&output_dir)
        .with_context(|| format!("Cannot create output directory: {}", output_dir.display()))?;

    let mut stem = input
        .file_stem()
        .context("Input file has no stem")?
        .to_string_lossy()
        .into_owned();
    
    // Sanitize the stem to avoid TeX engine bugs with spaces in filenames
    stem = stem.replace(' ', "_");

    let tex_path = output_dir.join(format!("{stem}.tex"));

    // ── 2. Parse ──────────────────────────────────────────────────────────────
    println!("Parsing {}…", input.display());
    let doc = docx_reader::parse(input, &output_dir)
        .with_context(|| format!("Failed to parse {}", input.display()))?;

    let elem_count = doc.elements.len();
    println!(
        "  Parsed {} block element{}.",
        elem_count,
        if elem_count == 1 { "" } else { "s" }
    );

    // ── 3. Render ─────────────────────────────────────────────────────────────
    println!("Rendering LaTeX ({} template)…", template_name(&args.template));
    let latex = latex_renderer::render(&doc, &args.template);

    // ── 4. Write .tex ─────────────────────────────────────────────────────────
    std::fs::write(&tex_path, &latex)
        .with_context(|| format!("Failed to write {}", tex_path.display()))?;
    println!("  Written → {}", tex_path.display());

    // ── 5. Compile (optional) ─────────────────────────────────────────────────
    if args.compile_pdf {
        println!("Compiling PDF…");
        let pdf_path = generated_pdfs::compile(&tex_path, &output_dir)?;
        println!("  PDF     → {}", pdf_path.display());
    }

    println!("Done.");
    Ok(())
}

fn template_name(t: &crate::cli_structure::Template) -> &'static str {
    use crate::cli_structure::Template;
    match t {
        Template::Default => "default",
        Template::Ieee    => "ieee",
        Template::Thesis  => "thesis",
    }
}


