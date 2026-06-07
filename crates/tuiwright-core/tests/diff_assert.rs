//! Integration tests for the diff and assert functionality.
//!
//! Tests cover:
//! - `tuiwright_core::diff` on grids produced by the fixture (proves style-aware diffing)
//! - `SnapshotGrid::save_baseline` / `load_baseline` round-trip
//! - Assertion helpers built from the public API (`to_plain_text` + `contains`)

use std::process::Command;
use tuiwright_core::{ansi_to_grid, diff, snapshot::Color};

fn fixture_bin() -> std::path::PathBuf {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest.parent().unwrap().parent().unwrap();
    workspace.join("target/debug/tuiwright-fixture")
}

fn run_fixture(extra_args: &[&str]) -> tuiwright_core::SnapshotGrid {
    let bin = fixture_bin();
    assert!(
        bin.exists(),
        "tuiwright-fixture not found at {}. Run `cargo test` from workspace root.",
        bin.display()
    );
    let output = Command::new(&bin)
        .arg("--snapshot-ansi")
        .args(extra_args)
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
    ansi_to_grid(&ansi, 80, 24)
}

/// Two snapshots of the same state must diff as identical.
#[test]
fn diff_identical_snapshots_match() {
    let bin = fixture_bin();
    if !bin.exists() {
        eprintln!("SKIP: fixture not built");
        return;
    }
    let g1 = run_fixture(&[]);
    let g2 = run_fixture(&[]);
    let d = diff(&g1, &g2);
    assert!(
        d.is_match(),
        "identical snapshots should match; got: {}",
        d.display()
    );
}

/// After a Down keypress the selected row changes style — diff detects this.
#[test]
fn diff_detects_selection_change() {
    let bin = fixture_bin();
    if !bin.exists() {
        eprintln!("SKIP: fixture not built");
        return;
    }

    let events_file = std::env::temp_dir().join("tuiwright_diff_test.ndjson");
    std::fs::write(&events_file, r#"{"key":"Down"}"#).unwrap();

    let before = run_fixture(&[]);
    let after = run_fixture(&["--replay", events_file.to_str().unwrap()]);

    let d = diff(&before, &after);
    assert!(!d.is_match(), "snapshots before/after Down should differ");
    // The selection highlight moves — at minimum the symbol or style of the
    // formerly-selected row and newly-selected row change.
    assert!(
        d.changed_cells.len() >= 2,
        "at least 2 cells should change when selection moves; got {} changed",
        d.changed_cells.len()
    );

    // Verify colour change: the new selection has Yellow fg (Ansi 3).
    let has_yellow = d
        .changed_cells
        .iter()
        .any(|cd| matches!(cd.actual.style.fg, Some(Color::Ansi(3))));
    assert!(
        has_yellow,
        "at least one changed cell should have Yellow fg after Down"
    );

    std::fs::remove_file(&events_file).ok();
}

/// Baseline save/load round-trip preserves the grid exactly.
#[test]
fn baseline_save_load_roundtrip() {
    let bin = fixture_bin();
    if !bin.exists() {
        eprintln!("SKIP: fixture not built");
        return;
    }

    let grid = run_fixture(&[]);
    let path = std::env::temp_dir().join("tuiwright_baseline_test.snap.json");
    grid.save_baseline(&path).expect("save_baseline failed");

    let loaded = tuiwright_core::SnapshotGrid::load_baseline(&path).expect("load_baseline failed");

    let d = diff(&grid, &loaded);
    assert!(
        d.is_match(),
        "loaded baseline should match original; diff: {}",
        d.display()
    );

    std::fs::remove_file(&path).ok();
}

/// Text assertion helpers: plain_text contains expected strings.
#[test]
fn assert_contains_text() {
    let bin = fixture_bin();
    if !bin.exists() {
        eprintln!("SKIP: fixture not built");
        return;
    }

    let grid = run_fixture(&[]);
    let text = grid.to_plain_text();

    // Things that must be present.
    for expected in &[
        "tuiwright fixture",
        "Design tokens",
        "Design System Components",
    ] {
        assert!(
            text.contains(expected),
            "expected {:?} in rendered text; got:\n{text}",
            expected
        );
    }

    // Things that must not be present.
    for forbidden in &["panic", "error", "FAIL"] {
        assert!(
            !text.contains(forbidden),
            "unexpectedly found {:?} in rendered text",
            forbidden
        );
    }
}
