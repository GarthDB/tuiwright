//! Rendering helpers: ANSI grid → PNG via `freeze`, ANSI grid → GIF via `agg`.

use crate::ansi::grid_to_ansi;
use crate::snapshot::SnapshotGrid;
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Stdio;

/// Render a [`SnapshotGrid`] to a PNG file by piping its ANSI representation
/// to `freeze`.
///
/// Requires `freeze` to be in `$PATH`.  Install via:
///   `brew install charmbracelet/tap/freeze`   (macOS)
///   `go install github.com/charmbracelet/freeze@latest`
///
/// `output` should end in `.png`, `.svg`, or `.webp`.
pub async fn grid_to_png(grid: &SnapshotGrid, output: &Path) -> Result<()> {
    let ansi = grid_to_ansi(grid);

    let height = grid.rows as u32 * 18; // approximate: ~18px per row at default font size

    let mut child = tokio::process::Command::new("freeze")
        .args([
            "--language",
            "ansi",
            "--output",
            output.to_str().context("output path is not valid UTF-8")?,
            "--height",
            &height.to_string(),
            "--show-line-numbers=false",
            "--window=false",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .context("failed to spawn `freeze` — is it installed and in $PATH?")?;

    // Write ANSI to stdin.
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        stdin
            .write_all(ansi.as_bytes())
            .await
            .context("failed to write to freeze stdin")?;
    }

    let status = child.wait().await.context("freeze did not exit cleanly")?;
    if !status.success() {
        anyhow::bail!("freeze exited with status {status}");
    }
    Ok(())
}

/// Convert an asciinema `.cast` file to a GIF via `agg`.
///
/// Requires `agg` to be in `$PATH`.  Install via:
///   `cargo install --git https://github.com/asciinema/agg`
///   or `brew install agg` (when available)
///
/// Returns `Ok(())` on success.
pub async fn cast_to_gif(cast_path: &Path, gif_path: &Path) -> Result<()> {
    let status = tokio::process::Command::new("agg")
        .arg(cast_path)
        .arg(gif_path)
        .status()
        .await
        .context("failed to spawn `agg` — is it installed and in $PATH?")?;

    if !status.success() {
        anyhow::bail!("agg exited with status {status}");
    }
    Ok(())
}

/// Check whether `freeze` is available in $PATH.
pub async fn freeze_available() -> bool {
    tokio::process::Command::new("freeze")
        .arg("--version")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check whether `agg` is available in $PATH.
pub async fn agg_available() -> bool {
    tokio::process::Command::new("agg")
        .arg("--version")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}
