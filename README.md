# tuiwright

> **Playwright-style tools for developing TUI apps with AI agents.**

tuiwright is an MCP (Model Context Protocol) server that gives Claude Code a
tight feedback loop for building terminal UIs — the same way Claude uses
Playwright to iterate on web UIs.

```
edit code → tui_headless → see rendered grid + PNG → iterate
                    ↓
              tui_open → tui_send_keys → tui_snapshot → verify live behavior
```

## Why

When Claude develops HTML it can *see* the rendered output (Playwright screenshot),
assert on it, and fix it — without leaving the conversation.  TUIs have had no
equivalent.  tuiwright fills that gap:

| What Claude can do with tuiwright | How |
|-----------------------------------|-----|
| See the rendered TUI as text + PNG | `tui_snapshot` / `tui_headless` |
| Drive keyboard input deterministically | `tui_send_keys` |
| Wait for expected output | `tui_wait_for` |
| Resize the terminal | `tui_resize` |
| Record a session as `.cast` / GIF | `tui_record_start/stop`, `tui_to_gif` |
| Fast headless iteration (no PTY) | `tui_headless` |

## Architecture

Two rendering paths behind one MCP tool surface:

```
Claude ── MCP tools ──▶ tuiwright-mcp (rmcp, stdio)
                          │
                          ├─ HEADLESS (fast, deterministic)
                          │    tui_headless(ndjson) → headless_snapshot cmd
                          │    → ANSI output → text grid + freeze → PNG
                          │
                          └─ LIVE (verify real terminal)
                               tui_open → rmux ensure_session
                               tui_send_keys → rmux send_text
                               tui_snapshot → rmux snapshot() → text + freeze → PNG
                               tui_record_* → asciinema .cast → agg → GIF
```

**External tools:**
- [`rmux`](https://github.com/Helvesec/rmux) — daemon-backed terminal multiplexer; typed SDK for driving real TUIs
- [`freeze`](https://github.com/charmbracelet/freeze) — render ANSI output → PNG *(required for image output)*
- [`asciinema`](https://asciinema.org/) + [`agg`](https://github.com/asciinema/agg) — session recording → GIF *(optional)*

## Quick start

### Install dependencies

```bash
# freeze (PNG rendering — required for image/both format)
brew install charmbracelet/tap/freeze

# rmux (live terminal control)
cargo install rmux   # or see https://github.com/Helvesec/rmux

# agg (GIF from asciinema — optional)
cargo install agg
```

### Build tuiwright

```bash
git clone https://github.com/GarthDB/tuiwright
cd tuiwright
cargo build --release
```

### Configure your project

Create `tuiwright.toml` in your TUI project root:

```toml
[launch]
command = "target/debug/my-tui"
args = ["--data", "data.json"]

[size]
cols = 80
rows = 24

# Command to render a state NDJSON headlessly to styled ANSI stdout.
# {} is substituted with the NDJSON path at runtime.
headless_snapshot = "target/debug/my-tui --replay {} --snapshot-ansi"
```

### Register as an MCP server in Claude Code

```json
// .claude/settings.json
{
  "mcpServers": {
    "tuiwright": {
      "command": "/path/to/tuiwright",
      "args": [],
      "type": "stdio"
    }
  }
}
```

### Use from Claude Code

```
# Fast inner loop (headless, deterministic)
tui_headless(ndjson="/path/to/session.ndjson", format="both")

# Live verification
tui_open(cols=80, rows=24)
tui_send_keys(keys=":query\r")
tui_wait_for(text="Results")
tui_snapshot(format="both")
tui_resize(cols=120, rows=40)
tui_snapshot(format="both")
tui_close()
```

## Headless contract

The `headless_snapshot` command must:
1. Accept a path to an NDJSON file (recorded message stream)
2. Replay the messages into the app's update loop
3. Perform one final render
4. Print **styled ANSI output** to stdout (SGR escape sequences)

This is a thin adapter per app.  For ratatui apps using the Elm Architecture
(pure `update` + stateless `draw`), it is typically ~30 lines reusing the
existing `TestBackend`.

## MCP tools reference

| Tool | Description |
|------|-------------|
| `tui_open` | Launch the TUI app in a live rmux pane |
| `tui_send_keys` | Send keystrokes (Enter `\r`, Esc `\x1b`, arrows `\x1b[A/B/C/D`, Ctrl-C `\x03`) |
| `tui_snapshot` | Capture the pane as text grid and/or PNG |
| `tui_wait_for` | Block until text appears in the pane |
| `tui_resize` | Resize the terminal pane |
| `tui_close` | Close the rmux session |
| `tui_record_start` | Begin an asciinema `.cast` recording |
| `tui_record_stop` | Stop the recording |
| `tui_to_gif` | Convert a `.cast` to a GIF via `agg` |
| `tui_headless` | Headless replay + render (no PTY, deterministic) |

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
