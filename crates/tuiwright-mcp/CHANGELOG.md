# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/GarthDB/tuiwright/releases/tag/tuiwright-mcp-v0.1.0) - 2026-06-07

### Added

- M6 — publish and distribute
- M4 — ANSI edge-case tests, error-path coverage, live integration test, CI macOS
- M3 — visual diff, baseline snapshots, and tui_assert
- fix live rmux-sdk bindings and implement real asciinema recording
- implement headless inner loop — SGR decode, fixture TUI, integration test
- initial scaffold of tuiwright MCP server

### Fixed

- handle freeze subprocess crash in render tests
- use realistic grid size in render tests to avoid freeze 0.2.2 SIGSEGV
- remove dead snap1_text binding in live integration test
- address M4 PR review — template extraction, CI fixes, test coverage
- check headless_snapshot.is_none() directly in tui_headless
- address M3 PR review — error semantics, fragile split, config default

### Other

- apply rustfmt to render test assertions
- harden CI and add branch protection gates
