//! CLI argument definitions using `clap`.
//!
//! Exposes:
//!   - `Cli`      — the top-level parser (subcommands)
//!   - `Template` — the `--template` enum
//!   - `Command`  — the subcommand discriminant

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

// ── Entry point ───────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(
    name = "docx2tex",
    version,
    about = "Convert .docx files to LaTeX",
    long_about = "A fast, offline converter that turns Word documents (.docx) \
                  into clean LaTeX source files, with optional PDF compilation."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

// ── Subcommands ───────────────────────────────────────────────────────────────

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Convert a .docx file to .tex (and optionally to PDF).
    Convert(ConvertArgs),

    /// Parse a .docx file and print a human-readable document structure summary.
    Validate(ValidateArgs),
}

// ── Convert args ──────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct ConvertArgs {
    /// Path to the input .docx file.
    pub input: PathBuf,

    /// Directory to write the output files into.
    /// Defaults to the same directory as the input file.
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// LaTeX document template to use.
    #[arg(long, default_value = "default", value_name = "TEMPLATE")]
    pub template: Template,

    /// Compile the generated .tex file to PDF using pdflatex or tectonic.
    #[arg(long)]
    pub compile_pdf: bool,
}

// ── Validate args ─────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct ValidateArgs {
    /// Path to the input .docx file.
    pub input: PathBuf,
}

// ── Template preset ───────────────────────────────────────────────────────────

#[derive(ValueEnum, Debug, Clone, Default, PartialEq, Eq)]
pub enum Template {
    /// General-purpose article (12pt, A4, sensible margins).
    #[default]
    Default,

    /// IEEE Transactions / Conference format (IEEEtran, two-column).
    Ieee,

    /// Academic thesis (report class, fancy headers, natbib).
    Thesis,
}
