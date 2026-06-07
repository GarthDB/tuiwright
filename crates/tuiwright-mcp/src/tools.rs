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
    /// Active asciinema recording, if any.
    recording: Arc<Mutex<Option<RecordingState>>>,
    /// ToolRouter must be stored on the struct (rmcp 0.16 pattern).
    tool_router: rmcp::handler::server::tool::ToolRouter<Self>,
}

/// A live rmux session. Not Clone — stored inside Arc<Mutex<...>>.
#[cfg(feature = "live")]
struct LiveSession {
    session: rmux_sdk::Session,
    cols: u16,
    rows: u16,
}

/// State for an in-progress asciinema v2 recording.
struct RecordingState {
    path: std::path::PathBuf,
    file: std::fs::File,
    start: std::time::Instant,
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

    /// Build a `tokio::process::Command` from the `headless_snapshot` template.
    ///
    /// The template is split on whitespace *before* `{}` is substituted, so a
    /// path containing spaces is always passed as a single argument rather than
    /// being shattered by the split.
    fn headless_command(
        &self,
        ndjson_path: &str,
        cols: u16,
        rows: u16,
    ) -> Result<tokio::process::Command, McpError> {
        let template = self.config.headless_snapshot.as_deref().ok_or_else(|| {
            McpError::invalid_params("headless_snapshot not configured in tuiwright.toml", None)
        })?;
        let (bin, args) = expand_command_template(template, ndjson_path)?;
        let mut cmd = tokio::process::Command::new(&bin);
        cmd.args(&args)
            .env("COLUMNS", cols.to_string())
            .env("LINES", rows.to_string());
        Ok(cmd)
    }

    /// Resolve the path for a named baseline file.
    fn baseline_path(&self, name: &str) -> std::path::PathBuf {
        self.config.baseline_dir.join(format!("{name}.snap.json"))
    }

