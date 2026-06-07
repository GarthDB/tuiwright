//! Cell-level diff between two [`SnapshotGrid`]s.
//!
//! [`diff`] compares grids of equal or different dimensions and returns a
//! [`GridDiff`] describing which cells changed.  [`GridDiff::display`]
//! renders a human-readable summary suitable for MCP tool output.

use crate::snapshot::{Cell, CellStyle, Color, CursorState, SnapshotGrid};
use serde::{Deserialize, Serialize};

/// A single cell that differs between baseline and actual.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellDiff {
    pub row: u16,
    pub col: u16,
    pub expected: Cell,
    pub actual: Cell,
}

/// The result of comparing two [`SnapshotGrid`]s.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridDiff {
    pub expected_size: (u16, u16),
    pub actual_size: (u16, u16),
    /// True when the grids have different dimensions (implies all cells differ).
    pub size_mismatch: bool,
    pub changed_cells: Vec<CellDiff>,
    /// Set when the expected grid had a cursor and it differed from actual.
    ///
    /// `None` means either no cursor was expected (back-compat) or they matched.
    /// When present, holds `(expected_cursor, actual_cursor)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor_mismatch: Option<(Option<CursorState>, Option<CursorState>)>,
}

impl GridDiff {
    /// Returns true when there are no differences (cells, size, or cursor).
    pub fn is_match(&self) -> bool {
        !self.size_mismatch && self.changed_cells.is_empty() && self.cursor_mismatch.is_none()
    }

    /// Human-readable summary for MCP tool output.
    pub fn display(&self) -> String {
        if self.is_match() {
            return "match — grids are identical".to_string();
        }

        let mut out = String::new();

        if self.size_mismatch {
            out.push_str(&format!(
                "size mismatch: expected {}x{}, got {}x{}\n",
                self.expected_size.0, self.expected_size.1, self.actual_size.0, self.actual_size.1,
            ));
        }

        let total = self.changed_cells.len();
        if total > 0 {
            out.push_str(&format!("{total} cell(s) differ:\n"));
        }

        // Show up to 20 changed cells in detail.
        for cd in self.changed_cells.iter().take(20) {
            let exp_sym = escape_sym(&cd.expected.symbol);
            let act_sym = escape_sym(&cd.actual.symbol);
            if exp_sym != act_sym {
                out.push_str(&format!(
                    "  [{},{}] symbol: {:?} → {:?}",
                    cd.row, cd.col, exp_sym, act_sym
                ));
            } else {
                out.push_str(&format!("  [{},{}] {:?}", cd.row, cd.col, act_sym));
            }
            let style_note = style_diff_note(&cd.expected.style, &cd.actual.style);
            if !style_note.is_empty() {
                out.push_str(&format!(" ({})", style_note));
            }
            out.push('\n');
        }
        if total > 20 {
            out.push_str(&format!("  … and {} more\n", total - 20));
        }

        if let Some((exp_c, act_c)) = &self.cursor_mismatch {
            let fmt = |c: &Option<CursorState>| match c {
                Some(cs) => format!("row={} col={} visible={}", cs.row, cs.col, cs.visible),
                None => "<none>".to_string(),
            };
            out.push_str(&format!(
                "cursor mismatch: expected {} → actual {}\n",
                fmt(exp_c),
                fmt(act_c)
            ));
        }

        out
    }
}

/// Compare `expected` (baseline) against `actual`.
///
/// Cursor comparison is opt-in: if `expected.cursor` is `None` (the case for
/// all old baselines), cursors are never compared so back-compat is preserved.
pub fn diff(expected: &SnapshotGrid, actual: &SnapshotGrid) -> GridDiff {
    let size_mismatch = expected.cols != actual.cols || expected.rows != actual.rows;

    if size_mismatch {
        return GridDiff {
            expected_size: (expected.cols, expected.rows),
            actual_size: (actual.cols, actual.rows),
            size_mismatch: true,
            changed_cells: vec![],
            cursor_mismatch: None,
        };
    }

    let cols = expected.cols as usize;
    let rows = expected.rows as usize;
    let mut changed_cells = Vec::new();

    for r in 0..rows {
        for c in 0..cols {
            let idx = r * cols + c;
            let exp = &expected.cells[idx];
            let act = &actual.cells[idx];
            if !cells_equal(exp, act) {
                changed_cells.push(CellDiff {
                    row: r as u16,
                    col: c as u16,
                    expected: exp.clone(),
                    actual: act.clone(),
                });
            }
        }
    }

    // Only compare cursors when the expected baseline has one set.
    let cursor_mismatch = if let Some(exp_c) = expected.cursor {
        if actual.cursor != Some(exp_c) {
            Some((Some(exp_c), actual.cursor))
        } else {
            None
        }
    } else {
        None
    };

    GridDiff {
        expected_size: (expected.cols, expected.rows),
        actual_size: (actual.cols, actual.rows),
        size_mismatch: false,
        changed_cells,
        cursor_mismatch,
    }
}

fn cells_equal(a: &Cell, b: &Cell) -> bool {
    a.symbol == b.symbol && styles_equal(&a.style, &b.style)
}

