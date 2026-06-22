//! Invoke an installed LaTeX compiler (`pdflatex` or `tectonic`) to compile a
//! `.tex` file and produce a `.pdf` in `output_dir`.
//!
//! Returns the path to the generated PDF on success.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{bail, Context};

// ── Public entry point ────────────────────────────────────────────────────────

/// Compile `tex_path` to PDF.  Output is written into `output_dir`.
///
/// The function first tries `pdflatex`, then falls back to `tectonic`.
/// If neither is found on `$PATH`, an informative error is returned.
pub fn compile(tex_path: &Path, output_dir: &Path) -> anyhow::Result<PathBuf> {
    // Ensure output dir exists.
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("Failed to create output directory: {}", output_dir.display()))?;

    if which("pdflatex") {
        run_pdflatex(tex_path, output_dir)
    } else if which("tectonic") {
        run_tectonic(tex_path, output_dir)
    } else {
        bail!(
            "No LaTeX compiler found on PATH.\n\
             Install pdflatex (e.g. via TeX Live or MiKTeX) or tectonic, \
             then retry with --compile-pdf."
        )
    }
}

// ── Compiler backends ─────────────────────────────────────────────────────────

fn run_pdflatex(tex_path: &Path, output_dir: &Path) -> anyhow::Result<PathBuf> {
    // Run twice to resolve cross-references (TOC, labels, etc.)
    for pass in 1..=2 {
        let status = Command::new("pdflatex")
            .args([
                "-interaction=nonstopmode",
                "-halt-on-error",
                &format!("-output-directory={}", output_dir.display()),
                &tex_path.display().to_string(),
            ])
            .status()
            .with_context(|| format!("pdflatex pass {pass} failed to start"))?;

        if !status.success() {
            bail!(
                "pdflatex pass {pass} failed (exit code {:?}).\n\
                 Check the .log file in {} for details.",
                status.code(),
                output_dir.display()
            );
        }
    }

    pdf_output_path(tex_path, output_dir)
}

fn run_tectonic(tex_path: &Path, output_dir: &Path) -> anyhow::Result<PathBuf> {
    let status = Command::new("tectonic")
        .args([
            "--outdir",
            &output_dir.display().to_string(),
            &tex_path.display().to_string(),
        ])
        .status()
        .context("tectonic failed to start")?;

    if !status.success() {
        bail!(
            "tectonic failed (exit code {:?}).",
            status.code()
        );
    }

    pdf_output_path(tex_path, output_dir)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Check whether `program` exists on `$PATH`.
fn which(program: &str) -> bool {
    Command::new(if cfg!(windows) { "where" } else { "which" })
        .arg(program)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Derive the expected PDF output path from the `.tex` input path and output dir.
fn pdf_output_path(tex_path: &Path, output_dir: &Path) -> anyhow::Result<PathBuf> {
    let stem = tex_path
        .file_stem()
        .context("Input file has no stem")?
        .to_string_lossy();
    Ok(output_dir.join(format!("{stem}.pdf")))
}
