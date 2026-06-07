//! Encode and decode ANSI SGR-escaped text to/from [`SnapshotGrid`].
//!
//! `grid_to_ansi` encodes a grid → ANSI for piping to `freeze` (→ PNG).
//! `ansi_to_grid` decodes ANSI SGR output → a styled grid for inspection.

use crate::snapshot::{Cell, CellStyle, Color, CursorState, SnapshotGrid};

/// Render a [`SnapshotGrid`] as a string containing ANSI SGR escape sequences.
///
/// The output is designed to be fed to `freeze --output <file.png>` via stdin,
/// or written to a `.ans` file for later processing.
pub fn grid_to_ansi(grid: &SnapshotGrid) -> String {
    let cols = grid.cols as usize;
    let mut out = String::new();

    for row in grid.cells.chunks(cols) {
        // Reset at the start of each row.
        out.push_str("\x1b[0m");

        let mut prev_style: Option<&CellStyle> = None;
        for cell in row {
            // Only emit SGR codes if style changed.
            if prev_style
                .map(|p| styles_differ(p, &cell.style))
                .unwrap_or(true)
            {
                out.push_str(&style_to_sgr(&cell.style));
            }
            out.push_str(&cell.symbol);
            prev_style = Some(&cell.style);
        }
        // Reset at end of row, then newline.
        out.push_str("\x1b[0m\n");
    }

    // If the grid carries cursor state, append a CUP escape (1-based row;col).
    // The ansi_to_grid decoder recognises this and restores the cursor field.
    if let Some(c) = grid.cursor {
        out.push_str(&format!("\x1b[{};{}H", c.row + 1, c.col + 1));
    }

    out
}

fn styles_differ(a: &CellStyle, b: &CellStyle) -> bool {
    a.bold != b.bold
        || a.italic != b.italic
        || a.underline != b.underline
        || a.dim != b.dim
        || !colors_eq(&a.fg, &b.fg)
        || !colors_eq(&a.bg, &b.bg)
}

fn colors_eq(a: &Option<Color>, b: &Option<Color>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(ca), Some(cb)) => color_code(ca) == color_code(cb),
        _ => false,
    }
}

fn style_to_sgr(s: &CellStyle) -> String {
    let mut codes: Vec<String> = vec!["0".to_string()]; // always reset first

    if s.bold {
        codes.push("1".to_string());
    }
    if s.dim {
        codes.push("2".to_string());
    }
    if s.italic {
        codes.push("3".to_string());
    }
    if s.underline {
        codes.push("4".to_string());
    }
    if let Some(fg) = &s.fg {
        codes.push(color_fg_code(fg));
    }
    if let Some(bg) = &s.bg {
        codes.push(color_bg_code(bg));
    }

    format!("\x1b[{}m", codes.join(";"))
}

fn color_code(c: &Color) -> String {
    match c {
        Color::Ansi(n) => n.to_string(),
        Color::Indexed(n) => format!("5;{n}"),
        Color::Rgb(r, g, b) => format!("2;{r};{g};{b}"),
    }
}

fn color_fg_code(c: &Color) -> String {
    match c {
        Color::Ansi(n) if *n < 8 => format!("{}", 30 + n),
        Color::Ansi(n) => format!("{}", 82 + n), // bright: 90-97
        Color::Indexed(n) => format!("38;5;{n}"),
        Color::Rgb(r, g, b) => format!("38;2;{r};{g};{b}"),
    }
}

fn color_bg_code(c: &Color) -> String {
    match c {
        Color::Ansi(n) if *n < 8 => format!("{}", 40 + n),
        Color::Ansi(n) => format!("{}", 92 + n), // bright: 100-107
        Color::Indexed(n) => format!("48;5;{n}"),
        Color::Rgb(r, g, b) => format!("48;2;{r};{g};{b}"),
    }
}