fn styles_equal(a: &CellStyle, b: &CellStyle) -> bool {
    a.bold == b.bold
        && a.dim == b.dim
        && a.italic == b.italic
        && a.underline == b.underline
        && colors_equal(a.fg.as_ref(), b.fg.as_ref())
        && colors_equal(a.bg.as_ref(), b.bg.as_ref())
}

fn colors_equal(a: Option<&Color>, b: Option<&Color>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(Color::Ansi(x)), Some(Color::Ansi(y))) => x == y,
        (Some(Color::Indexed(x)), Some(Color::Indexed(y))) => x == y,
        (Some(Color::Rgb(r1, g1, b1)), Some(Color::Rgb(r2, g2, b2))) => {
            r1 == r2 && g1 == g2 && b1 == b2
        }
        _ => false,
    }
}

fn escape_sym(s: &str) -> String {
    if s == " " {
        "·".to_string()
    } else {
        s.to_string()
    }
}

fn style_diff_note(exp: &CellStyle, act: &CellStyle) -> String {
    let mut parts = Vec::new();
    if exp.bold != act.bold {
        parts.push(if act.bold { "+bold" } else { "-bold" });
    }
    if exp.italic != act.italic {
        parts.push(if act.italic { "+italic" } else { "-italic" });
    }
    if exp.underline != act.underline {
        parts.push(if act.underline {
            "+underline"
        } else {
            "-underline"
        });
    }
    if exp.dim != act.dim {
        parts.push(if act.dim { "+dim" } else { "-dim" });
    }
    if !colors_equal(exp.fg.as_ref(), act.fg.as_ref()) {
        parts.push("fg changed");
    }
    if !colors_equal(exp.bg.as_ref(), act.bg.as_ref()) {
        parts.push("bg changed");
    }
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{Cell, CellStyle};

    fn grid(symbols: &[&str], cols: u16, rows: u16) -> SnapshotGrid {
        let cells = symbols
            .iter()
            .map(|s| Cell {
                symbol: s.to_string(),
                style: CellStyle::default(),
            })
            .collect();
        SnapshotGrid::new(cols, rows, cells)
    }

    #[test]
    fn identical_grids_match() {
        let g = grid(&["a", "b", "c", "d"], 2, 2);
        let d = diff(&g, &g);
        assert!(d.is_match());
        assert_eq!(d.display(), "match — grids are identical");
    }

    #[test]
    fn symbol_change_detected() {
        let exp = grid(&["a", "b", "c", "d"], 2, 2);
        let mut act = exp.clone();
        act.cells[2].symbol = "X".to_string();
        let d = diff(&exp, &act);
        assert!(!d.is_match());
        assert_eq!(d.changed_cells.len(), 1);
        assert_eq!(d.changed_cells[0].row, 1);
        assert_eq!(d.changed_cells[0].col, 0);
    }

    #[test]
    fn size_mismatch_reported() {
        let exp = grid(&["a", "b"], 2, 1);
        let act = grid(&["a", "b", "c"], 3, 1);
        let d = diff(&exp, &act);
        assert!(d.size_mismatch);
        assert!(!d.is_match());
    }

    /// When expected has no cursor (back-compat), cursor differences are ignored.
    #[test]
    fn no_expected_cursor_is_backcompat() {
        let exp = grid(&["a", "b"], 2, 1); // cursor == None
        let mut act = exp.clone();
        act.cursor = Some(CursorState { row: 0, col: 1, visible: true });
        let d = diff(&exp, &act);
        assert!(d.is_match(), "should match when expected has no cursor");
        assert!(d.cursor_mismatch.is_none());
    }

    /// When expected has a cursor and actual matches, no mismatch is reported.
    #[test]
    fn matching_cursors_no_mismatch() {
        let mut exp = grid(&["a", "b"], 2, 1);
        exp.cursor = Some(CursorState { row: 0, col: 1, visible: true });
        let act = exp.clone();
        let d = diff(&exp, &act);
        assert!(d.is_match());
        assert!(d.cursor_mismatch.is_none());
    }

    /// When expected has a cursor and actual has a different one, mismatch is reported.
    #[test]
    fn cursor_mismatch_detected() {
        let mut exp = grid(&["a", "b"], 2, 1);
        exp.cursor = Some(CursorState { row: 0, col: 0, visible: true });
        let mut act = exp.clone();
        act.cursor = Some(CursorState { row: 0, col: 1, visible: true });
        let d = diff(&exp, &act);
        assert!(!d.is_match());
        assert!(d.cursor_mismatch.is_some());
        let display = d.display();
        assert!(display.contains("cursor mismatch"), "display: {display}");
    }

    /// When expected has a cursor and actual has none, mismatch is reported.
    #[test]
    fn cursor_expected_but_absent_in_actual() {
        let mut exp = grid(&["a", "b"], 2, 1);
        exp.cursor = Some(CursorState { row: 0, col: 0, visible: true });
        let act = grid(&["a", "b"], 2, 1); // no cursor
        let d = diff(&exp, &act);
        assert!(!d.is_match());
        assert!(d.cursor_mismatch.is_some());
    }
}
