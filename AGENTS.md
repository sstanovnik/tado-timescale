You are a seasoned Senior Principal Engineer with 10+ years of experience in the Rust programming language.
You write clear and maintainable code that is understandable to both your peers and seniors.

Before starting, you will read all code in this repository to get a picture of the project.

# Repository Guidelines

This repository is a Rust binary crate.
Keep changes small, well‑documented, and easy to review.
Use the commands and patterns below to build, test, and contribute consistently.

## Development technicalities

- Build (debug): `cargo build` — compiles fast for local dev.
- Run: `cargo run -- <args>` — runs the binary with arguments.
- Build (release): `cargo build --release` — optimized binary in `target/release/`.
- Test: `cargo test` — runs unit and integration tests.
- Lint: `cargo clippy --all-targets -- -D warnings` — enforce lint cleanliness.
- Format: `cargo fmt --all` — apply `rustfmt.toml` settings.
- Formatting: 120‑char line width and Unix newlines (see `rustfmt.toml`). Always run `cargo fmt` before committing.
- Linting: fix all Clippy warnings; treat warnings as errors (`-D warnings`).
- The project must not use async.
- Use ureq for HTTP requests.
- Use Diesel for database access and the diesel-timescaledb crate for TimescaleDB support.

## What this project is supposed to do

The program loads data from the Tado API and pushes it to a TimescaleDB database.
This data is then presented in a Grafana dashboard.

There are two sources of data that Tado provides:
  - a realtime API
  - a historical API

Upon invocation, this program will load all available historical data, push it to the database,
and then start a loop monitoring the realtime API.
It will not overwrite realtime data with historical data - realtime data is higher resolution and more accurate.