/// Parse an ANSI SGR-escaped string into a [`SnapshotGrid`].
///
/// The input is expected to be in the format produced by [`grid_to_ansi`]:
/// each row ends with `\n`, cells carry SGR codes that encode style.  CSI
/// sequences are handled as follows:
/// - `m` (SGR): applied to the current style.
/// - `H` / `f` (CUP — cursor position): parsed and stored in `grid.cursor`.
///   ANSI uses **1-based** row;col; missing params default to 1. The resulting
///   `CursorState` has `visible = true`.
/// - All other CSI sequences: silently skipped.
///
/// `cur_row`/`cur_col` below track where the next *cell character* is placed
/// and are independent of the cursor position captured from CUP.
pub fn ansi_to_grid(input: &str, cols: u16, rows: u16) -> SnapshotGrid {
    let col_usize = cols as usize;
    let row_usize = rows as usize;
    let mut cells = vec![Cell::default(); col_usize * row_usize];
    let mut cur_style = CellStyle::default();
    let mut cur_row = 0usize;
    let mut cur_col = 0usize;
    let mut cursor: Option<CursorState> = None;

    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'\x1b' && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            // CSI sequence: ESC [ <params> <final_byte>
            i += 2; // skip ESC [
            let param_start = i;
            // Collect intermediate/parameter bytes (0x20–0x3F) until final byte (0x40–0x7E).
            while i < bytes.len() && !(0x40..=0x7E).contains(&bytes[i]) {
                i += 1;
            }
            let final_byte = bytes.get(i).copied().unwrap_or(b'm');
            let params_str = std::str::from_utf8(&bytes[param_start..i]).unwrap_or("");
            match final_byte {
                b'm' => apply_sgr(params_str, &mut cur_style),
                // CUP (cursor position) — H and f are synonyms.
                // Params are "row;col" 1-based; missing parts default to 1.
                b'H' | b'f' => {
                    cursor = Some(parse_cup(params_str));
                }
                _ => {}
            }
            // Skip the final byte.
            if i < bytes.len() {
                i += 1;
            }
        } else if bytes[i] == b'\n' {
            // End of row — advance to next row.
            if cur_row >= row_usize {
                break;
            }
            cur_row += 1;
            cur_col = 0;
            cur_style = CellStyle::default(); // encoder resets at row start/end
            i += 1;
        } else if bytes[i] == b'\r' {
            i += 1;
        } else {
            if cur_row >= row_usize {
                break;
            }
            // UTF-8 character: consume all continuation bytes.
            let ch_start = i;
            i += 1;
            while i < bytes.len() && bytes[i] & 0xC0 == 0x80 {
                i += 1;
            }
            if cur_col < col_usize && cur_row < row_usize {
                let sym = std::str::from_utf8(&bytes[ch_start..i]).unwrap_or(" ");
                cells[cur_row * col_usize + cur_col] = Cell {
                    symbol: sym.to_string(),
                    style: cur_style.clone(),
                };
                cur_col += 1;
            }
        }
    }

    SnapshotGrid {
        cols,
        rows,
        cells,
        cursor,
    }
}

/// Parse a CUP parameter string `"row;col"` (both 1-based, both optional, default 1)
/// into a zero-based [`CursorState`].
fn parse_cup(params: &str) -> CursorState {
    let mut parts = params.splitn(2, ';');
    let row1: u16 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    let col1: u16 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    CursorState {
        row: row1.saturating_sub(1),
        col: col1.saturating_sub(1),
        visible: true,
    }
}

