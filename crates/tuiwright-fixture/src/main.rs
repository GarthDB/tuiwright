//! tuiwright-fixture — a tiny ratatui TUI used as a deterministic render target
//! for testing the tuiwright headless inner loop.
//!
//! Normal interactive mode: renders a coloured, styled UI to the terminal.
//!
//! `--snapshot-ansi [--replay <ndjson>]`: creates a TestBackend (no TTY, no
//! alternate screen), optionally applies events from an NDJSON file, draws one
//! frame, then writes the final frame to stdout as ANSI SGR text.  This is the
//! format the `tui_headless` MCP tool expects from `headless_snapshot`.

use crossterm::event::KeyCode;
use ratatui::{
    backend::TestBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

const ITEMS: &[&str] = &[
    "Design tokens",
    "Color scales",
    "Typography",
    "Spacing system",
    "Component library",
    "Icon set",
    "Motion",
    "Accessibility",
];

struct Model {
    selected: usize,
    status: String,
}

impl Model {
    fn new() -> Self {
        Self {
            selected: 0,
            status: "Use ↑/↓ to select  •  q to quit".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Events (for NDJSON replay)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(tag = "key")]
enum Event {
    Down,
    Up,
    #[serde(other)]
    Other,
}

fn apply_event(model: &mut Model, event: Event) {
    match event {
        Event::Down => {
            model.selected = (model.selected + 1).min(ITEMS.len() - 1);
            model.status = format!("Selected: {}", ITEMS[model.selected]);
        }
        Event::Up => {
            model.selected = model.selected.saturating_sub(1);
            model.status = format!("Selected: {}", ITEMS[model.selected]);
        }
        Event::Other => {}
    }
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

fn draw(frame: &mut ratatui::Frame, model: &Model) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // title bar
            Constraint::Min(1),    // list
            Constraint::Length(1), // status bar
        ])
        .split(area);

    // Title bar — blue background, bold white text.
    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            " ◈ tuiwright fixture ",
            Style::default()
                .fg(Color::White)
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " — headless TUI render target",
            Style::default().fg(Color::LightCyan).bg(Color::Blue),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(title, chunks[0]);

    // Selectable list — highlighted row uses yellow fg + bold.
    let items: Vec<ListItem> = ITEMS
        .iter()
        .enumerate()
        .map(|(i, &name)| {
            let style = if i == model.selected {
                Style::default()
                    .fg(Color::Yellow)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else if i % 2 == 0 {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::Gray)
            };
            ListItem::new(Span::styled(format!("  {name}"), style))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(model.selected));

    let list = List::new(items)
        .block(
            Block::default()
                .title(Span::styled(
                    " Design System Components ",
                    Style::default()
                        .fg(Color::LightGreen)
                        .add_modifier(Modifier::ITALIC),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .highlight_symbol("► ");

    frame.render_stateful_widget(list, chunks[1], &mut list_state);

    // Status bar — dark background, colored text.
    let status = Paragraph::new(Span::styled(
        format!(" {}", model.status),
        Style::default().fg(Color::LightYellow),
    ))
    .style(Style::default().bg(Color::Black));
    frame.render_widget(status, chunks[2]);
}

// ---------------------------------------------------------------------------
// Headless snapshot mode
// ---------------------------------------------------------------------------

/// Convert a ratatui `Color` to its foreground SGR code string.
/// Encoding matches tuiwright-core's `color_fg_code` so that
/// `ansi_to_grid` can decode it correctly.
fn fg_code(c: Color) -> Option<String> {
    match c {
        Color::Reset => None,
        Color::Black => Some("30".into()),
        Color::Red => Some("31".into()),
        Color::Green => Some("32".into()),
        Color::Yellow => Some("33".into()),
        Color::Blue => Some("34".into()),
        Color::Magenta => Some("35".into()),
        Color::Cyan => Some("36".into()),
        Color::Gray => Some("37".into()),
        // Bright colours: fg 90-97 (tuiwright-core: 82+n for n=8..15)
        Color::DarkGray => Some("90".into()),
        Color::LightRed => Some("91".into()),
        Color::LightGreen => Some("92".into()),
        Color::LightYellow => Some("93".into()),
        Color::LightBlue => Some("94".into()),
        Color::LightMagenta => Some("95".into()),
        Color::LightCyan => Some("96".into()),
        Color::White => Some("97".into()),
        Color::Indexed(n) => Some(format!("38;5;{n}")),
        Color::Rgb(r, g, b) => Some(format!("38;2;{r};{g};{b}")),
    }
}

/// Convert a ratatui `Color` to its background SGR code string.
fn bg_code(c: Color) -> Option<String> {
    match c {
        Color::Reset => None,
        Color::Black => Some("40".into()),
        Color::Red => Some("41".into()),
        Color::Green => Some("42".into()),
        Color::Yellow => Some("43".into()),
        Color::Blue => Some("44".into()),
        Color::Magenta => Some("45".into()),
        Color::Cyan => Some("46".into()),
        Color::Gray => Some("47".into()),
        // Bright bg: 100-107 (tuiwright-core: 92+n for n=8..15)
        Color::DarkGray => Some("100".into()),
        Color::LightRed => Some("101".into()),
        Color::LightGreen => Some("102".into()),
        Color::LightYellow => Some("103".into()),
        Color::LightBlue => Some("104".into()),
        Color::LightMagenta => Some("105".into()),
        Color::LightCyan => Some("106".into()),
        Color::White => Some("107".into()),
        Color::Indexed(n) => Some(format!("48;5;{n}")),
        Color::Rgb(r, g, b) => Some(format!("48;2;{r};{g};{b}")),
    }
}

/// Build a full SGR code string for a ratatui cell.
fn cell_sgr(cell: &ratatui::buffer::Cell) -> String {
    let mut codes = vec!["0".to_string()]; // reset first

    let m = cell.modifier;
    if m.contains(Modifier::BOLD) {
        codes.push("1".into());
    }
    if m.contains(Modifier::DIM) {
        codes.push("2".into());
    }
    if m.contains(Modifier::ITALIC) {
        codes.push("3".into());
    }
    if m.contains(Modifier::UNDERLINED) {
        codes.push("4".into());
    }
    if let Some(code) = fg_code(cell.fg) {
        codes.push(code);
    }
    if let Some(code) = bg_code(cell.bg) {
        codes.push(code);
    }

    format!("\x1b[{}m", codes.join(";"))
}

/// Render the terminal buffer to an ANSI SGR string.
/// Format matches tuiwright-core `grid_to_ansi`: reset at row start/end, SGR
/// emitted only on style change.
fn buffer_to_ansi(buf: &ratatui::buffer::Buffer) -> String {
    let area = buf.area;
    let mut out = String::new();

    for y in area.top()..area.bottom() {
        out.push_str("\x1b[0m"); // reset at row start

        // Track previous style codes to emit SGR only on change.
        let mut prev_codes: Option<String> = None;

        for x in area.left()..area.right() {
            let cell = buf.cell((x, y)).expect("cell within bounds");
            let codes = cell_sgr(cell);
            if prev_codes.as_deref() != Some(&codes) {
                out.push_str(&codes);
                prev_codes = Some(codes);
            }
            out.push_str(cell.symbol());
        }

        out.push_str("\x1b[0m\n"); // reset + newline
    }

    out
}

/// Run headless: apply optional NDJSON replay, then dump the final frame as ANSI.
fn run_snapshot(replay_path: Option<&str>) -> anyhow::Result<()> {
    let cols: u16 = std::env::var("COLUMNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(80);
    let rows: u16 = std::env::var("LINES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(24);

    let backend = TestBackend::new(cols, rows);
    let mut terminal = Terminal::new(backend)?;
    let mut model = Model::new();

    // Apply replay events if provided.
    if let Some(path) = replay_path {
        let content = std::fs::read_to_string(path)?;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(event) = serde_json::from_str::<Event>(line) {
                apply_event(&mut model, event);
            }
        }
    }

    // Draw final frame.
    terminal.draw(|frame| draw(frame, &model))?;

    // Emit ANSI to stdout.
    let ansi = buffer_to_ansi(terminal.backend().buffer());
    print!("{ansi}");

    Ok(())
}

// ---------------------------------------------------------------------------
// Interactive mode
// ---------------------------------------------------------------------------

fn run_interactive() -> anyhow::Result<()> {
    use crossterm::{
        event::{self, Event as CrosstermEvent},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use ratatui::backend::CrosstermBackend;
    use std::io;

    enable_raw_mode()?;
    execute!(io::stderr(), EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stderr());
    let mut terminal = Terminal::new(backend)?;
    let mut model = Model::new();

    loop {
        terminal.draw(|frame| draw(frame, &model))?;

        if let CrosstermEvent::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Down | KeyCode::Char('j') => {
                    apply_event(&mut model, Event::Down);
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    apply_event(&mut model, Event::Up);
                }
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(io::stderr(), LeaveAlternateScreen)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.contains(&"--snapshot-ansi".to_string()) {
        // Headless mode.
        let replay = args
            .windows(2)
            .find(|w| w[0] == "--replay")
            .map(|w| w[1].as_str());
        run_snapshot(replay)
    } else {
        run_interactive()
    }
}
