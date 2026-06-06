//! MCP tool definitions for tuiwright.
//!
//! Each public async method decorated with `#[tool]` becomes an MCP tool that
//! Claude Code can call.  The `#[tool_router]` and `#[tool_handler]` macros
//! auto-generate the dispatch boilerplate.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tuiwright_core::Config;

// ---------------------------------------------------------------------------
// Server struct
// ---------------------------------------------------------------------------

/// Shared state for the MCP server.
#[derive(Clone)]
pub struct TuiwrightServer {
    config: Config,
    /// Active live session (rmux), if any.
    #[cfg(feature = "live")]
    session: Arc<Mutex<Option<LiveSession>>>,
    /// Path to an in-progress asciinema recording, if any.
    recording: Arc<Mutex<Option<std::path::PathBuf>>>,
    /// ToolRouter must be stored on the struct (rmcp 0.16 pattern).
    tool_router: rmcp::handler::server::tool::ToolRouter<Self>,
}

#[cfg(feature = "live")]
#[derive(Clone)]
struct LiveSession {
    name: String,
    client: rmux_sdk::Rmux,
    cols: u16,
    rows: u16,
}

impl TuiwrightServer {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            #[cfg(feature = "live")]
            session: Arc::new(Mutex::new(None)),
            recording: Arc::new(Mutex::new(None)),
            tool_router: Self::tool_router(),
        }
    }
}