/// Apply a semicolon-delimited list of SGR parameters to `style`.
fn apply_sgr(params: &str, style: &mut CellStyle) {
    if params.is_empty() {
        // ESC[m == ESC[0m
        *style = CellStyle::default();
        return;
    }
    let nums: Vec<u16> = params.split(';').filter_map(|s| s.parse().ok()).collect();
    let mut j = 0usize;
    while j < nums.len() {
        match nums[j] {
            0 => *style = CellStyle::default(),
            1 => style.bold = true,
            2 => style.dim = true,
            3 => style.italic = true,
            4 => style.underline = true,
            // Standard fg colors (30–37 → Ansi 0–7).
            30..=37 => style.fg = Some(Color::Ansi((nums[j] - 30) as u8)),
            // Bright fg (90–97 → Ansi 8–15; encoder uses 82+n for n≥8).
            90..=97 => style.fg = Some(Color::Ansi((nums[j] - 82) as u8)),
            // Extended fg (38;5;n or 38;2;r;g;b).
            38 => match nums.get(j + 1).copied() {
                Some(5) => {
                    if let Some(&n) = nums.get(j + 2) {
                        style.fg = Some(Color::Indexed(n as u8));
                        j += 2;
                    }
                }
                Some(2) => {
                    if let (Some(&r), Some(&g), Some(&b)) =
                        (nums.get(j + 2), nums.get(j + 3), nums.get(j + 4))
                    {
                        style.fg = Some(Color::Rgb(r as u8, g as u8, b as u8));
                        j += 4;
                    }
                }
                _ => {}
            },
            // Standard bg colors (40–47 → Ansi 0–7).
            40..=47 => style.bg = Some(Color::Ansi((nums[j] - 40) as u8)),
            // Bright bg (100–107 → Ansi 8–15; encoder uses 92+n for n≥8).
            100..=107 => style.bg = Some(Color::Ansi((nums[j] - 92) as u8)),
            // Extended bg (48;5;n or 48;2;r;g;b).
            48 => match nums.get(j + 1).copied() {
                Some(5) => {
                    if let Some(&n) = nums.get(j + 2) {
                        style.bg = Some(Color::Indexed(n as u8));
                        j += 2;
                    }
                }
                Some(2) => {
                    if let (Some(&r), Some(&g), Some(&b)) =
                        (nums.get(j + 2), nums.get(j + 3), nums.get(j + 4))
                    {
                        style.bg = Some(Color::Rgb(r as u8, g as u8, b as u8));
                        j += 4;
                    }
                }
                _ => {}
            },
            _ => {}
        }
        j += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::Cell;

    #[test]
    fn plain_ascii_roundtrip() {
        let cells: Vec<Cell> = "hello"
            .chars()
            .map(|c| Cell {
                symbol: c.to_string(),
                style: Default::default(),
            })
            .collect();
        let grid = SnapshotGrid::new(5, 1, cells);
        let ansi = grid_to_ansi(&grid);
        // Strip all escape sequences, should get "hello".
        let plain: String = ansi
            .chars()
            .fold((String::new(), false), |(mut acc, in_esc), ch| {
                if ch == '\x1b' {
                    (acc, true)
                } else if in_esc && ch == 'm' {
                    (acc, false)
                } else if !in_esc && ch != '\n' {
                    acc.push(ch);
                    (acc, false)
                } else {
                    (acc, in_esc)
                }
            })
            .0;
        assert_eq!(plain, "hello");
    }

    /// All 8 bright ANSI fg colors (Ansi 8–15) must round-trip through grid_to_ansi → ansi_to_grid.
    #[test]
    fn bright_fg_roundtrip() {
        for n in 8u8..=15 {
            let style = CellStyle {
                fg: Some(Color::Ansi(n)),
                ..Default::default()
            };
            let grid = SnapshotGrid::new(
                1,
                1,
                vec![Cell {
                    symbol: "X".to_string(),
                    style,
                }],
            );
            let decoded = ansi_to_grid(&grid_to_ansi(&grid), 1, 1);
            assert_eq!(
                decoded.cells[0].style.fg,
                Some(Color::Ansi(n)),
                "bright fg Ansi({n}) did not round-trip"
            );
        }
    }

    /// All 8 bright ANSI bg colors (Ansi 8–15) must round-trip.
    #[test]
    fn bright_bg_roundtrip() {
        for n in 8u8..=15 {
            let style = CellStyle {
                bg: Some(Color::Ansi(n)),
                ..Default::default()
            };
            let grid = SnapshotGrid::new(
                1,
                1,
                vec![Cell {
                    symbol: "X".to_string(),
                    style,
                }],
            );
            let decoded = ansi_to_grid(&grid_to_ansi(&grid), 1, 1);
            assert_eq!(
                decoded.cells[0].style.bg,
                Some(Color::Ansi(n)),
                "bright bg Ansi({n}) did not round-trip"
            );
        }
    }

    /// Multi-byte UTF-8 graphemes must land in a single cell, not scatter across
    /// continuation bytes.
    ///
    /// Note: wide characters (😀 occupies 2 terminal columns) are stored as-is.
    /// The decoder treats each grapheme cluster as one cell entry regardless of
    /// display width; callers that need to account for wide-char column offsets
    /// must do so at a higher level.
    #[test]
    fn multi_byte_grapheme_single_cell() {
        let emoji = "😀"; // U+1F600 — 4 bytes, wide (2 terminal cols)
        let cjk = "中"; // U+4E2D — 3 bytes, wide (2 terminal cols)
        let cells = vec![
            Cell {
                symbol: emoji.to_string(),
                style: Default::default(),
            },
            Cell {
                symbol: cjk.to_string(),
                style: Default::default(),
            },
        ];
        let grid = SnapshotGrid::new(2, 1, cells);
        let decoded = ansi_to_grid(&grid_to_ansi(&grid), 2, 1);
        assert_eq!(decoded.cells[0].symbol, emoji);
        assert_eq!(decoded.cells[1].symbol, cjk);
    }

    /// SGR params not handled by this decoder must not corrupt state or panic.
    #[test]
    fn unknown_sgr_codes_ignored() {
        // 22 = bold-off, 39/49 = default fg/bg (both are valid ANSI codes but
        // not yet implemented — the decoder ignores unrecognised params).
        // 999 = reserved/undefined. None should panic; the symbol must be placed.
        let input = "\x1b[1;22;39;49;999mA\x1b[0m\n";
        let decoded = ansi_to_grid(input, 1, 1);
        assert_eq!(decoded.cells[0].symbol, "A");
    }

    /// A CSI sequence truncated at end-of-input must not panic.
    #[test]
    fn partial_csi_at_eof_no_panic() {
        let input = "A\x1b["; // ESC [ with no params or final byte
        let decoded = ansi_to_grid(input, 2, 1);
        assert_eq!(decoded.cells[0].symbol, "A");
    }

    /// CSI H (CUP — cursor home) is now captured, not skipped.
    /// Cell content before/after the CUP still lands at the sequential grid positions.
    #[test]
    fn cursor_home_csi_captured() {
        // ESC[H with no params means row=1,col=1 (ANSI 1-based) → (row=0,col=0) zero-based.
        let input = "A\x1b[HB\n";
        let decoded = ansi_to_grid(input, 2, 1);
        assert_eq!(decoded.cells[0].symbol, "A");
        assert_eq!(decoded.cells[1].symbol, "B");
        let cursor = decoded.cursor.expect("CUP should set cursor");
        assert_eq!(cursor.row, 0);
        assert_eq!(cursor.col, 0);
    }

    /// CSI 2C (cursor forward) is still silently skipped; next char placed at the current column.
    #[test]
    fn cursor_forward_csi_skipped() {
        let input = "A\x1b[2CB\n";
        let decoded = ansi_to_grid(input, 2, 1);
        assert_eq!(decoded.cells[0].symbol, "A");
        assert_eq!(decoded.cells[1].symbol, "B");
    }

    /// CUP at an arbitrary position round-trips through grid_to_ansi → ansi_to_grid.
    #[test]
    fn cursor_cup_roundtrip() {
        use crate::snapshot::CursorState;
        let cells = vec![Cell::default(); 4];
        let mut grid = SnapshotGrid::new(2, 2, cells);
        grid.cursor = Some(CursorState {
            row: 1,
            col: 1,
            visible: true,
        });
        let ansi = grid_to_ansi(&grid);
        // The CUP escape should be present (1-based → "2;2").
        assert!(
            ansi.contains("\x1b[2;2H"),
            "expected CUP in output: {:?}",
            ansi
        );
        let decoded = ansi_to_grid(&ansi, 2, 2);
        let c = decoded.cursor.expect("cursor should be decoded");
        assert_eq!(c.row, 1);
        assert_eq!(c.col, 1);
        assert!(c.visible);
    }

    /// CUP with missing params defaults to (0, 0) (ANSI 1-based 1,1 → 0,0 zero-based).
    #[test]
    fn cursor_cup_defaults_to_home() {
        let input = "\x1b[H";
        let decoded = ansi_to_grid(input, 1, 1);
        let c = decoded.cursor.expect("ESC[H should produce a cursor");
        assert_eq!(c.row, 0);
        assert_eq!(c.col, 0);
    }

    /// CUP with only-row param leaves col at 0.
    #[test]
    fn cursor_cup_row_only() {
        let input = "\x1b[3H";
        let decoded = ansi_to_grid(input, 1, 4);
        let c = decoded.cursor.expect("ESC[3H should produce a cursor");
        assert_eq!(c.row, 2); // 3 → 0-based 2
        assert_eq!(c.col, 0); // missing col defaults to 1 → 0
    }

    /// No CUP in input → cursor field is None.
    #[test]
    fn no_cup_means_no_cursor() {
        let input = "AB\n";
        let decoded = ansi_to_grid(input, 2, 1);
        assert!(decoded.cursor.is_none());
    }

    /// Round-trip: grid_to_ansi → ansi_to_grid must reproduce symbols and styles.
    #[test]
    fn styled_roundtrip() {
        let styles = [
            CellStyle {
                fg: Some(Color::Ansi(1)), // red
                bg: None,
                bold: true,
                italic: false,
                underline: false,
                dim: false,
            },
            CellStyle {
                fg: Some(Color::Ansi(10)), // bright green
                bg: Some(Color::Ansi(4)),  // blue bg
                bold: false,
                italic: true,
                underline: false,
                dim: false,
            },
            CellStyle {
                fg: Some(Color::Indexed(220)),
                bg: Some(Color::Indexed(18)),
                bold: false,
                italic: false,
                underline: true,
                dim: false,
            },
            CellStyle {
                fg: Some(Color::Rgb(255, 128, 0)),
                bg: Some(Color::Rgb(0, 32, 64)),
                bold: true,
                italic: false,
                underline: false,
                dim: true,
            },
        ];
        let symbols = ["A", "B", "C", "D"];
        let cells: Vec<Cell> = symbols
            .iter()
            .zip(styles.iter())
            .map(|(sym, st)| Cell {
                symbol: sym.to_string(),
                style: st.clone(),
            })
            .collect();

        let grid = SnapshotGrid::new(4, 1, cells.clone());

        let ansi = grid_to_ansi(&grid);
        let decoded = ansi_to_grid(&ansi, 4, 1);

        assert_eq!(decoded.cols, 4);
        assert_eq!(decoded.rows, 1);
        assert_eq!(decoded.cells.len(), 4);

        for (i, (orig, got)) in cells.iter().zip(decoded.cells.iter()).enumerate() {
            assert_eq!(orig.symbol, got.symbol, "symbol mismatch at cell {i}");
            assert_eq!(orig.style.bold, got.style.bold, "bold mismatch at cell {i}");
            assert_eq!(
                orig.style.italic, got.style.italic,
                "italic mismatch at cell {i}"
            );
            assert_eq!(
                orig.style.underline, got.style.underline,
                "underline mismatch at cell {i}"
            );
            assert_eq!(orig.style.dim, got.style.dim, "dim mismatch at cell {i}");
            // Color comparison via the code representation.
            assert_eq!(
                orig.style.fg.as_ref().map(color_code),
                got.style.fg.as_ref().map(color_code),
                "fg mismatch at cell {i}"
            );
            assert_eq!(
                orig.style.bg.as_ref().map(color_code),
                got.style.bg.as_ref().map(color_code),
                "bg mismatch at cell {i}"
            );
        }
    }
}
