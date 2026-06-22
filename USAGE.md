# docx2tex — CLI Usage Guide

**docx2tex** converts Microsoft Word documents (`.docx`) into clean LaTeX source files, with optional PDF compilation and three academic template presets.

## Why generate PDFs with docx2tex instead of Word?

When you "Save as PDF" directly from Word, any messy manual formatting (like inconsistent margins or slightly mismatched font sizes) gets baked directly into the document. Word also calculates spacing line-by-line, often resulting in awkward gaps between words.

By generating a PDF via LaTeX through **docx2tex**, you gain several "superpowers":
1. **Superior Typography**: LaTeX uses an advanced global paragraph-breaking algorithm to calculate the most mathematically beautiful way to hyphenate words and space letters (kerning).
2. **Strict Templates**: By passing a template (like `ieee`), the engine strips out all the messy manual styling from Word and forcefully recalculates the entire document to perfectly match professional academic standards.
3. **Automatic Ligatures**: LaTeX automatically merges tricky letter combinations (like 'f' and 'i') into beautiful single glyphs (ligatures).
4. **Beautiful Math**: If you incorporate math, LaTeX is the undisputed gold standard for rendering complex equations cleanly.

---

## Installation

### From source

```bash
git clone git@github.com:muhwezim78/latex_generator.git
cd latex_generator/latex
cargo build --release
```

The binary will be at `target/release/docx2tex` (Linux/macOS) or `target\release\docx2tex.exe` (Windows).

### Docker (includes pdflatex — no local LaTeX install needed)

```bash
docker build -t docx2tex .
```

---

## Quick Start

```bash
# Convert a Word document to LaTeX
docx2tex convert thesis.docx

# Convert with a specific template and output directory
docx2tex convert thesis.docx --template thesis --output ./output

# Convert and compile straight to PDF
docx2tex convert paper.docx --template ieee --output ./out --compile-pdf

# Inspect document structure without writing any files
docx2tex validate report.docx
```

---

## Commands

### `convert` — Convert a `.docx` to `.tex`

```
docx2tex convert <INPUT> [OPTIONS]
```

| Argument | Type | Default | Description |
|---|---|---|---|
| `<INPUT>` | path | *(required)* | Path to the `.docx` file |
| `--output <DIR>` | path | same dir as input | Directory to write output files into |
| `--template <NAME>` | enum | `default` | LaTeX template preset (see below) |
| `--compile-pdf` | flag | off | Compile the `.tex` to PDF after conversion |

**What it produces:**

```
output/
├── my_document.tex      ← generated LaTeX source
└── media/
    ├── image1.png       ← extracted images (if any)
    └── image2.jpg
```

If `--compile-pdf` is used and `pdflatex` or `tectonic` is on your PATH:

```
output/
├── my_document.tex
├── my_document.pdf      ← compiled PDF
└── media/
```

---

### `validate` — Inspect document structure

```
docx2tex validate <INPUT>
```

Parses the `.docx` and prints a human-readable summary. No files are written.

**Example output:**

```
Validating thesis.docx…

Document Summary
================
  Title      : My Doctoral Thesis
  Headings   : 12
  Paragraphs : 87
  Lists      : 5
  Tables     : 3
  Images     : 8
  Page breaks: 2

Structure
---------
  [  1] H1: "Introduction"
  [  2] Paragraph: "This thesis investigates the effect of…"
  [  3] H2: "Background"
  [  4] Paragraph: "Prior work by Smith et al. (2019) established…"
  [  5] List (unordered, level 0): 4 item(s)
  [  6] Table: 3 row(s) × 4 col(s)
  [  7] Image: media/figure1.png
  ...
```

---

## Template Presets

Pass the template name via `--template <NAME>`.

### `default` *(default)*

General-purpose academic document.

```
documentclass: article (12pt, A4)
packages:      geometry, graphicx, booktabs, hyperref, parskip, microtype
headings:      \section → \subsection → \subsubsection → \paragraph
tables:        booktabs (toprule / midrule / bottomrule)
```

```bash
docx2tex convert report.docx --template default
```

---

### `ieee`

IEEE Transactions / Conference format — strict two-column layout.

> **⚠️ Warning about layout:** Because this is a two-column format, short documents (like a 1-page letter) will only fill up the left side of the page, leaving the right side blank. If you want your text to stretch across the full page, use the `default` template instead!

```
documentclass: IEEEtran (conference mode)
packages:      graphicx, amsmath, amssymb, cite, hyperref
headings:      \section → \subsection → \subsubsection → \paragraph
tables:        plain \hline rules (IEEE style)
```

