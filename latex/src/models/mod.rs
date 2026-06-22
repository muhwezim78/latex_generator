//! Core domain model for a parsed Word document.
//! Every other module depends on these types; this module has no I/O of its own.

// ── Document root ─────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct Document {
    /// Optional document title extracted from metadata or the first Heading 1.
    pub title: Option<String>,
    /// Top-level content blocks in document order.
    pub elements: Vec<Element>,
}

// ── Block-level elements ──────────────────────────────────────────────────────

#[derive(Debug)]
pub enum Element {
    /// A heading (level 1–4 maps to \section … \subsubsubsection).
    Heading { level: u8, runs: Vec<Run> },
    /// A body paragraph.
    Paragraph { runs: Vec<Run> },
    /// A bulleted or numbered list.
    List {
        ordered: bool,
        level: u8,
        items: Vec<Vec<Run>>,
    },
    /// A table.
    Table { rows: Vec<Vec<Cell>> },
    /// An image already copied to <output>/media/.
    Image {
        path: String,
        width_cm: Option<f64>,
        caption: Option<String>,
    },
    /// An explicit page break.
    PageBreak,
}

// ── Inline runs ───────────────────────────────────────────────────────────────

#[derive(Debug, Default, Clone)]
pub struct Run {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub superscript: bool,
    pub subscript: bool,
    /// Monospace / code style.
    pub mono: bool,
    /// Hyperlink target URL if this run is a link.
    pub hyperlink: Option<String>,
}

// ── Table cell ────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct Cell {
    pub runs: Vec<Run>,
}

// ── Error type ────────────────────────────────────────────────────────────────

#[derive(thiserror::Error, Debug)]
pub enum DocError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("XML parse error: {0}")]
    Xml(String),

    #[error("Required entry missing from archive: {0}")]
    Missing(String),
}
