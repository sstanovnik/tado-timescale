Tado → TimescaleDB Ingestor
===========================

This binary ingests data from the Tado API into a TimescaleDB database, then you can visualise it via Grafana.

Key behaviour:
- Loads and writes all historical data on startup, then enters a realtime polling loop.

Authentication (Refresh Token Only)
-----------------------------------
Tado changed their OAuth flow; username/password and the old OAuth password grant no longer work.
This program now uses a browser-derived refresh token and rotates it in memory.

Provide the token via environment or a persistence file:
- `INITIAL_TADO_REFRESH_TOKEN` — a browser-derived refresh token used only when the persistence file is absent.
- `TADO_REFRESH_TOKEN_PERSISTENCE_FILE` (default `token.txt`) — path where rotated tokens are stored; place an existing token here to skip the env var after first run.

Incognito/Private browsing strongly recommended
- Obtain the refresh token in a private/incognito window so the CLI and your day-to-day browser do not share a token.
  This avoids one session invalidating the other when the token rotates.

How to obtain a refresh token
- Log in to https://app.tado.com in a private window and capture the refresh call.


Notes:
- The program mimics the browser’s headers for both token refresh and API calls (User-Agent version configurable).
- Tokens rotate. The app only stores the refreshed token in memory; it is not persisted. If you restart the binary, you must supply a valid refresh token again.

Configuration
-------------
Configuration can be supplied via environment variables or a `.env` file. The binary loads `./.env` by default
and you can override the location with `--env-file <path>`.

- `DATABASE_URL` (default `postgres://postgres:postgres@localhost:5432/tado`)
- `REALTIME_INTERVAL_SECS` (default `60`)
- `BACKFILL_ENABLED` (default `true`)
- `BACKFILL_FROM_DATE` (optional) — limit historical backfill to start at this UTC date (format `YYYY-MM-DD`).
- `INITIAL_TADO_REFRESH_TOKEN` (required when the persistence file does not exist)
- `TADO_REFRESH_TOKEN_PERSISTENCE_FILE` (default `token.txt`)
- `TADO_FIREFOX_VERSION` (default `143.0`) — version string in the spoofed User-Agent.

Build & Run
-----------
- Build: `cargo build`
- Run: `INITIAL_TADO_REFRESH_TOKEN=... cargo run --`
- Release build: `cargo build --release`
- Lint: `cargo clippy --all-targets -- -D warnings`
- Format: `cargo fmt --all`
