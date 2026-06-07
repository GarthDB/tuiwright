# Project Instructions for AI Agents

This file provides instructions and context for AI coding agents working on this project.

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:6cd5cc61 -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/SYNC_CONCEPTS.md for details and anti-patterns.

## Agent Context Profiles

The managed Beads block is task-tracking guidance, not permission to override repository, user, or orchestrator instructions.

- **Conservative (default)**: Use `bd` for task tracking. Do not run git commits, git pushes, or Dolt remote sync unless explicitly asked. At handoff, report changed files, validation, and suggested next commands.
- **Minimal**: Keep tool instruction files as pointers to `bd prime`; use the same conservative git policy unless active instructions say otherwise.
- **Team-maintainer**: Only when the repository explicitly opts in, agents may close beads, run quality gates, commit, and push as part of session close. A current "do not commit" or "do not push" instruction still wins.

## Session Completion

This protocol applies when ending a Beads implementation workflow. It is subordinate to explicit user, repository, and orchestrator instructions.

1. **File issues for remaining work** - Create beads for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **Handle git/sync by active profile**:
   ```bash
   # Conservative/minimal/default: report status and proposed commands; wait for approval.
   git status

   # Team-maintainer opt-in only, unless current instructions forbid it:
   git pull --rebase
   bd dolt push
   git push
   git status
   ```
5. **Hand off** - Summarize changes, validation, issue status, and any blocked sync/commit/push step

**Critical rules:**
- Explicit user or orchestrator instructions override this Beads block.
- Do not commit or push without clear authority from the active profile or the current user request.
- If a required sync or push is blocked, stop and report the exact command and error.
<!-- END BEADS INTEGRATION -->


## Build & Test

```bash
# Build (headless-only, no rmux required)
cargo build --no-default-features

# Build with live path (requires rmux daemon)
cargo build

# Run headless tests (CI-safe, no external deps except freeze)
cargo test --no-default-features

# Run all tests including live path (requires rmux + freeze)
cargo test --features live

# Lint
cargo clippy --no-default-features -- -D warnings

# Format
cargo fmt --all

# Security audit
cargo audit
```

The `live` feature gates everything that touches the rmux daemon.
CI headless tests require `freeze` on PATH (ANSI → PNG); live tests additionally require `rmux`.

## Architecture Overview

Workspace with three crates:

- **`tuiwright-core`** — library: ANSI decode (`ansi.rs`), config (`config.rs`), rendering/PNG (`render.rs`), snapshot grid (`snapshot.rs`), visual diff (`diff.rs`).
- **`tuiwright-mcp`** — binary (`tuiwright`): MCP server over stdio using `rmcp`. Exposes all `tui_*` tools. Live tools are feature-gated behind `live = ["dep:rmux-sdk"]`.
- **`tuiwright-fixture`** — small ratatui app used by integration tests; not published to crates.io.

Two rendering paths share one MCP tool surface:

```
headless  →  tui_headless(ndjson)  →  headless_snapshot cmd  →  ANSI stdout  →  grid + PNG
live      →  tui_open  →  rmux session  →  tui_send_keys / tui_snapshot / tui_record_*
```

## Conventions & Patterns

- **Feature gating**: headless tests run with `--no-default-features`; live tests require `--features live`. Never make headless code depend on `rmux-sdk`.
- **`schemars`**: all MCP tool input structs derive `JsonSchema` so `rmcp` generates the JSON Schema automatically.
- **Snapshot tests**: use `insta` with RON serialisation. Run `cargo insta review` to accept new snapshots.
- **Error types**: use `thiserror` for library errors; `anyhow` for binary/test error propagation.
- **Config**: `tuiwright.toml` in the target project root; loaded by `tuiwright_core::Config`. The `headless_snapshot` field uses `{}` as the NDJSON path placeholder.
