//! tuiwright-core — shared types and utilities.
//!
//! This crate is agnostic of MCP; it can be used standalone for scripting,
//! testing, or embedding in other tooling.

pub mod ansi;
pub mod config;
pub mod diff;
pub mod render;
pub mod snapshot;

pub use ansi::ansi_to_grid;
pub use config::Config;
pub use diff::{diff, CellDiff, GridDiff};
pub use snapshot::{Cell, CellStyle, CursorState, SnapshotGrid};
