# tuiwright Roadmap

Milestones are sequenced: each must be complete before the next begins.
Beads IDs are listed next to each milestone for tracking (`bd show <id>`).

---

## ✅ M1 — Headless inner loop  *(shipped)*
SGR decode (`ansi_to_grid`), self-contained fixture TUI (`tuiwright-fixture`),
integration test, `freeze` in CI. The deterministic path is the primary inner loop.
**PR:** [#2](https://github.com/GarthDB/tuiwright/pull/2)

---

## ✅ M2 — Live path + recording  *(shipped)*
Real `rmux-sdk` 0.5 bindings (`EnsureSession` builder, `PaneCell` API), actual
asciinema v2 recording (`tui_record_start`/`tui_record_stop`), `build-live` restored
as a required CI gate.
**PR:** [#4](https://github.com/GarthDB/tuiwright/pull/4)

---

## ✅ M3 — Expand tool surface  *(shipped)*
`diff` module, baseline persistence (`.snap.json`), `tui_diff` and `tui_assert` MCP
tools with focused diff output on failure. All covered by the `--no-default-features`
test gate.
**PR:** [#6](https://github.com/GarthDB/tuiwright/pull/6)

---

## ✅ M4 — Harden & test  *(shipped)*
Live-path integration test (rmux-gated), ANSI decode edge cases (cursor moves, partial
SGR, multi-byte graphemes), error-path coverage, macOS matrix in CI.
**PR:** [#7](https://github.com/GarthDB/tuiwright/pull/7)

---

## ✅ M5 — Dogfood (spectrum-design-data)  *(shipped)*
End-to-end example replays and golden snapshots for spectrum-design-data screens.
Documented walkthrough of Claude driving the TUI via MCP tools.
**PR:** [#8](https://github.com/GarthDB/tuiwright/pull/8)

---

## ✅ M6 — Publish & distribute  *(shipped)*
CLAUDE.md filled, CONTRIBUTING.md and CHANGELOG.md added, crates.io metadata, release-plz
automation (auto release PRs + crates.io publish), binary distribution via GitHub Actions.
**PR:** [#9](https://github.com/GarthDB/tuiwright/pull/9)

---

## M7 — Stability & docs  `tuiwright-675`
**Goal:** make tuiwright trustworthy for external users — stable public API, discoverable
docs, and a safe upgrade path.

| # | Task | Bead |
|---|------|------|
| 1 | Confirm `tuiwright-fixture` publish exclusion | `tuiwright-675.1` |
| 2 | `[package.metadata.docs.rs]` config in `tuiwright-core/Cargo.toml` | `tuiwright-675.2` |
| 3 | Public API `///` doc comments for all pub items in `tuiwright-core` | `tuiwright-675.3` |
| 4 | `cargo semver-checks` job in CI | `tuiwright-675.4` |
| 5 | First-run error quality pass (`freeze`/`rmux` messages with install hints) | `tuiwright-675.5` |

**Done when:** `cargo doc` produces a clean docs.rs-quality page, `semver-checks` runs in
CI, and missing-dependency errors suggest exact install commands.
