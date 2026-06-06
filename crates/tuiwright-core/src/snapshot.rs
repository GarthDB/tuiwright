//! A terminal pane snapshot — a 2-D grid of styled cells.
//!
//! This is the canonical representation inside tuiwright.  Both the headless
//! and live paths produce a `SnapshotGrid`; downstream code renders it to
//! plain text or ANSI SGR (→ freeze → PNG).

use serde::{Deserialize, Serialize};

/// A full snapshot of a terminal pane at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotGrid {
    pub cols: u16,
    pub rows: u16,
    /// Row-major, length == cols * rows.
    pub cells: Vec<Cell>,
}

impl SnapshotGrid {
    /// Render the grid as plain text (no ANSI codes), one row per line, with
    /// trailing whitespace stripped from each row.
    pub fn to_plain_text(&self) -> String {
        let cols = self.cols as usize;
        let mut out = String::new();
        for row in self.cells.chunks(cols) {
            let s: String = row.iter().map(|c| c.symbol.as_str()).collect();
            out.push_str(s.trim_end());
            out.push('\n');
        }
        out
    }
}

/// A single terminal cell.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cell {
    /// The rendered grapheme (usually one char, but can be multi-byte).
    pub symbol: String,
    pub style: CellStyle,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            symbol: " ".to_string(),
            style: CellStyle::default(),
        }
    }
}

/// Style attributes for a single cell.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CellStyle {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub dim: bool,
}

/// Terminal color — supports the three common encoding forms.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum Color {
    /// ANSI named color (index 0–15).
    Ansi(u8),
    /// xterm 256-color index.
    Indexed(u8),
    /// 24-bit RGB.
    Rgb(u8, u8, u8),
}
