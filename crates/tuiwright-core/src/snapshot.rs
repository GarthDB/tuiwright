//! A terminal pane snapshot — a 2-D grid of styled cells.
//!
//! This is the canonical representation inside tuiwright.  Both the headless
//! and live paths produce a `SnapshotGrid`; downstream code renders it to
//! plain text or ANSI SGR (→ freeze → PNG).

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Cursor position and visibility captured from a terminal pane.
///
/// `row` and `col` are zero-based. `visible` reflects whether the cursor is
/// currently shown (hidden via `\x1b[?25l`, etc.). `style` is the raw cursor
/// shape integer from the terminal (0 = default block).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CursorState {
    pub row: u16,
    pub col: u16,
    pub visible: bool,
}

impl Default for CursorState {
    fn default() -> Self {
        Self { row: 0, col: 0, visible: true }
    }
}

/// A full snapshot of a terminal pane at a point in time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotGrid {
    pub cols: u16,
    pub rows: u16,
    /// Row-major, length == cols * rows.
    pub cells: Vec<Cell>,
    /// Terminal cursor position and visibility, if captured.
    ///
    /// `None` is the default — existing baselines (`.snap.json`) that predate
    /// cursor support will deserialize to `None` and **never fail a diff**,
    /// preserving back-compat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<CursorState>,
}

impl SnapshotGrid {
    /// Construct a grid with no cursor set (cursor = None).
    pub fn new(cols: u16, rows: u16, cells: Vec<Cell>) -> Self {
        Self { cols, rows, cells, cursor: None }
    }

    /// Save this grid as a JSON baseline file.
    pub fn save_baseline(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load a previously saved baseline from a JSON file.
    pub fn load_baseline(path: &Path) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&raw)?)
    }

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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CellStyle {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub dim: bool,
}

/// Terminal color — supports the three common encoding forms.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum Color {
    /// ANSI named color (index 0–15).
    Ansi(u8),
    /// xterm 256-color index.
    Indexed(u8),
    /// 24-bit RGB.
    Rgb(u8, u8, u8),
}
