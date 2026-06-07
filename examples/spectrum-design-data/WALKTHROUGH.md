# Driving spectrum-design-data via tuiwright MCP

This walkthrough shows how Claude can drive a real TUI application using tuiwright's MCP tools.
The subject is [spectrum-design-data](https://github.com/GarthDB/spectrum-design-data), a design
token explorer for Adobe Spectrum.

## Prerequisites

1. Build the tuiwright MCP server:
   ```bash
   cargo build --release -p tuiwright-mcp
   ```

2. Build the spectrum-design-data binary (run inside that repo):
   ```bash
   cargo build --release -p design-data
   ```

3. Create `tuiwright.toml` in the spectrum-design-data repo root
   (replace `/absolute/path/to` with the actual path on your machine):
   ```toml
   [launch]
   command = "/absolute/path/to/spectrum-design-data/sdk/target/release/design-data"
   args = ["packages/design-data/tokens", "--components", "packages/design-data/components"]
   [size]
   cols = 120
   rows = 40
   headless_snapshot = "/absolute/path/to/spectrum-design-data/sdk/target/release/design-data packages/design-data/tokens --components packages/design-data/components --snapshot-ansi --replay {}"
   ```

## Headless usage (no live session required)

The `tui_headless` tool runs the app's `headless_snapshot` command with a replay file and returns
a snapshot without launching a real terminal. It accepts an NDJSON path via the `ndjson` field.

### Capture the initial browse screen

Pass the empty replay file (zero messages → initial state):

```
Tool: tui_headless
Input: { "ndjson": "examples/spectrum-design-data/browse.ndjson", "format": "text" }
```

Returns plain text showing the initial token browser:

```
▶  4166 tokens · packages/design-data/tokens
   Spectrum Design Data  v0.2.0
   : commands · / search · ? help · q quit
   >
   :query <expr>             Filter tokens  e.g. background-color/*
   ...
```

### Query for background-color tokens

```
Tool: tui_headless
Input: { "ndjson": "examples/spectrum-design-data/query-background-color.ndjson", "format": "text" }
```

The replay file contains one message:
```json
{"PaletteSubmit":"query property=background-color"}
```

Returns the query result grid showing 19 matched tokens with their UUID, file, and layer columns.
Column values are truncated to fit the configured terminal width (120 cols).

### Inspect a component schema

```
Tool: tui_headless
Input: { "ndjson": "examples/spectrum-design-data/describe-button.ndjson", "format": "text" }
```

The replay file contains:
```json
{"PaletteSubmit":"describe button"}
```

Returns the Button component schema — options, variants, states — rendered as a scrollable JSON
pane inside the TUI frame. Long URLs are truncated to the terminal width.

## Live session usage

For interactive exploration, use a live rmux session. This requires rmux to be installed.

```
# Open a session
Tool: tui_open
Output: { "session_id": "sdd-0" }

# Take a snapshot of the current state
Tool: tui_snapshot
Input: { "session_id": "sdd-0", "format": "text" }

# Navigate: open the command palette
Tool: tui_send_keys
Input: { "session_id": "sdd-0", "keys": ":" }

# Submit a query command
Tool: tui_send_keys
Input: { "session_id": "sdd-0", "keys": "query property=background-color\r" }

# Wait for the result to render
Tool: tui_wait_for
Input: { "session_id": "sdd-0", "text": "matched" }

# Snapshot the result
Tool: tui_snapshot
Input: { "session_id": "sdd-0", "format": "text" }

# Close when done
Tool: tui_close
Input: { "session_id": "sdd-0" }
```

## Regression testing with tui_diff

`tui_diff` compares a headless snapshot against a named baseline (`.snap.json` file stored in
`baseline_dir`). It reports which cells changed — catching regressions in token count, column
layout, or color coding. The `baseline` field is a name without extension; the tool appends
`.snap.json` automatically.

Create a baseline on first run:

```
Tool: tui_diff
Input: {
  "baseline": "query-background-color",
  "ndjson": "examples/spectrum-design-data/query-background-color.ndjson",
  "create_if_missing": true
}
```

On subsequent runs (without `create_if_missing`), tui_diff returns a diff if anything changed:

```
Tool: tui_diff
Input: {
  "baseline": "query-background-color",
  "ndjson": "examples/spectrum-design-data/query-background-color.ndjson"
}
```

## Example replay files

The `examples/spectrum-design-data/` directory contains ready-to-use replay scripts:

| File | Screen |
|------|--------|
| `browse.ndjson` | Initial token browser (empty replay — shows initial state) |
| `query-background-color.ndjson` | Query result for `property=background-color` (19 tokens) |
| `describe-button.ndjson` | Button component schema view |

Golden ANSI snapshots (`.ansi`) are stored alongside each replay file and were captured from
design-data v0.2.0 with 4166 tokens loaded at 120×40. Column values and URLs are truncated to
fit the terminal width — this is expected and is not a bug in the golden files.
