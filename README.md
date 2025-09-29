Tado → TimescaleDB Ingestor
===========================

Keep every datapoint from your Tado smart heating setup in a queryable TimescaleDB warehouse ready for Grafana
dashboards.
The binary speaks directly to the Tado APIs, reconciles historical gaps, and keeps a realtime feed flowing without
needing
third-party cloud exports.

Highlights
----------

- Full historical capture with gap detection so you only request the days that really need filling.
- Realtime loop that mirrors the browser API and deduplicates measurements as they land in TimescaleDB hypertables.
- Browser-style OAuth flow using a refresh token, with automatic rotation and optional persistence to disk.
- Built-in Grafana provisioning and dashboards for climate, weather, and device health out of the box.
- Optional synthetic data generator for development, demos, or validating your Grafana wiring.

How It Works
------------

1. Read configuration from environment or `--env-file`, then authenticate to Tado with a Firefox-flavoured user agent.
2. Apply Diesel migrations and prepare TimescaleDB hypertables (`climate_measurements`, `weather_measurements`,
   `events`).
3. Sync reference data (homes, zones, devices, memberships) so foreign keys are satisfied for inserts.
4. Backfill historical day reports, skipping bogus Tado placeholders and only patching gaps above the configured
   threshold.
5. Stream fresh realtime measurements on a fixed cadence, caching the zone mapping gathered during startup.
6. Persist rotated refresh tokens to disk so future runs can resume without re-supplying the initial secret.

Quick Start
-----------

1. Install prerequisites: Rust toolchain, Docker + Compose v2, and `psql`.
2. Run `docker compose up -d db grafana` to launch TimescaleDB and Grafana locally.
3. Obtain a Tado refresh token (see *Authentication*) and place it in `token.txt` or export
   `INITIAL_TADO_REFRESH_TOKEN`.
4. (Optional) Create a `.env` file to pin environment variables; the binary loads `./.env` automatically when present.
5. Start the ingestor: `cargo run -- --env-file .env`.
6. Open Grafana at http://localhost:3000 (default admin/admin) and load the bundled dashboard to explore the data.

Authentication
--------------

- Tado retired the password grant, so the CLI uses the same OAuth browser client as app.tado.com.
- In a private/incognito window, log in to https://app.tado.com, then capture the `refresh_token` from the
  `/oauth2/token` network call.
- Provide that token either by creating `token.txt` (the default `TADO_REFRESH_TOKEN_PERSISTENCE_FILE`) or by exporting
  `INITIAL_TADO_REFRESH_TOKEN` for the first run. The binary rotates tokens automatically and rewrites the persistence
  file so subsequent launches can skip the env var.
- Avoid sharing the same token with your everyday browser session to prevent mutual invalidation.

Configuration Reference
-----------------------

| Variable                              | Default                                            | Description                                                         |
|---------------------------------------|----------------------------------------------------|---------------------------------------------------------------------|
| `DATABASE_URL`                        | `postgres://postgres:postgres@localhost:5432/tado` | TimescaleDB connection string.                                      |
| `REALTIME_INTERVAL_SECS`              | `60`                                               | Polling interval for the realtime loop.                             |
| `REALTIME_ENABLED`                    | `true`                                             | Skip the realtime loop when set to `false`.                         |
| `BACKFILL_ENABLED`                    | `true`                                             | Disable historical day-report backfill entirely.                    |
| `BACKFILL_FROM_DATE`                  | _unset_                                            | UTC date (`YYYY-MM-DD`) limiting how far back the backfill travels. |
| `BACKFILL_REQUESTS_PER_SECOND`        | _unset_                                            | Throttle day-report requests to this rate.                          |
| `BACKFILL_SAMPLE_RATE`                | _unset_                                            | Sample day reports using `1/N` syntax, e.g. `1/3`.                  |
| `BACKFILL_MIN_GAP_MINUTES`            | `240`                                              | Minimum gap length that qualifies for historical patching.          |
| `MAX_REQUEST_RETRIES`                 | `3`                                                | Retry budget for 5xx responses when calling Tado.                   |
| `TADO_FIREFOX_VERSION`                | `143.0`                                            | Firefox version advertised in the spoofed User-Agent.               |
| `TADO_REFRESH_TOKEN_PERSISTENCE_FILE` | `token.txt`                                        | Where the rotated refresh token is stored.                          |
| `INITIAL_TADO_REFRESH_TOKEN`          | _required once_                                    | Seed token used when the persistence file is missing.               |
| `FAKE_DATA_MODE`                      | `false`                                            | Generate synthetic data and skip the Tado API entirely.             |

Backfill Strategy & Data Quality
--------------------------------

- Tado day reports beyond ~1 year often contain placeholders (20°C / 50% humidity); those rows are filtered before
  inserts so TimescaleDB only holds genuine measurements.
- Gaps are detected per-zone using the existing TimescaleDB data; only days with ≥ `BACKFILL_MIN_GAP_MINUTES` of missing
  readings are requested, and only the missing intervals are written back.
- Weather backfill is home-scoped and aligned with the same window to maintain consistency across dashboards.

Operating Modes
---------------

- **Normal mode:** Talk to the live Tado API, perform historical catch-up, then enter the realtime loop.
- **Fake data mode:** Set `FAKE_DATA_MODE=true` to synthesize five years of 15-minute climate and weather data across
  eight example zones. Useful for demos or validating dashboards without real hardware. Toggle it off again before the
  next production run.

Grafana & Observability
-----------------------

- The repository ships with provisioning files under `grafana/` so a fresh Grafana instance comes preloaded with the
  Timescale datasource and an opinionated dashboard.
- Logs are emitted via `env_logger`; set `RUST_LOG=debug` (in `.env` or environment) for verbose tracing while
  diagnosing
  ingest issues.

Development & Maintenance
-------------------------

- Build (debug): `cargo build`
- Run (foreground): `cargo run -- --env-file .env`
- Lint: `cargo clippy --all-targets -- -D warnings`
- Format: `cargo fmt --all`
- Tests: `cargo test`
- Schema regen & hypertable check: `./generate-schema.sh`
- Bulk import past exports (CSV): `python3 transfer.py data-tado.csv`
