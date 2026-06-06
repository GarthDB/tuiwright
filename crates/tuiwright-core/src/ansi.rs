//! Convert a [`SnapshotGrid`] to ANSI SGR-escaped text suitable for piping to
//! `freeze` to produce a PNG screenshot.

use crate::snapshot::{CellStyle, Color, SnapshotGrid};

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
            if prev_style.map(|p| styles_differ(p, &cell.style)).unwrap_or(true) {
                out.push_str(&style_to_sgr(&cell.style));
            }
            out.push_str(&cell.symbol);
            prev_style = Some(&cell.style);
        }
        // Reset at end of row, then newline.
        out.push_str("\x1b[0m\n");
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
        let grid = SnapshotGrid {
            cols: 5,
            rows: 1,
            cells,
        };
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
}
