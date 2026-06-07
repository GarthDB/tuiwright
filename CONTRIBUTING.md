# Contributing to tuiwright

Thank you for your interest in contributing!

## Dev setup

**Prerequisites**

| Tool | Purpose | Install |
|------|---------|---------|
| Rust stable (≥ 1.88) | build | `rustup update stable` |
| [`freeze`](https://github.com/charmbracelet/freeze) | ANSI → PNG in tests | `brew install charmbracelet/tap/freeze` |
| [`rmux`](https://github.com/Helvesec/rmux) | live path tests only | see [Helvesec/rmux releases](https://github.com/Helvesec/rmux/releases) |

**Clone and build**

```bash
git clone https://github.com/GarthDB/tuiwright
cd tuiwright
cargo build --no-default-features   # headless only
cargo build                         # headless + live (requires rmux)
```

**Run tests**

```bash
cargo test --no-default-features    # CI-safe headless tests
cargo test --features live          # all tests (requires rmux daemon running)
```

**Lint and format** (CI enforces these)

```bash
cargo clippy --no-default-features -- -D warnings
cargo fmt --all
```

## Pull request workflow

1. Fork the repo and create a branch from `main`.
2. Make your changes and add tests.
3. Ensure `cargo fmt`, `cargo clippy`, and `cargo test --no-default-features` all pass.
4. Open a PR against `main`. The CI matrix runs headless tests, MSRV (1.88), and a security audit automatically.

## Feature flags

- `default = ["live"]` — builds the rmux live path.
- `--no-default-features` — headless-only; safe to run in CI without a running rmux daemon.

Any change that adds live-path code must remain behind `#[cfg(feature = "live")]` or a `live` dependency so the headless CI gate stays green.

## Adding a new MCP tool

1. Add the input struct (with `#[derive(Deserialize, JsonSchema)]`) and handler function in `crates/tuiwright-mcp/src/tools.rs`.
2. Register the handler in the `list_tools` / `call_tool` match arms.
3. Add at least one unit test (headless) and document the tool in the README tool reference table.

## Code of conduct

Be respectful. This project follows the [Contributor Covenant](https://www.contributor-covenant.org/version/2/1/code_of_conduct/).