```bash
docx2tex convert paper.docx --template ieee --output ./ieee_out
```

> **Note:** The IEEE class requires the `IEEEtran.cls` file to be on your LaTeX path. It is included in TeX Live (`texlive-publishers`) and MiKTeX.

---

### `thesis`

Single-sided academic thesis with chapter-level structure.

```
documentclass: report (12pt, A4, oneside)
packages:      geometry, graphicx, booktabs, hyperref, natbib, fancyhdr, microtype
headings:      \chapter → \section → \subsection → \subsubsection
tables:        booktabs (toprule / midrule / bottomrule)
headers:       fancy (fancyhdr)
```

```bash
docx2tex convert mythesis.docx --template thesis --output ./thesis_out --compile-pdf
```

---

## PDF Compilation (`--compile-pdf`)

When this flag is set, `docx2tex` will attempt to invoke a LaTeX compiler automatically.

**Compiler priority:**
1. `pdflatex` — run twice (two-pass) to resolve cross-references, TOC, labels
2. `tectonic` — single-pass fallback; faster and auto-downloads packages

If neither is found you will see:

```
Error: No LaTeX compiler found on PATH.
Install pdflatex (e.g. via TeX Live or MiKTeX) or tectonic, then retry with --compile-pdf.
```

**Installing a LaTeX compiler:**

| Platform | Recommended |
|---|---|
| Windows | [MiKTeX](https://miktex.org/) or [TeX Live](https://tug.org/texlive/) |
| macOS | `brew install --cask mactex` or [tectonic](https://tectonic-typesetting.github.io) |
| Linux | `apt install texlive-full` or `cargo install tectonic` |
| Docker | Use the provided `DockerFile` — pdflatex included |

---

## Docker Usage

Mount your working directory as `/data` inside the container:

```bash
# Build the image once
docker build -t docx2tex .

# Convert (output lands in ./out on your host)
docker run --rm \
  -v $(pwd):/data \
  docx2tex convert /data/my_document.docx --output /data/out

# Convert with template + PDF compilation
docker run --rm \
  -v $(pwd):/data \
  docx2tex convert /data/paper.docx \
    --template ieee \
    --output /data/out \
    --compile-pdf

# Validate
docker run --rm \
  -v $(pwd):/data \
  docx2tex validate /data/my_document.docx
```

> On **Windows PowerShell**, replace `$(pwd)` with `${PWD}`.

---

## What Gets Converted

| Word element | LaTeX output |
|---|---|
| Heading 1–4 | `\section` / `\subsection` / etc. (template-aware) |
| Body paragraphs | Plain text paragraphs |
| **Bold** | `\textbf{}` |
| *Italic* | `\textit{}` |
| Underline | `\uline{}` (via `ulem` to support line-wrapping) |
| Superscript | `\textsuperscript{}` |
| Subscript | `\textsubscript{}` |
| Monospace / code | `\texttt{}` |
| Hyperlinks | `\href{url}{text}` |
| Bulleted lists | `itemize` environment |
| Numbered lists | `enumerate` environment |
| Tables | `tabular` with booktabs or `\hline` rules |
| Embedded images | `\includegraphics` inside `figure` float |
| Page breaks | `\clearpage` |
| Special characters | Automatically escaped (`&`, `%`, `$`, `#`, `_`, `{`, `}`, `~`, `^`, `\`) |

---

## Known Limitations

- **Footnotes / endnotes** — not yet supported (planned)
- **Equations / math blocks** — not yet supported (planned)
- **Headers / footers** — not converted (use template preamble manually)
- **Comments / tracked changes** — ignored
- **Complex nested styles** — may be simplified

---

## Full Help Reference

```
$ docx2tex --help

Convert .docx files to LaTeX

Usage: docx2tex <COMMAND>

Commands:
  convert   Convert a .docx file to .tex (and optionally to PDF)
  validate  Parse a .docx file and print a human-readable document structure summary
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

```
$ docx2tex convert --help

Convert a .docx file to .tex (and optionally to PDF)

Usage: docx2tex convert [OPTIONS] <INPUT>

Arguments:
  <INPUT>  Path to the input .docx file

Options:
  -o, --output <DIR>        Directory to write the output files into
      --template <TEMPLATE> LaTeX document template to use [default: default]
                            [possible values: default, ieee, thesis]
      --compile-pdf         Compile the generated .tex file to PDF
  -h, --help                Print help
```

```
$ docx2tex validate --help

Parse a .docx file and print a human-readable document structure summary

Usage: docx2tex validate <INPUT>

Arguments:
  <INPUT>  Path to the input .docx file

Options:
  -h, --help  Print help
```
