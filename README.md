Tado → TimescaleDB Ingestor
===========================

Make your Tado data your own by slurping it all into TimescaleDB.
Stop relying on anemic Tado reporting and instead _just use Grafana_.

Highlights
----------

- Full historical capture with backfill gap detection so you only request the days that really need filling.
- Realtime loop that captures high-resolution data in real time.
- Uses a (manual) browser OAuth flow using a refresh token, with automatic rotation and persistence to disk.
- A Grafana dashboard for climate, weather, and device health out of the box.
- Optional synthetic data generator to play around with.

Quick Start with Docker Compose
-------------------------------
1. Obtain a Tado refresh token (see **Authentication**) and create an `.env` with at least:
   ```shell
   INITIAL_TADO_REFRESH_TOKEN=put-your-refresh-token-here
   ```
   For other settings, see `.env.example` and the **Configuration Reference**).
2. Start `tado-timescale`, TimescaleDB, and Grafana:
   ```shell
   docker compose up -d
   ```
3. Open Grafana at http://localhost:3000 (default user/pass is admin/admin) and look at the bundled dashboard.
   Depending on how much data you have, it may take a few minutes for the data to be loaded into the database.
   After historical data is loaded, the realtime loop will start.

Authentication
--------------

- Tado [retired the password grant](https://github.com/home-assistant/core/issues/151223),
  so `tado-timescale` uses the same OAuth browser authentication flow as the official Tado browser app.
- This flow is unfortunately manual---at first. Once you have a refresh token, `tado-timescale` will automatically
  rotate it and persist it to disk.
- How to obtain a refresh token:
  - Open a new Private-mode window.
  - Open the Developer Tools (F12) and click on the Network tab.
  - Optionally, type in "token" in the top search bar so you only see the relevant request.
  - Go to https://app.tado.com and log in.
  - You will see a request to `https://auth.tado.com/oauth2/token`.
  - Click on the request and use the Developer Tools to inspect the response - the "Body" or "Response" tab.
  - Copy the `refresh_token`'s value from the response.
  - **IMPORTANT: Close the Private-mode window.**
    This prevents your browser from storing, and automatically refreshing, the refresh token.
    When this happens, the previous refresh token will be invalidated, and you will have to start over.

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

- Tado day reports beyond ~1 year contain placeholders (20°C / 50% humidity); those rows are filtered before
  inserts so the database only holds genuine measurements.
- Gaps are detected per-zone using the existing TimescaleDB data; only days with ≥ `BACKFILL_MIN_GAP_MINUTES` of missing
  readings are requested, and only the missing intervals are written back.

Operating Modes
---------------

- **Normal mode:** Talk to the live Tado API, perform historical catch-up, then enter the realtime loop.
- **Fake data mode:** Set `FAKE_DATA_MODE=true` to synthesize five years of 15-minute climate and weather data across
  eight example zones. Useful for demos or validating dashboards without real hardware.

Developer Setup & Maintenance
-----------------------------
This section targets contributors running the project with a local Rust toolchain.

**Host-based workflow:**
1. Install the Rust toolchain (`rustup`), Docker + Compose v2, and `psql`.
2. Start supporting services: `docker compose up -d db grafana`.
3. Capture a refresh token and either save it to `token.txt` or export `INITIAL_TADO_REFRESH_TOKEN`.
4. Create `.env` from `.env.template` and modify as needed.
5. Execute the binary via Cargo: `cargo run`.
6. Visit Grafana at http://localhost:3000 (admin/admin) to verify data appears.

**Day-to-day commands:**
- Build (debug): `cargo build`
- Run (foreground): `cargo run -- --env-file .env`
- Lint: `cargo clippy --all-targets -- -D warnings`
- Format: `cargo fmt --all`
- Tests: `cargo test`
- Schema regen & hypertable check: `./generate-schema.sh`
- Bulk import past exports: `python3 transfer.py data-tado.csv`

A note you may or may not like.
-------------------------------
About 99% of this repository is LLM-generated.
Sorry.

I do swear that I have reviewed every single line of code, although not in-depth.
I am not a Rust expert, but I do kind-of-sort-of know what I'm doing.

It's absolutely amazing how far this takes you---but it's not, absolutely not, a replacement for a developer.
It has definitely up the development, and, most importantly, allowed me to _~~fucking finish a fucking project for
fucks sake why cant i ever finish what i start~~_ power through times of lacking willpower and focus.
But if I were not at the level I am now, or hadn't known how Rust, Grafana, or the libraries I chose (yes, that was me)
work, the LLM would just spew trash around and I wouldn't have known---and nothing would have worked.

Perhaps most impressively: the initial Grafana dashboard was entirely one-shot generated by the LLM.
I absolutely couldn't believe that it actually worked, both syntax-wise and the fact that the visualisations were
actually useful.
Of course, when you look closely, the dashboard has quite a few rough edges, butt-ugly SQL which is inefficient to boot,
and I wouldn't be caught dead making this a product of my profession.
However, for a toy project, it's perfect.

Sometimes, there's just something you want to do, and you know you can't scrounge up the necessary time or willpower
to do it yourself.
If the quality of the work isn't really a priority, generating code is quite a nice thing.
Just don't expect a next-token-prediction machine to make good decisions for you---that's still your responsibility.

Oh, yeah, another thing nobody really ever mentions, I don't think.
This project, to the time of writing, cost about 25 dollarydoos to LLM-generate with usage-based pricing.
Is that a lot?
Maybe.
