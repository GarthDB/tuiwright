//! Configuration types — mirrors `tuiwright.toml` schema.

use serde::{Deserialize, Serialize};

/// Default terminal size used when `tuiwright.toml` doesn't specify one.
pub const DEFAULT_COLS: u16 = 80;
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
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SizeConfig {
    pub cols: u16,
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
