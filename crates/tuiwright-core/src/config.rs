//! Configuration types — mirrors `tuiwright.toml` schema.

use serde::{Deserialize, Serialize};

/// Default terminal width in columns when `tuiwright.toml` doesn't specify one.
pub const DEFAULT_COLS: u16 = 80;
/// Default terminal height in rows when `tuiwright.toml` doesn't specify one.
pub const DEFAULT_ROWS: u16 = 24;

/// Root configuration, read from `tuiwright.toml` in the project root (or
/// supplied via CLI flags to the MCP server).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// How to launch the TUI app in a live rmux pane.
    #[serde(default)]
    pub launch: LaunchConfig,

    /// Default terminal dimensions.
    #[serde(default)]
    pub size: SizeConfig,

    /// How to invoke the app's headless snapshot command.
    ///
    /// The command must accept an NDJSON path as its last argument and print
    /// styled ANSI output to stdout.  Example:
    ///   `headless_snapshot = "design-data --replay {} --snapshot-ansi"`
    ///
    /// `{}` is substituted with the NDJSON file path at runtime.
    pub headless_snapshot: Option<String>,

    /// Directory where baseline `.snap.json` files are stored.
    /// Defaults to `.tuiwright/baselines` relative to the working directory.
    #[serde(default = "default_baseline_dir")]
    pub baseline_dir: std::path::PathBuf,
}

/// How to launch the TUI app in a live rmux session.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LaunchConfig {
    /// Path to the TUI binary (or shell command).
    pub command: Option<String>,
    /// Arguments passed after the command.
    #[serde(default)]
    pub args: Vec<String>,
    /// Extra environment variables for the process.
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
}

/// Terminal dimensions used as defaults for headless snapshots and live sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SizeConfig {
    /// Terminal width in columns.
    pub cols: u16,
    /// Terminal height in rows.
    pub rows: u16,
}

impl Default for SizeConfig {
    fn default() -> Self {
        Self {
            cols: DEFAULT_COLS,
            rows: DEFAULT_ROWS,
        }
    }
}

fn default_baseline_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(".tuiwright/baselines")
}

impl Config {
    /// Load config from `tuiwright.toml`, falling back to defaults if absent.
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        if path.exists() {
            let raw = std::fs::read_to_string(path)?;
            Ok(toml::from_str(&raw)?)
        } else {
            Ok(Self::default())
        }
    }
}
