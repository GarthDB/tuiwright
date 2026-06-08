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
    // Resolve config path: --config <path> or default tuiwright.toml in cwd.
    let config_path = parse_config_path();
    let config = tuiwright_core::Config::load(&config_path)?;

    let server = TuiwrightServer::new(config);

    // Run as a stdio MCP server (Claude Code default transport).
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}

fn parse_config_path() -> std::path::PathBuf {
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--config" {
            if let Some(path) = args.get(i + 1) {
                return std::path::PathBuf::from(path);
            }
        }
        i += 1;
    }
    std::path::PathBuf::from("tuiwright.toml")
}