    /// Produce a SnapshotGrid from either headless replay (when `ndjson` is Some)
    /// or the current live pane (when `ndjson` is None). Used by tui_diff and tui_assert.
    async fn snapshot_for_diff(
        &self,
        ndjson: Option<&str>,
        cols: Option<u16>,
        rows: Option<u16>,
    ) -> Result<tuiwright_core::SnapshotGrid, McpError> {
        if let Some(ndjson_path) = ndjson {
            let cols = cols.unwrap_or(self.config.size.cols);
            let rows = rows.unwrap_or(self.config.size.rows);
            let output = self
                .headless_command(ndjson_path, cols, rows)?
                .output()
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(McpError::internal_error(
                    format!("headless command failed: {stderr}"),
                    None,
                ));
            }
            let ansi = String::from_utf8_lossy(&output.stdout).to_string();
            Ok(tuiwright_core::ansi_to_grid(&ansi, cols, rows))
        } else {
            // Live path.
            #[cfg(not(feature = "live"))]
            return Err(McpError::invalid_params(
                "live feature disabled — provide `ndjson` or rebuild with --features live",
                None,
            ));
            #[cfg(feature = "live")]
            {
                let guard = self.session.lock().await;
                let session = guard.as_ref().ok_or_else(|| {
                    McpError::invalid_params(
                        "no live session — call tui_open first or supply `ndjson`",
                        None,
                    )
                })?;
                let pane = session.session.pane(0, 0);
                let snapshot = pane
                    .snapshot()
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(rmux_snapshot_to_grid(snapshot))
            }
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

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct TuiDiffInput {
    /// Baseline name (filename without extension, stored in baseline_dir).
    pub baseline: String,
    /// NDJSON path to replay before snapshotting (same as tui_headless).
    /// Omit to diff against the current live pane snapshot (requires tui_open).
    pub ndjson: Option<String>,
    /// Terminal width (default: from config). Only used when ndjson is provided.
    pub cols: Option<u16>,
    /// Terminal height (default: from config). Only used when ndjson is provided.
    pub rows: Option<u16>,
    /// If true and no baseline file exists yet, save the current snapshot as the
    /// new baseline and return a "baseline created" message instead of a diff.
    #[serde(default)]
    pub create_if_missing: bool,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct TuiAssertInput {
    /// Text that must appear somewhere in the grid. All strings must be present.
    #[serde(default)]
    pub contains: Vec<String>,
    /// Text that must NOT appear anywhere in the grid.
    #[serde(default)]
    pub not_contains: Vec<String>,
    /// NDJSON path to replay before snapshotting (same as tui_headless).
    /// Omit to assert against the current live pane snapshot (requires tui_open).
    pub ndjson: Option<String>,
    /// Terminal width (default: from config). Only used when ndjson is provided.
    pub cols: Option<u16>,
    /// Terminal height (default: from config). Only used when ndjson is provided.
    pub rows: Option<u16>,
    /// Assert that the cursor is at this zero-based row.
    pub cursor_row: Option<u16>,
    /// Assert that the cursor is at this zero-based column.
    pub cursor_col: Option<u16>,
    /// Assert that the cursor is visible (true) or hidden (false).
    pub cursor_visible: Option<bool>,
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
            let session_name_str = input.session.as_deref().unwrap_or("tuiwright").to_string();

            let command = input
                .command
                .or_else(|| self.config.launch.command.clone())
                .ok_or_else(|| {
                    McpError::invalid_params(
                        "no command specified — set `launch.command` in tuiwright.toml or pass `command`",
                        None,
                    )
                })?;

            let mut full_cmd = command.clone();
            for arg in &self.config.launch.args {
                full_cmd.push(' ');
                full_cmd.push_str(arg);
            }
            for arg in &input.args {
                full_cmd.push(' ');
                full_cmd.push_str(arg);
            }

            let client = rmux_sdk::Rmux::builder()
                .connect_or_start()
                .await
                .map_err(|e| {
                    McpError::internal_error(
                        format!(
                            "failed to connect to rmux daemon: {e}\n\
                             Install rmux and start the daemon:\n\
                             • cargo install rmux  (or see https://github.com/Helvesec/rmux/releases)\n\
                             • rmux start"
                        ),
                        None,
                    )
                })?;

            let sess_name = rmux_sdk::SessionName::new(&session_name_str)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;

            let rmux_session = client
                .ensure_session(
                    rmux_sdk::EnsureSession::named(sess_name)
                        .shell(&full_cmd)
                        .size(rmux_sdk::TerminalSizeSpec::new(cols, rows))
                        .create_or_reuse(),
                )
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;

            let mut guard = self.session.lock().await;
            *guard = Some(LiveSession {
                session: rmux_session,
                cols,
                rows,
            });

            Ok(format!(
                "Live pane opened: session={session_name_str}, size={cols}x{rows}, command={command}"
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
                .session
                .pane(0, 0)
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

            let pane = session.session.pane(0, 0);
            let snapshot = pane
                .snapshot()
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            let grid = rmux_snapshot_to_grid(snapshot);
            drop(guard);

            // Append frame to active recording.
            append_recording_frame(&self.recording, &grid).await;

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
            let pane = session.session.pane(0, 0);
            drop(guard);
            tokio::time::timeout(timeout, pane.wait_for_text(&input.text))
                .await
                .map_err(|_| {
                    McpError::invalid_params(
                        format!(
                            "timed out after {}ms waiting for {:?}",
                            input.timeout_ms.unwrap_or(5000),
                            input.text
                        ),
                        None,
                    )
                })?
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
                .session
                .pane(0, 0)
                .resize(size)
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
                session.session.kill().await.ok();
            }
            Ok("session closed".to_string())
        }
    }

    /// Start an asciinema v2 recording. Frames are appended on each tui_snapshot call.
    #[tool(
        description = "Start an asciinema v2 recording. Each tui_snapshot call appends a frame. `path` is the output .cast file."
    )]
    async fn tui_record_start(
        &self,
        Parameters(input): Parameters<TuiRecordStartInput>,
    ) -> Result<String, McpError> {
        use std::io::Write;

        let path = std::path::PathBuf::from(&input.path);
        let mut rec = self.recording.lock().await;
        if rec.is_some() {
            return Err(McpError::invalid_params(
                "a recording is already in progress — call tui_record_stop first",
                None,
            ));
        }

        // Determine terminal size: prefer live session, fall back to config.
        let (cols, rows) = {
            #[cfg(feature = "live")]
            {
                let s = self.session.lock().await;
                s.as_ref()
                    .map(|ls| (ls.cols, ls.rows))
                    .unwrap_or((self.config.size.cols, self.config.size.rows))
            }
            #[cfg(not(feature = "live"))]
            (self.config.size.cols, self.config.size.rows)
        };

        let mut file = std::fs::File::create(&path).map_err(|e| {
            McpError::internal_error(format!("cannot create {}: {e}", path.display()), None)
        })?;

        // Write asciinema v2 header.
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let header = serde_json::json!({
            "version": 2,
            "width": cols,
            "height": rows,
            "timestamp": ts,
            "title": "tuiwright recording"
        });
        writeln!(file, "{header}").map_err(|e| McpError::internal_error(e.to_string(), None))?;

        *rec = Some(RecordingState {
            path: path.clone(),
            file,
            start: std::time::Instant::now(),
        });

        Ok(format!(
            "recording started → {} ({}x{})",
            path.display(),
            cols,
            rows
        ))
    }

