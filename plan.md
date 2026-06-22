# DOCX → LaTeX Converter Plan

## Research summary

A `.docx` file is not a plain text document. It is a ZIP archive containing XML parts such as:

- `word/document.xml` for body content
- `word/styles.xml` for paragraph and character styles
- `word/numbering.xml` for list numbering
- `word/media/` for embedded images
- `[Content_Types].xml` and relationships metadata

This means the converter can be built in Rust without needing to rely on external document libraries for the basic pipeline.

## Recommended technical direction

### Core crates

- `zip` for reading the document package
- `quick-xml` for parsing the XML structure
- `anyhow` for error handling
- `serde` / `serde_json` only if we decide to support config or metadata output
- optional `thiserror` for cleaner domain-specific errors

### Suggested architecture

```text
DOCX file
  ↓
ZIP reader
  ↓
XML parser
  ↓
Document AST
  ↓
LaTeX renderer
  ↓
.output.tex + extracted assets
```

The AST layer is important because it lets us support multiple outputs later (`LaTeX`, `Markdown`, `HTML`, etc.) without rewriting parsing logic.

## Proposed data model

A small internal representation should be enough for the first version:

```rust
struct Document {
    elements: Vec<Element>,
}

enum Element {
    Heading { level: u8, runs: Vec<Run> },
    Paragraph { runs: Vec<Run> },
    List { level: u8, items: Vec<Vec<Run>> },
    Table { rows: Vec<Vec<Cell>> },
    Image { path: String, width: Option<String> },
    PageBreak,
}
```

Run-level formatting can capture:

- bold
- italic
- underline
- superscript/subscript
- hyperlinks
- inline code / monospace if needed later

## MVP scope

The first working version should support:

1. Paragraphs
2. Headings (`H1`–`H4`)
3. Bold / italic / underline
4. Basic lists
5. Simple tables
6. Embedded images
7. Proper escaping of special LaTeX characters

This is a strong first milestone because it covers the most common document content without overreaching.

## Stretch goals

After the MVP works reliably, the project can expand to:

- footnotes and endnotes
- hyperlinks and bookmarks
- page breaks and sectioning
- headers / footers
- equations and math blocks
- citations / bibliography support
- template presets (IEEE, ACM, thesis templates)

## Validation strategy

The project should include tests that compare generated LaTeX for a set of known sample documents.

Recommended test types:

- unit tests for escaping and formatting helpers
- integration tests for parsing simple XML snippets
- golden-file tests for complete `.docx` inputs

## Implementation plan

### Phase 1 — foundation

- initialize the Rust project
- add the parsing/rendering dependencies
- create a CLI that reads one `.docx` file and writes one `.tex` file
- verify the output for a minimal sample document

### Phase 2 — parser

- read `word/document.xml`
- parse paragraphs, runs, and text nodes
- identify heading styles and list structures
- capture tables and images

### Phase 3 — renderer

- convert the AST into LaTeX
- handle formatting, escapes, and whitespace correctly
- preserve document order and block boundaries

### Phase 4 — robustness

- support more Word XML structures
- extract media files correctly
- add error reporting and debug modes

### Phase 5 — optional service layer

If the tool becomes more than a CLI, a small API layer could be added later:

- upload `.docx`
- return generated `.tex` and assets
- optionally produce a ZIP package for download

## Open questions

- Should the project stay as a single CLI tool, or eventually become a web service?
- Do we need exact Word fidelity for complex documents, or is “good enough” conversion acceptable for first release?
- Should we use a custom XML parser or evaluate a higher-level Rust library first?

## Recommendation

Start with a small Rust CLI that focuses on accurate parsing of common text structures and clean LaTeX output. That gives a solid base for later expansion without overcomplicating the first milestone.