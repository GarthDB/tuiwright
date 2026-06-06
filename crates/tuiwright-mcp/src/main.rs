//! tuiwright — Playwright-style MCP server for developing TUIs with Claude.
//!
//! Usage (stdio, registered in .claude/settings.json):
//!   `tuiwright [--config tuiwright.toml]`

mod tools;

use anyhow::Result;
use rmcp::ServiceExt;
use tools::TuiwrightServer;

#[tokio::main]
async fn main() -> Result<()> {
    // Resolve config path (default: tuiwright.toml in cwd).
    let config_path = std::path::PathBuf::from("tuiwright.toml");
    let config = tuiwright_core::Config::load(&config_path)?;

    let server = TuiwrightServer::new(config);

    // Run as a stdio MCP server (Claude Code default transport).
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}
