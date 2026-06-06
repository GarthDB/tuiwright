//! Integration test: run `tuiwright-fixture --snapshot-ansi`, decode the ANSI
//! output through `ansi_to_grid`, and assert that symbols and styles survive
//! the round-trip.
//!
//! This test proves the full headless inner loop works end-to-end:
//!   fixture (ratatui TestBackend) → ANSI SGR text → ansi_to_grid → SnapshotGrid
//!
//! The fixture binary must already be built (e.g. by `cargo build` or `cargo
//! test` run at the workspace level, which builds all members first).

use std::process::Command;
use tuiwright_core::{ansi_to_grid, snapshot::Color};

/// Locate the `tuiwright-fixture` binary under the workspace target directory.
fn fixture_bin() -> std::path::PathBuf {
    // CARGO_MANIFEST_DIR is crates/tuiwright-core; workspace root is two levels up.
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest.parent().unwrap().parent().unwrap();
    workspace.join("target/debug/tuiwright-fixture")
}

#[test]
fn headless_fixture_round_trip() {
    let bin = fixture_bin();
    if !bin.exists() {
        // The binary is built alongside other workspace members; skip if absent
        // (e.g. running `cargo test -p tuiwright-core` in isolation).
        eprintln!(
            "SKIP: tuiwright-fixture not found at {}. Run `cargo test` from the workspace root.",
            bin.display()
        );
        return;
    }

    // ── Run fixture in headless mode ──────────────────────────────────────
    let output = Command::new(&bin)
        .arg("--snapshot-ansi")
        .env("COLUMNS", "80")
        .env("LINES", "24")
        .output()
        .expect("failed to spawn tuiwright-fixture");

    assert!(
        output.status.success(),
        "fixture exited non-zero: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let ansi = String::from_utf8_lossy(&output.stdout);
    assert!(
        !ansi.is_empty(),
        "fixture produced no output on --snapshot-ansi"
    );

    // ── Decode ANSI → SnapshotGrid ────────────────────────────────────────
    let grid = ansi_to_grid(&ansi, 80, 24);

    assert_eq!(grid.cols, 80, "grid width");
    assert_eq!(grid.rows, 24, "grid height");
    assert_eq!(grid.cells.len(), 80 * 24, "cell count == cols * rows");

    // ── Plain-text content checks ─────────────────────────────────────────
    let text = grid.to_plain_text();
    assert!(
        text.contains("tuiwright fixture"),
        "title should appear in plain text; got:\n{text}"
    );
    assert!(
        text.contains("Design tokens"),
        "list items should appear; got:\n{text}"
    );
    assert!(
        text.contains("Design System Components"),
        "list block title should appear; got:\n{text}"
    );

    // ── Style checks on the title bar (row 0) ────────────────────────────
    // The title bar Paragraph renders " ◈ tuiwright fixture …" with
    //   fg=White (Ansi 15), bg=Blue (Ansi 4), bold=true
    // in the first Span of row 0 (the leading space is inside the styled span).
    let row0 = &grid.cells[..80];

    // Find first cell with a non-default style (the leading " " inside the span).
    let title_cell = row0
        .iter()
        .find(|c| c.style.bg.is_some())
        .expect("row 0 should have cells with a background colour");

    assert!(
        title_cell.style.bold,
        "title bar cell should be bold; style={:?}",
        title_cell.style
    );
    match &title_cell.style.bg {
        Some(Color::Ansi(4)) => {} // Blue ✓
        other => panic!("title bar bg should be Blue (Ansi(4)), got {other:?}"),
    }
    match &title_cell.style.fg {
        // White maps to Ansi(15) via the bright fg path.
        Some(Color::Ansi(15)) => {} // White ✓
        other => panic!("title bar fg should be White (Ansi(15)), got {other:?}"),
    }

    // ── Style checks on the selected list item (row 4) ───────────────────
    // Layout: title(3 rows) + list top border(1 row) = row 4 is first item.
    // Selected item style: fg=Yellow (Ansi 3), bg=DarkGray (Ansi 8), bold=true.
    let row4_start = 4 * 80;
    let row4 = &grid.cells[row4_start..row4_start + 80];

    // Find first non-space, non-border cell in the list content area.
    let selected_cell = row4
        .iter()
        .find(|c| c.symbol.trim() != "" && c.symbol != "│" && c.symbol != "►")
        .expect("row 4 should have content cells");

    assert!(
        selected_cell.style.bold,
        "selected list item should be bold; style={:?}",
        selected_cell.style
    );
    match &selected_cell.style.fg {
        Some(Color::Ansi(3)) => {} // Yellow ✓
        other => panic!("selected item fg should be Yellow (Ansi(3)), got {other:?}"),
    }

    // ── With replay: Down moves selection to second item ─────────────────
    let events_file = std::env::temp_dir().join("tuiwright_test_events.ndjson");
    std::fs::write(&events_file, r#"{"key":"Down"}"#).unwrap();

    let replayed = Command::new(&bin)
        .arg("--snapshot-ansi")
        .arg("--replay")
        .arg(&events_file)
        .env("COLUMNS", "80")
        .env("LINES", "24")
        .output()
        .expect("failed to run fixture with replay");

    assert!(replayed.status.success());
    let replayed_ansi = String::from_utf8_lossy(&replayed.stdout);
    let replayed_grid = ansi_to_grid(&replayed_ansi, 80, 24);
    let replayed_text = replayed_grid.to_plain_text();

    // After one Down, status bar should mention the second item.
    assert!(
        replayed_text.contains("Color scales"),
        "after Down, 'Color scales' should be selected; text:\n{replayed_text}"
    );

    std::fs::remove_file(&events_file).ok();
}
