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

#[derive(Debug, Clone)]
pub enum Element {
    /// A heading (level 1–4 maps to \section … \subsubsubsection).
    Heading { level: u8, runs: Vec<Run> },
    /// A body paragraph.
    Paragraph { runs: Vec<Run> },
    /// A bulleted or numbered list.
    /// Each item carries its own indent level so nested lists can be rendered.
    List {
        ordered: bool,
        items: Vec<(u8, Vec<Run>)>,
    },
    /// A table.
    Table { rows: Vec<Vec<Cell>>, caption: Option<String> },
    /// An image already copied to <output>/media/.
    Image {
        path: String,
        width_cm: Option<f64>,
        caption: Option<String>,
    },
    /// An explicit page break.
    PageBreak,
    /// An auto-generated (or detected) table of contents.
    TocBlock,
    /// An auto-generated list of figures (replaces a manual dot-leader LOF).
    LofBlock,
    /// An auto-generated list of tables (replaces a manual dot-leader LOT).
    LotBlock,
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

#[derive(Debug, Default, Clone)]
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