// ---------------------------------------------------------------------------
// Input types (must derive JsonSchema for rmcp to build the tool schema)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct TuiOpenInput {
    /// Override the command from tuiwright.toml.
    pub command: Option<String>,
    /// Extra arguments appended to the launch command.
    #[serde(default)]
    pub args: Vec<String>,
    /// Terminal width in columns (default: from config).
    pub cols: Option<u16>,
    /// Terminal height in rows (default: from config).
    pub rows: Option<u16>,
    /// rmux session name (default: "tuiwright").
    pub session: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct TuiSendKeysInput {
    /// Keys to send.  Use `\r` for Enter, `\x1b` for Escape, `\x03` for Ctrl-C,
    /// `\x1b[A/B/C/D` for arrow keys.
    pub keys: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum SnapshotFormat {
    /// Plain text grid (no colours).
    Text,
    /// PNG screenshot path (requires `freeze` in $PATH).
    Image,
    /// Both text grid and PNG path.
    #[default]
    Both,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct TuiSnapshotInput {
    /// Output format: "text", "image", or "both" (default).
    #[serde(default)]
    pub format: SnapshotFormat,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct TuiWaitForInput {
    /// Text string to wait for in the pane output.
    pub text: String,
    /// Timeout in milliseconds (default: 5000).
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct TuiResizeInput {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct TuiRecordStartInput {
    /// Path for the output `.cast` file.
    pub path: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct TuiToGifInput {
    /// Path to an existing `.cast` file.
    pub cast: String,
    /// Desired output `.gif` path.
    pub gif: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct TuiHeadlessInput {
    /// Path to an NDJSON file (recorded message stream for the target app).
    pub ndjson: String,
    /// Terminal width (default: from config).
    pub cols: Option<u16>,
    /// Terminal height (default: from config).
    pub rows: Option<u16>,
    /// Output format: "text", "image", or "both" (default).
    #[serde(default)]
    pub format: SnapshotFormat,
}

// ---------------------------------------------------------------------------
// Tool implementations
// ---------------------------------------------------------------------------

#[tool_router]
impl TuiwrightServer {
    /// Open a live terminal pane and launch the TUI app inside it via rmux.
    /// Must be called before tui_send_keys / tui_snapshot / tui_wait_for.
    #[tool(
        description = "Open a live terminal pane and launch the configured TUI app in it via rmux. Call this before tui_send_keys, tui_snapshot, or tui_wait_for."
    )]
    async fn tui_open(
        &self,
        Parameters(input): Parameters<TuiOpenInput>,
    ) -> Result<String, McpError> {
        #[cfg(not(feature = "live"))]
        {
            let _ = input;
            return Ok("live feature disabled — rebuild with --features live".to_string());
        }
        #[cfg(feature = "live")]
        {
            let cols = input.cols.unwrap_or(self.config.size.cols);
            let rows = input.rows.unwrap_or(self.config.size.rows);
            let session_name = input.session.as_deref().unwrap_or("tuiwright").to_string();

            let command = input
                .command
                .or_else(|| self.config.launch.command.clone())
                .ok_or_else(|| {
                    McpError::invalid_params(
                        "no command specified — set `launch.command` in tuiwright.toml or pass `command`",
                        None,
                    )
                })?;

            let client = rmux_sdk::Rmux::builder()
                .connect_or_start()
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;

            let mut full_cmd = command.clone();
            for arg in &self.config.launch.args {
                full_cmd.push(' ');
                full_cmd.push_str(arg);
            }
            for arg in &input.args {
                full_cmd.push(' ');
                full_cmd.push_str(arg);
            }

            let size = rmux_sdk::TerminalSizeSpec::new(cols, rows);
            client
                .ensure_session(
                    rmux_sdk::SessionName::new(&session_name)
                        .map_err(|e| McpError::internal_error(e.to_string(), None))?,
                    &full_cmd,
                    size,
                    rmux_sdk::SessionPolicy::RecreateIfExists,
                )
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;

            let mut guard = self.session.lock().await;
            *guard = Some(LiveSession {
                name: session_name.clone(),
                client,
                cols,
                rows,
            });

            Ok(format!(
                "Live pane opened: session={session_name}, size={cols}x{rows}, command={command}"
            ))
        }
    }

    /// Send keystrokes to the live terminal pane.
    #[tool(
        description = "Send keystrokes to the live terminal pane. Use \\r for Enter, \\x1b for Escape, \\x03 for Ctrl-C, \\x1b[A/B/C/D for arrow keys."
    )]
    async fn tui_send_keys(
        &self,
        Parameters(input): Parameters<TuiSendKeysInput>,
    ) -> Result<String, McpError> {
        #[cfg(not(feature = "live"))]
        {
            let _ = input;
            return Ok("live feature disabled".to_string());
        }
        #[cfg(feature = "live")]
        {
            let guard = self.session.lock().await;
            let session = guard.as_ref().ok_or_else(|| {
                McpError::invalid_params("no live session — call tui_open first", None)
            })?;
            session
                .client
                .pane(&session.name, 0, 0)
                .send_text(&input.keys)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            Ok(format!("sent: {:?}", input.keys))
        }
    }

    /// Snapshot the current pane content as a text grid and/or PNG.
    #[tool(
        description = "Snapshot the current live terminal pane. format: \"text\" (plain grid), \"image\" (PNG path, requires freeze), or \"both\" (default)."
    )]
    async fn tui_snapshot(
        &self,
        Parameters(input): Parameters<TuiSnapshotInput>,
    ) -> Result<String, McpError> {
        #[cfg(not(feature = "live"))]
        {
            let _ = input;
            return Ok("live feature disabled".to_string());
        }
        #[cfg(feature = "live")]
        {
            let guard = self.session.lock().await;
            let session = guard.as_ref().ok_or_else(|| {
                McpError::invalid_params("no live session — call tui_open first", None)
            })?;

            let snapshot = session
                .client
                .pane(&session.name, 0, 0)
                .snapshot()
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;

            let grid = rmux_snapshot_to_grid(snapshot, session.cols, session.rows);
            render_snapshot(&grid, &input.format)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))
        }
    }

    /// Wait until the pane contains the given text (polling rmux).
    #[tool(
        description = "Block until the live pane output contains `text`. timeout_ms defaults to 5000."
    )]
    async fn tui_wait_for(
        &self,
        Parameters(input): Parameters<TuiWaitForInput>,
    ) -> Result<String, McpError> {
        #[cfg(not(feature = "live"))]
        {
            let _ = input;
            return Ok("live feature disabled".to_string());
        }
        #[cfg(feature = "live")]
        {
            let timeout = std::time::Duration::from_millis(input.timeout_ms.unwrap_or(5000));
            let guard = self.session.lock().await;
            let session = guard.as_ref().ok_or_else(|| {
                McpError::invalid_params("no live session — call tui_open first", None)
            })?;
            session
                .client
                .pane(&session.name, 0, 0)
                .wait_for_text(&input.text, timeout)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            Ok(format!("found: {:?}", input.text))
        }
    }

    /// Resize the live terminal pane.
    #[tool(description = "Resize the live terminal pane to the given cols x rows.")]
    async fn tui_resize(
        &self,
        Parameters(input): Parameters<TuiResizeInput>,
    ) -> Result<String, McpError> {
        #[cfg(not(feature = "live"))]
        {
            let _ = input;
            return Ok("live feature disabled".to_string());
        }
        #[cfg(feature = "live")]
        {
            let mut guard = self.session.lock().await;
            let session = guard.as_mut().ok_or_else(|| {
                McpError::invalid_params("no live session — call tui_open first", None)
            })?;
            let size = rmux_sdk::TerminalSizeSpec::new(input.cols, input.rows);
            session
                .client
                .resize_pane(&session.name, 0, 0, size)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            session.cols = input.cols;
            session.rows = input.rows;
            Ok(format!("resized to {}x{}", input.cols, input.rows))
        }
    }

    /// Close the live terminal session.
    #[tool(description = "Close the live rmux terminal session.")]
    async fn tui_close(&self) -> Result<String, McpError> {
        #[cfg(not(feature = "live"))]
        return Ok("live feature disabled".to_string());
        #[cfg(feature = "live")]
        {
            let mut guard = self.session.lock().await;
            if let Some(session) = guard.take() {
                session.client.kill_session(&session.name).await.ok();
            }
            Ok("session closed".to_string())
        }
    }

    /// Start an asciinema recording of the live session.
    #[tool(
        description = "Start an asciinema recording of the live terminal session. `path` is the output .cast file."
    )]
    async fn tui_record_start(
        &self,
        Parameters(input): Parameters<TuiRecordStartInput>,
    ) -> Result<String, McpError> {
        let path = std::path::PathBuf::from(&input.path);
        let mut rec = self.recording.lock().await;
        if rec.is_some() {
            return Err(McpError::invalid_params(
                "a recording is already in progress — call tui_record_stop first",
                None,
            ));
        }
        *rec = Some(path.clone());
        Ok(format!(
            "recording started → {} (asciinema integration: phase 4)",
            path.display()
        ))
    }

    /// Stop the current asciinema recording.
    #[tool(description = "Stop the in-progress asciinema recording and finalise the .cast file.")]
    async fn tui_record_stop(&self) -> Result<String, McpError> {
        let mut rec = self.recording.lock().await;
        let path = rec
            .take()
            .ok_or_else(|| McpError::invalid_params("no recording in progress", None))?;
        Ok(format!("recording stopped → {}", path.display()))
    }

    /// Convert an asciinema .cast file to a GIF via `agg`.
    #[tool(
        description = "Convert an asciinema .cast file to a GIF using agg. Requires agg in $PATH."
    )]
    async fn tui_to_gif(
        &self,
        Parameters(input): Parameters<TuiToGifInput>,
    ) -> Result<String, McpError> {
        let cast = std::path::Path::new(&input.cast);
        let gif = std::path::Path::new(&input.gif);
        tuiwright_core::render::cast_to_gif(cast, gif)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(format!("GIF written to {}", gif.display()))
    }

    /// Run the app headlessly (NDJSON replay) and return a text grid and/or PNG.
    /// Deterministic — no PTY, no rmux.  The fast inner loop for iterative development.
    #[tool(
        description = "Run the TUI app headlessly by replaying an NDJSON message stream, then return the final rendered screen as a text grid and/or PNG (requires freeze). Deterministic and fast — the preferred inner loop for iterative development."
    )]
    async fn tui_headless(
        &self,
        Parameters(input): Parameters<TuiHeadlessInput>,
    ) -> Result<String, McpError> {
        let cmd_template = self
            .config
            .headless_snapshot
            .as_deref()
            .ok_or_else(|| McpError::invalid_params(
                "headless_snapshot not configured in tuiwright.toml — set it to your app's ANSI snapshot command, e.g. `design-data --replay {} --snapshot-ansi`",
                None,
            ))?;

        let cols = input.cols.unwrap_or(self.config.size.cols);
        let rows = input.rows.unwrap_or(self.config.size.rows);
        let cmd_str = cmd_template.replace("{}", &input.ndjson);

        // Split command into program + args (simple whitespace split — no shell quoting).
        let mut parts = cmd_str.split_whitespace();
        let bin = parts
            .next()
            .ok_or_else(|| McpError::invalid_params("headless_snapshot command is empty", None))?;
        let args: Vec<&str> = parts.collect();

        let output = tokio::process::Command::new(bin)
            .args(&args)
            .env("COLUMNS", cols.to_string())
            .env("LINES", rows.to_string())
            .output()
            .await
            .map_err(|e| {
                McpError::internal_error(format!("failed to run `{cmd_str}`: {e}"), None)
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(McpError::internal_error(
                format!("headless command failed: {stderr}"),
                None,
            ));
        }

        let ansi_output = String::from_utf8_lossy(&output.stdout).to_string();
        let grid = tuiwright_core::ansi_to_grid(&ansi_output, cols, rows);

        render_snapshot(&grid, &input.format)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))
    }
}

