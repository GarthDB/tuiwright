# Driving spectrum-design-data via tuiwright MCP

This walkthrough shows how Claude can drive a real TUI application using tuiwright's MCP tools.
The subject is [spectrum-design-data](https://github.com/GarthDB/spectrum-design-data), a design
token explorer for Adobe Spectrum.

## Prerequisites

1. Build the release binary:
   ```bash
   cargo build --release -p tuiwright-mcp
   ```

2. The spectrum-design-data binary must be built:
   ```bash
   # In the spectrum-design-data repo:
   cargo build --release -p design-data
   ```

3. `tuiwright.toml` is already configured in `spectrum-design-data/`:
   ```toml
   [launch]
   command = ".../design-data"
   args = ["packages/design-data/tokens", "--components", "packages/design-data/components"]
   [size]
   cols = 120
   rows = 40
   headless_snapshot = ".../design-data packages/design-data/tokens \
     --components packages/design-data/components --snapshot-ansi --replay {}"
   ```

## Headless usage (no live session required)

The `tui_headless` tool replays an NDJSON message stream and returns a snapshot without
launching a real terminal.

### Capture the initial browse screen

```
Tool: tui_headless
Input: { "replay": [] }
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

Write a replay file and snapshot:

```json
{"PaletteSubmit":"query property=background-color"}
```

```
Tool: tui_headless
Input: { "replay": [{"PaletteSubmit":"query property=background-color"}] }
```

Returns the query result grid showing 19 matched tokens with their UUID, file, and layer columns.

### Inspect a component schema

```
Tool: tui_headless
Input: { "replay": [{"PaletteSubmit":"describe button"}] }
```

Returns the Button component schema — options, variants, states — rendered as a scrollable JSON
pane inside the TUI frame.

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

## Regression testing with baselines

Capture a golden baseline once:

```
Tool: tui_headless
Input: { "replay": [{"PaletteSubmit":"query property=background-color"}], "baseline": "examples/spectrum-design-data/query-background-color" }
```

On subsequent runs, `tui_diff` compares the live snapshot against the baseline and reports
which cells changed — catching regressions in token count, column layout, or color coding.

## Example replay files

The `examples/spectrum-design-data/` directory contains ready-to-use replay scripts:

| File | Screen |
|------|--------|
| `browse.ndjson` | Initial token browser (empty replay — shows initial state) |
| `query-background-color.ndjson` | Query result for `property=background-color` (19 tokens) |
| `describe-button.ndjson` | Button component schema view |

Golden ANSI snapshots (`.ansi`) are stored alongside each replay file and were captured from
design-data v0.2.0 with 4166 tokens loaded.
