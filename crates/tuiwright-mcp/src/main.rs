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
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--config" {
            match args.next() {
                Some(path) => return std::path::PathBuf::from(path),
                None => {
                    eprintln!("error: --config requires a path argument");
                    std::process::exit(1);
                }
            }
        }
    }
    std::path::PathBuf::from("tuiwright.toml")
}
