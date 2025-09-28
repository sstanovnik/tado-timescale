Tado → TimescaleDB Ingestor
===========================

This binary ingests data from the Tado API into a TimescaleDB database, then you can visualise it via Grafana.

Key behaviour:
- Loads and writes all historical data on startup, then enters a realtime polling loop.
- Tado's historical day reports return placeholder data beyond roughly one year in the past (indoors fixed at 20 C / 50% and outdoor fields null); the backfill step skips these bogus readings before inserting into TimescaleDB.
- Historical backfill only requests days that contain measurement gaps of at least four hours (default, configurable via `BACKFILL_MIN_GAP_MINUTES`) and limits inserts to the missing intervals to avoid duplicating existing data.
- The realtime loop assumes the set of zones is static for the duration of the process: the mapping is cached at startup, and if a subsequent API call for a previously-known zone fails (for example because the zone was deleted), the binary logs an error and exits. Restart the process after adding or removing zones in Tado.

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
- Tokens rotate. Each refreshed token is written back to `TADO_REFRESH_TOKEN_PERSISTENCE_FILE`, and subsequent runs
  will load that file when present. Provide `INITIAL_TADO_REFRESH_TOKEN` only when seeding the persistence file for
  the first run or when rotating manually.

Configuration
-------------
Configuration can be supplied via environment variables or a `.env` file. The binary loads `./.env` by default
and you can override the location with `--env-file <path>`.

- `DATABASE_URL` (default `postgres://postgres:postgres@localhost:5432/tado`)
- `REALTIME_INTERVAL_SECS` (default `60`)
- `REALTIME_ENABLED` (default `true`) — skip the realtime polling loop when set to `false`.
- `MAX_REQUEST_RETRIES` (default `3`) — number of retry attempts after a 5xx response before surfacing the error.
- `BACKFILL_ENABLED` (default `true`)
- `BACKFILL_FROM_DATE` (optional) — limit historical backfill to start at this UTC date (format `YYYY-MM-DD`).
- `BACKFILL_REQUESTS_PER_SECOND` (optional) — throttle historical day-report calls to this maximum rate.
- `BACKFILL_SAMPLE_RATE` (optional) — sample historical day-report requests; expects the form `1/N` (e.g. `1/3`).
- `BACKFILL_MIN_GAP_MINUTES` (default `240`) — minimum gap length (in minutes) in stored climate data that triggers historical backfill for a day.
- `INITIAL_TADO_REFRESH_TOKEN` (required when the persistence file does not exist)
- `TADO_REFRESH_TOKEN_PERSISTENCE_FILE` (default `token.txt`)
- `TADO_FIREFOX_VERSION` (default `143.0`) — version string in the spoofed User-Agent.
- `FAKE_DATA_MODE` (default `false`) — when `true`, skips the Tado API and generates synthetic backfill data instead.

Build & Run
-----------
- Build: `cargo build`
- Docker build (BuildKit): `docker buildx build . --tag local/tado-timescale --load`
- Run: `INITIAL_TADO_REFRESH_TOKEN=... cargo run --`
- Release build: `cargo build --release`
- Lint: `cargo clippy --all-targets -- -D warnings`
- Format: `cargo fmt --all`

Fake Data Mode
--------------
Set `FAKE_DATA_MODE=true` to populate the database with synthetic history instead of contacting the Tado API. The run will:
- upsert a simulated home (`tado_home_id` `4201337`) with eight representative zones,
- generate five years of 15-minute climate and weather measurements with realistic seasonal and daily variation,
- write the rows through the same insertion path used by real backfill,
- exit after completion (no realtime loop or API calls).

Toggle the flag back to `false` before running against the real service.