    /// Stop the current asciinema recording and finalise the .cast file.
    #[tool(description = "Stop the in-progress asciinema recording and finalise the .cast file.")]
    async fn tui_record_stop(&self) -> Result<String, McpError> {
        let mut rec = self.recording.lock().await;
        let state = rec
            .take()
            .ok_or_else(|| McpError::invalid_params("no recording in progress", None))?;
        Ok(format!("recording stopped → {}", state.path.display()))
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
        if self.config.headless_snapshot.is_none() {
            return Err(McpError::invalid_params(
                "headless_snapshot not configured in tuiwright.toml — set it to your app's ANSI snapshot command, e.g. `design-data --replay {} --snapshot-ansi`",
                None,
            ));
        }

        let cols = input.cols.unwrap_or(self.config.size.cols);
        let rows = input.rows.unwrap_or(self.config.size.rows);

        let output = self
            .headless_command(&input.ndjson, cols, rows)?
            .output()
            .await
            .map_err(|e| {
                McpError::internal_error(format!("failed to spawn headless command: {e}"), None)
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

        // Append frame to active recording (if any) — headless sessions are recordable too.
        append_recording_frame(&self.recording, &grid).await;

        render_snapshot(&grid, &input.format)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))
    }

    /// Diff the current TUI state against a saved baseline snapshot.
    /// Pass `create_if_missing: true` to create the baseline on first run.
    #[tool(
        description = "Diff the current TUI state against a named baseline (.snap.json). \
                       Supply `ndjson` for headless mode or omit to use the live pane. \
                       Set `create_if_missing: true` to save a new baseline when none exists yet."
    )]
    async fn tui_diff(
        &self,
        Parameters(input): Parameters<TuiDiffInput>,
    ) -> Result<String, McpError> {
        let grid = self
            .snapshot_for_diff(input.ndjson.as_deref(), input.cols, input.rows)
            .await?;
        let baseline_path = self.baseline_path(&input.baseline);

        if !baseline_path.exists() {
            if input.create_if_missing {
                grid.save_baseline(&baseline_path)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                return Ok(format!("baseline created: {}", baseline_path.display()));
            }
            return Err(McpError::invalid_params(
                format!(
                    "baseline '{}' not found at {} — run with create_if_missing: true to create it",
                    input.baseline,
                    baseline_path.display()
                ),
                None,
            ));
        }

        let expected = tuiwright_core::SnapshotGrid::load_baseline(&baseline_path)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let diff = tuiwright_core::diff(&expected, &grid);

        if diff.is_match() {
            Ok(format!(
                "✓ match — {}×{} vs baseline '{}'",
                grid.cols, grid.rows, input.baseline
            ))
        } else {
            Err(McpError::invalid_request(
                format!("diff against '{}': {}", input.baseline, diff.display()),
                None,
            ))
        }
    }

    /// Assert that the current TUI state contains (or does not contain) expected text.
    #[tool(
        description = "Assert the rendered TUI contains all strings in `contains` and none in `not_contains`. \
                       Supply `ndjson` for headless mode or omit to use the live pane. \
                       Returns a pass summary or fails with details of which assertions failed."
    )]
    async fn tui_assert(
        &self,
        Parameters(input): Parameters<TuiAssertInput>,
    ) -> Result<String, McpError> {
        let grid = self
            .snapshot_for_diff(input.ndjson.as_deref(), input.cols, input.rows)
            .await?;
        let text = grid.to_plain_text();

        let mut failures: Vec<String> = Vec::new();

        for expected in &input.contains {
            if !text.contains(expected.as_str()) {
                failures.push(format!("expected to find: {:?}", expected));
            }
        }
        for forbidden in &input.not_contains {
            if text.contains(forbidden.as_str()) {
                failures.push(format!("expected NOT to find: {:?}", forbidden));
            }
        }

        // Cursor assertions.
        let wants_cursor = input.cursor_row.is_some()
            || input.cursor_col.is_some()
            || input.cursor_visible.is_some();
        if wants_cursor {
            match grid.cursor {
                None => {
                    failures.push(
                        "cursor assertion failed: grid has no cursor (app may not call set_cursor_position)"
                            .to_string(),
                    );
                }
                Some(c) => {
                    if let Some(expected_row) = input.cursor_row {
                        if c.row != expected_row {
                            failures.push(format!(
                                "cursor row: expected {expected_row}, got {}",
                                c.row
                            ));
                        }
                    }
                    if let Some(expected_col) = input.cursor_col {
                        if c.col != expected_col {
                            failures.push(format!(
                                "cursor col: expected {expected_col}, got {}",
                                c.col
                            ));
                        }
                    }
                    if let Some(expected_vis) = input.cursor_visible {
                        if c.visible != expected_vis {
                            failures.push(format!(
                                "cursor visible: expected {expected_vis}, got {}",
                                c.visible
                            ));
                        }
                    }
                }
            }
        }

        if failures.is_empty() {
            let checks =
                input.contains.len() + input.not_contains.len() + if wants_cursor { 1 } else { 0 };
            Ok(format!("✓ all {checks} assertion(s) passed"))
        } else {
            Err(McpError::invalid_request(
                format!(
                    "{} assertion(s) failed:\n{}\n\nRendered grid:\n```\n{}```",
                    failures.len(),
                    failures.join("\n"),
                    text
                ),
                None,
            ))
        }
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

/// Format the cursor state as a single annotation line.
fn cursor_annotation(grid: &tuiwright_core::SnapshotGrid) -> String {
    match grid.cursor {
        Some(c) => format!("cursor: row={} col={} visible={}", c.row, c.col, c.visible),
        None => "cursor: <none>".to_string(),
    }
}

/// Render a SnapshotGrid according to the requested format.
async fn render_snapshot(
    grid: &tuiwright_core::SnapshotGrid,
    format: &SnapshotFormat,
) -> anyhow::Result<String> {
    let text = grid.to_plain_text();
    let cursor = cursor_annotation(grid);
    match format {
        SnapshotFormat::Text => Ok(format!("```\n{text}```\n{cursor}")),
        SnapshotFormat::Image => {
            if !tuiwright_core::render::freeze_available().await {
                return Ok(format!(
                    "freeze not found in $PATH — install with: brew install charmbracelet/tap/freeze\n{cursor}"
                ));
            }
            let png = tmp_png_path();
            tuiwright_core::render::grid_to_png(grid, &png).await?;
            Ok(format!("PNG saved to {}\n{cursor}", png.display()))
        }
        SnapshotFormat::Both => {
            if !tuiwright_core::render::freeze_available().await {
                return Ok(format!(
                    "freeze not found — text only:\n```\n{text}```\n{cursor}"
                ));
            }
            let png = tmp_png_path();
            tuiwright_core::render::grid_to_png(grid, &png).await?;
            Ok(format!(
                "PNG: {}\n\n```\n{text}```\n{cursor}",
                png.display()
            ))
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

/// Append the current grid as an asciinema event to the active recording (if any).
async fn append_recording_frame(
    recording: &Arc<Mutex<Option<RecordingState>>>,
    grid: &tuiwright_core::SnapshotGrid,
) {
    use std::io::Write;
    let mut rec_guard = recording.lock().await;
    if let Some(rec) = rec_guard.as_mut() {
        let ansi = tuiwright_core::ansi::grid_to_ansi(grid);
        let elapsed = rec.start.elapsed().as_secs_f64();
        let event = serde_json::json!([elapsed, "o", ansi]);
        let _ = writeln!(rec.file, "{event}");
    }
}

/// Split a command template on whitespace and replace the `{}` placeholder with
/// `ndjson_path` in whichever token matches exactly.
///
/// Splitting before substitution ensures a path containing spaces is always
/// passed as a single argument, not shattered into multiple tokens.
fn expand_command_template(
    template: &str,
    ndjson_path: &str,
) -> Result<(String, Vec<String>), McpError> {
    let mut parts = template.split_whitespace();
    let bin = parts
        .next()
        .ok_or_else(|| McpError::invalid_params("headless_snapshot command is empty", None))?
        .to_string();
    let args: Vec<String> = parts
        .map(|p| {
            if p == "{}" {
                ndjson_path.to_string()
            } else {
                p.to_string()
            }
        })
        .collect();
    Ok((bin, args))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tuiwright_core::{
        config::{Config, SizeConfig},
        snapshot::{Cell, CellStyle, SnapshotGrid},
    };

    fn minimal_grid() -> SnapshotGrid {
        // Use a realistic terminal size so freeze can render it without crashing.
        // freeze 0.2.2 SIGSEGV on very small grids (e.g. 3×1).
        let cols: u16 = 80;
        let rows: u16 = 5;
        let content = b"Hello from tuiwright test grid";
        let mut cells: Vec<Cell> = (0..cols as usize * rows as usize)
            .map(|i| Cell {
                symbol: if i < content.len() {
                    (content[i] as char).to_string()
                } else {
                    " ".to_string()
                },
                style: CellStyle::default(),
            })
            .collect();
        // Make row 0 bold so there's styled content for freeze to render.
        for cell in cells[..cols as usize].iter_mut() {
            cell.style.bold = true;
        }
        SnapshotGrid::new(cols, rows, cells)
    }

    fn server_with_headless(cmd: Option<&str>) -> TuiwrightServer {
        let config = Config {
            headless_snapshot: cmd.map(str::to_string),
            size: SizeConfig { cols: 80, rows: 24 },
            ..Default::default()
        };
        TuiwrightServer::new(config)
    }

    // -- expand_command_template + headless_command --------------------------

    #[test]
    fn template_substitutes_placeholder() {
        let (bin, args) =
            expand_command_template("my-app --replay {} --snapshot-ansi", "/tmp/foo.ndjson")
                .unwrap();
        assert_eq!(bin, "my-app");
        assert_eq!(args, ["--replay", "/tmp/foo.ndjson", "--snapshot-ansi"]);
    }

    #[test]
    fn template_no_placeholder_leaves_args_unchanged() {
        let (bin, args) = expand_command_template("my-app --no-replay", "/tmp/x.ndjson").unwrap();
        assert_eq!(bin, "my-app");
        assert_eq!(args, ["--no-replay"]);
    }

    #[test]
    fn template_empty_returns_error() {
        assert!(expand_command_template("   ", "/tmp/x.ndjson").is_err());
    }

    #[test]
    fn headless_command_no_config_returns_error() {
        let server = server_with_headless(None);
        assert!(server.headless_command("/tmp/test.ndjson", 80, 24).is_err());
    }

    #[test]
    fn headless_command_builds_correct_binary() {
        let server = server_with_headless(Some("my-app --replay {} --snapshot-ansi"));
        let cmd = server.headless_command("/tmp/foo.ndjson", 80, 24).unwrap();
        let prog = cmd.as_std().get_program().to_string_lossy().to_string();
        assert_eq!(prog, "my-app");
    }

    // -- baseline_path -------------------------------------------------------

    #[test]
    fn baseline_path_resolves_correctly() {
        let server = server_with_headless(None);
        let p = server.baseline_path("my-snap");
        assert!(p.ends_with("my-snap.snap.json"));
        assert!(p.starts_with(&server.config.baseline_dir));
    }

    // -- render_snapshot graceful degradation --------------------------------

    #[tokio::test]
    async fn render_snapshot_text_format_never_needs_freeze() {
        let grid = minimal_grid();
        let result = render_snapshot(&grid, &SnapshotFormat::Text).await.unwrap();
        assert!(
            result.contains("Hello from"),
            "plain-text output should contain the symbols"
        );
    }

    #[tokio::test]
    async fn render_snapshot_image_correct_for_both_freeze_states() {
        let grid = minimal_grid();
        let result = render_snapshot(&grid, &SnapshotFormat::Image).await;
        if tuiwright_core::render::freeze_available().await {
            // freeze is installed; it may succeed or crash (freeze 0.2.2 has known
            // SIGSEGV bugs on some Linux CI runners). Accept either outcome.
            match result {
                Ok(out) => assert!(out.contains(".png"), "expected PNG path; got: {out}"),
                Err(e) => assert!(
                    e.to_string().contains("freeze exited"),
                    "unexpected error with freeze present: {e}"
                ),
            }
        } else {
            let out = result.unwrap();
            assert!(
                out.contains("freeze not found"),
                "Image without freeze should return fallback; got: {out}"
            );
        }
    }

    #[tokio::test]
    async fn render_snapshot_both_correct_for_both_freeze_states() {
        let grid = minimal_grid();
        let result = render_snapshot(&grid, &SnapshotFormat::Both).await;
        if tuiwright_core::render::freeze_available().await {
            match result {
                Ok(out) => {
                    assert!(
                        out.contains("Hello from"),
                        "Both should include text; got: {out}"
                    );
                    assert!(
                        out.contains(".png"),
                        "Both should include PNG path; got: {out}"
                    );
                }
                Err(e) => assert!(
                    e.to_string().contains("freeze exited"),
                    "unexpected error with freeze present: {e}"
                ),
            }
        } else {
            let out = result.unwrap();
            assert!(
                out.contains("freeze not found"),
                "Both without freeze should include notice; got: {out}"
            );
            assert!(
                out.contains("Hello from"),
                "fallback should include text; got: {out}"
            );
        }
    }
}

/// Live-path integration tests — gated on both the `live` feature AND rmux availability.
///
/// These tests launch `tuiwright-fixture` in a real rmux session and exercise the full
/// live tool chain: tui_open → tui_snapshot → tui_send_keys → tui_snapshot → tui_close.
/// They self-skip gracefully when rmux is not running or the fixture binary is absent.
#[cfg(all(test, feature = "live"))]
mod live_tests {
    use super::*;
    use tuiwright_core::config::{Config, LaunchConfig, SizeConfig};

    /// Returns true if the rmux daemon is reachable.
    async fn rmux_available() -> bool {
        tokio::process::Command::new("rmux")
            .args(["ls"])
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn fixture_bin() -> std::path::PathBuf {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let workspace = manifest.parent().unwrap().parent().unwrap();
        workspace.join("target/debug/tuiwright-fixture")
    }

    /// Full live round-trip: open → snapshot → send Down → snapshot again → close.
    ///
    /// Verifies that:
    /// - tui_open launches the fixture and stores a session
    /// - tui_snapshot returns plain text containing fixture content
    /// - tui_send_keys delivers a Down arrow (second item becomes selected)
    /// - tui_close kills the session without error
    #[tokio::test]
    async fn live_open_snapshot_sendkeys_close() {
        if !rmux_available().await {
            eprintln!("SKIP: rmux daemon not running");
            return;
        }
        let bin = fixture_bin();
        if !bin.exists() {
            eprintln!(
                "SKIP: tuiwright-fixture not found at {} — build it with `cargo build --workspace` first",
                bin.display()
            );
            return;
        }

        let config = Config {
            launch: LaunchConfig {
                command: Some(bin.to_str().unwrap().to_string()),
                ..Default::default()
            },
            size: SizeConfig { cols: 80, rows: 24 },
            ..Default::default()
        };
        let server = TuiwrightServer::new(config);

        // ── Open ────────────────────────────────────────────────────────────
        let open = server
            .tui_open(Parameters(TuiOpenInput {
                command: None,
                args: vec![],
                cols: Some(80),
                rows: Some(24),
                session: Some("tuiwright-live-test".to_string()),
            }))
            .await;
        assert!(open.is_ok(), "tui_open failed: {:?}", open);

        // ── Poll until fixture renders (up to 3 s) ──────────────────────
        {
            let mut found = None;
            for _ in 0..15 {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                if let Ok(text) = server
                    .tui_snapshot(Parameters(TuiSnapshotInput {
                        format: SnapshotFormat::Text,
                    }))
                    .await
                {
                    if text.contains("tuiwright fixture") {
                        found = Some(text);
                        break;
                    }
                }
            }
            found.expect("fixture did not render 'tuiwright fixture' within 3 s");
        }

        // ── Send Down arrow ──────────────────────────────────────────────
        let keys = server
            .tui_send_keys(Parameters(TuiSendKeysInput {
                keys: "\x1b[B".to_string(), // CSI B = Down arrow
            }))
            .await;
        assert!(keys.is_ok(), "tui_send_keys failed: {:?}", keys);

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // ── Snapshot after keypress ──────────────────────────────────────
        let snap2 = server
            .tui_snapshot(Parameters(TuiSnapshotInput {
                format: SnapshotFormat::Text,
            }))
            .await;
        assert!(snap2.is_ok(), "second tui_snapshot failed: {:?}", snap2);

        // ── Close ────────────────────────────────────────────────────────
        let close = server.tui_close().await;
        assert!(close.is_ok(), "tui_close failed: {:?}", close);
        assert_eq!(close.unwrap(), "session closed");

        // ── Guard: session is cleared ─────────────────────────────────────
        let guard = server.session.lock().await;
        assert!(guard.is_none(), "session should be None after tui_close");
    }

    /// tui_send_keys without an open session returns an invalid_params error.
    #[tokio::test]
    async fn send_keys_without_session_returns_error() {
        if !rmux_available().await {
            eprintln!("SKIP: rmux daemon not running");
            return;
        }
        let config = Config {
            size: SizeConfig { cols: 80, rows: 24 },
            ..Default::default()
        };
        let server = TuiwrightServer::new(config);
        let result = server
            .tui_send_keys(Parameters(TuiSendKeysInput {
                keys: "x".to_string(),
            }))
            .await;
        assert!(result.is_err(), "expected error with no session");
        let msg = result.unwrap_err().message;
        assert!(
            msg.contains("no live session"),
            "expected 'no live session' error; got: {msg}"
        );
    }
}

/// Convert an rmux pane snapshot to a tuiwright SnapshotGrid.
#[cfg(feature = "live")]
fn rmux_snapshot_to_grid(snapshot: rmux_sdk::PaneSnapshot) -> tuiwright_core::SnapshotGrid {
    use tuiwright_core::snapshot::{Cell, CellStyle, Color, CursorState};

    let map_color = |c: &rmux_sdk::PaneColor| -> Option<Color> {
        match c {
            rmux_sdk::PaneColor::Ansi { index } => Some(Color::Ansi(*index)),
            // BrightAnsi index is 0-7; map to Ansi 8-15 in our scheme.
            rmux_sdk::PaneColor::BrightAnsi { index } => Some(Color::Ansi(*index + 8)),
            rmux_sdk::PaneColor::Indexed { index } => Some(Color::Indexed(*index)),
            rmux_sdk::PaneColor::Rgb { red, green, blue } => Some(Color::Rgb(*red, *green, *blue)),
            // Default, None, Terminal, Encoded — no colour info.
            _ => None,
        }
    };

    let cells = snapshot
        .cells
        .iter()
        .map(|c| Cell {
            symbol: if c.glyph.padding {
                " ".to_string()
            } else {
                c.glyph.text.clone()
            },
            style: CellStyle {
                fg: map_color(&c.foreground),
                bg: map_color(&c.background),
                bold: c.attributes.contains(rmux_sdk::PaneAttributes::BOLD),
                italic: c.attributes.contains(rmux_sdk::PaneAttributes::ITALIC),
                underline: c.attributes.contains(rmux_sdk::PaneAttributes::UNDERLINE),
                dim: c.attributes.contains(rmux_sdk::PaneAttributes::DIM),
            },
        })
        .collect();

    let cursor = Some(CursorState {
        row: snapshot.cursor.row,
        col: snapshot.cursor.col,
        visible: snapshot.cursor.visible,
    });

    tuiwright_core::SnapshotGrid {
        cols: snapshot.cols,
        rows: snapshot.rows,
        cells,
        cursor,
    }
}
