# Changelog

All notable changes to tuiwright are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- crates.io metadata (keywords, categories) for `tuiwright-core` and `tuiwright-mcp`
- `CONTRIBUTING.md` with dev setup and PR workflow
- `CHANGELOG.md` (this file)
- Release automation: tag-triggered GitHub release workflow
- Filled `CLAUDE.md` Build/Test, Architecture, and Conventions sections

---

## [0.2.0] — 2026-06-06  *(M5 — Dogfood)*

### Added
- End-to-end example replays using spectrum-design-data NDJSON fixtures
- Golden snapshot baselines for key spectrum-design-data screens
- Walkthrough document showing Claude driving a real TUI via MCP tools
- Defined regression-test boundary between tuiwright and downstream design-data repo

---

## [0.1.4] — 2026-06-06  *(M4 — Harden & test)*

### Added
- Live-path end-to-end integration test (rmux-gated, parallel to `freeze_available()`)
- ANSI decode edge-case coverage: cursor moves, partial SGR sequences, multi-byte graphemes
- Error-path tests: no session, timeout, absent `freeze`/`agg` binaries
- macOS matrix in `build-live` CI job; `freeze` installed in macOS step

### Fixed
- macOS `freeze` tar extraction path corrected for arm64/x86_64

---

## [0.1.3] — 2026-06-06  *(M3 — Expand tool surface)*

### Added
- `diff` module in `tuiwright-core`: cell-by-cell `SnapshotGrid` comparison
- Baseline persistence: `.snap.json` read/write, `baseline_dir` in `Config`
- `tui_diff` MCP tool: diff current snapshot against saved baseline
- `tui_assert` MCP tool: text/style assertions with focused diff on failure
- Integration tests against `tuiwright-fixture` exercising diff and assert

---

## [0.1.2] — 2026-06-06  *(M2 — Live path + recording)*

### Added
- Real `rmux-sdk` 0.5 bindings: `EnsureSession` builder, `PaneCell` API
- `tui_record_start` / `tui_record_stop`: asciinema v2 `.cast` recording
- `tui_to_gif`: convert `.cast` to GIF via `agg`
- `build-live` CI job restored as a required gate

---

## [0.1.0] — 2026-06-06  *(M1 — Headless inner loop)*

### Added
- Initial workspace scaffold (`tuiwright-core`, `tuiwright-mcp`, `tuiwright-fixture`)
- `ansi_to_grid`: SGR decode producing a `SnapshotGrid` of styled cells
- `tui_headless` MCP tool: replay NDJSON → ANSI → text grid + PNG
- `freeze` integration: ANSI output rendered to PNG via subprocess
- `tuiwright-fixture`: minimal ratatui app used by integration tests
- CI: fmt, clippy, MSRV (1.88), security audit, headless and live gates
