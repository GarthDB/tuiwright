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

## M3 — Expand tool surface  `tuiwright-41r`
**Goal:** turn "see the TUI" into "verify the TUI" with visual diffing and assertions.

| # | Task | Bead |
|---|------|------|
| 1 | `diff` module in `tuiwright-core`: cell-by-cell `SnapshotGrid` comparison | `tuiwright-41r.1` |
| 2 | Baseline persistence: `.snap.json` read/write, `baseline_dir` in `Config` | `tuiwright-41r.2` |
| 3 | `tui_diff` MCP tool: diff current snapshot vs saved baseline | `tuiwright-41r.3` |
| 4 | `tui_assert` MCP tool: text/style assertions with focused diff on failure | `tuiwright-41r.4` |
| 5 | Tests against `tuiwright-fixture` exercising diff + assert | `tuiwright-41r.5` |

**Done when:** a baseline can be created, a later run diffs against it, and `tui_assert`
passes/fails with clear output; all covered by the `--no-default-features` test gate.

---

## M4 — Harden & test  `tuiwright-btv`
**Goal:** raise confidence on the live path and ANSI decoder before others depend on them.

| # | Task | Bead |
|---|------|------|
| 1 | Live-path integration test (rmux-gated, like `freeze_available()`) | `tuiwright-btv.1` |
| 2 | ANSI decode edge cases: cursor moves, partial SGR, multi-byte graphemes | `tuiwright-btv.2` |
| 3 | Error-path coverage: no session, timeout, absent `freeze`/`agg` | `tuiwright-btv.3` |
| 4 | CI: macOS matrix, `freeze` in `build-live` | `tuiwright-btv.4` |

**Done when:** the live path has ≥1 real end-to-end test, decoder edge cases are covered,
and CI exercises the image branch on at least one runner.

---

## M5 — Dogfood (spectrum-design-data)  `tuiwright-hrs`
**Goal:** prove the loop on the real Spectrum Design Data TUI — the reason this project exists.

| # | Task | Bead |
|---|------|------|
| 1 | Example NDJSON replays + golden snapshots for key screens | `tuiwright-hrs.1` |
| 2 | Documented walkthrough of Claude driving the TUI via MCP tools | `tuiwright-hrs.2` |
| 3 | Decide regression-test boundary (tuiwright vs design-data repo) | `tuiwright-hrs.3` |

**Done when:** a reproducible example shows tuiwright capturing and verifying the real
design-data TUI, runnable from the README.

---

## M6 — Publish & distribute  `tuiwright-0ql`
**Goal:** make tuiwright installable and contributable by others.

| # | Task | Bead |
|---|------|------|
| 1 | Fill `CLAUDE.md` placeholders; README getting-started guide | `tuiwright-0ql.1` |
| 2 | Add `LICENSE`, `CONTRIBUTING.md`, `CHANGELOG.md` | `tuiwright-0ql.2` |
| 3 | crates.io metadata + publish `tuiwright-core` then `tuiwright-mcp` | `tuiwright-0ql.3` |
| 4 | Release automation (tag → GitHub release, consider `cargo-dist`) | `tuiwright-0ql.4` |

**Done when:** a new user can `cargo install tuiwright-mcp` and get a working MCP server,
and a tagged release exists.