// ---------------------------------------------------------------------------
// ServerHandler impl
// ---------------------------------------------------------------------------

#[tool_handler]
impl ServerHandler for TuiwrightServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Playwright-style tools for developing TUI apps with Claude. \
                 Use tui_headless for the fast inner loop (deterministic, no PTY); \
                 use tui_open + tui_send_keys + tui_snapshot for live verification."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

/// Render a SnapshotGrid according to the requested format.
async fn render_snapshot(
    grid: &tuiwright_core::SnapshotGrid,
    format: &SnapshotFormat,
) -> anyhow::Result<String> {
    let text = grid.to_plain_text();
    match format {
        SnapshotFormat::Text => Ok(format!("```\n{text}```")),
        SnapshotFormat::Image => {
            let png = tmp_png_path();
            tuiwright_core::render::grid_to_png(grid, &png).await?;
            Ok(format!("PNG saved to {}", png.display()))
        }
        SnapshotFormat::Both => {
            let png = tmp_png_path();
            tuiwright_core::render::grid_to_png(grid, &png).await?;
            Ok(format!("PNG: {}\n\n```\n{text}```", png.display()))
        }
    }
}

fn tmp_png_path() -> std::path::PathBuf {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    std::env::temp_dir().join(format!("tuiwright_{ts}.png"))
}

/// Convert an rmux pane snapshot to a tuiwright SnapshotGrid.
#[cfg(feature = "live")]
fn rmux_snapshot_to_grid(
    snapshot: rmux_sdk::PaneSnapshot,
    cols: u16,
    rows: u16,
) -> tuiwright_core::SnapshotGrid {
    use tuiwright_core::snapshot::{Cell, CellStyle, Color};

    let cells = snapshot
        .cells
        .into_iter()
        .map(|c| {
            let map_color = |col: rmux_sdk::Color| match col {
                rmux_sdk::Color::Ansi(n) => Color::Ansi(n),
                rmux_sdk::Color::Indexed(n) => Color::Indexed(n),
                rmux_sdk::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
            };
            Cell {
                symbol: c.symbol,
                style: CellStyle {
                    fg: c.fg.map(map_color),
                    bg: c.bg.map(map_color),
                    bold: c.bold,
                    italic: c.italic,
                    underline: c.underline,
                    dim: c.dim,
                },
            }
        })
        .collect();

    tuiwright_core::SnapshotGrid { cols, rows, cells }
}
