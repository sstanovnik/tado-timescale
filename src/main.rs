pub mod models {
    pub mod tado;
}

pub mod client;
pub mod config;
pub mod db {
    pub mod models;
}
pub mod schema;
pub mod utils;
pub mod services {
    pub mod backfill;
    pub mod realtime;
    pub mod refs;
}

use crate::client::TadoClient;
use crate::config::Config;
use crate::models::tado::HomeId;
use crate::services::{backfill, realtime, refs};
use diesel::prelude::*;
use diesel::PgConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use log::{error, info};
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct LoadedEnvFile {
    path: PathBuf,
    explicit: bool,
}

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

fn apply_database_migrations(conn: &mut PgConnection) -> Result<(), String> {
    match conn.run_pending_migrations(MIGRATIONS) {
        Ok(applied) => {
            if applied.is_empty() {
                info!("Database schema is up to date; no migrations were applied");
            } else {
                let names = applied.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", ");
                info!("Applied {} database migration(s): {}", applied.len(), names);
            }
            Ok(())
        }
        Err(e) => Err(format!("Applying database migrations failed: {}", e)),
    }
}

pub fn run() -> Result<(), String> {
    // 1) Load config
    let cfg = Config::from_env()?;
    info!(
        "Config loaded (realtime_interval={}s, realtime_enabled={}, backfill_enabled={}, backfill_from={}, backfill_rps={}, backfill_sample_rate={}, max_request_retries={})",
        cfg.realtime_interval.as_secs(),
        cfg.realtime_enabled,
        cfg.backfill_enabled,
        cfg.backfill_from_date
            .map(|d| d.to_string())
            .unwrap_or_else(|| "-".to_string()),
        cfg.backfill_requests_per_second
            .map(|v| v.get().to_string())
            .unwrap_or_else(|| "-".to_string()),
        cfg.backfill_sample_rate
            .map(|v| format!("1/{}", v.get()))
            .unwrap_or_else(|| "-".to_string()),
        cfg.max_request_retries.get()
    );

    // 2) Connect DB
    let mut conn = PgConnection::establish(&cfg.database_url).map_err(|e| format!("DB connection failed: {}", e))?;
    info!("Connected to database");

    // 3) Apply pending database migrations
    apply_database_migrations(&mut conn)?;

    // 4) Init Tado client
    let client = TadoClient::new(
        &cfg.tado_refresh_token,
        &cfg.tado_firefox_version,
        cfg.tado_refresh_token_file.clone(),
        cfg.max_request_retries,
    )
    .map_err(|e| format!("Tado auth failed (refresh token invalid/expired?): {}", e))?;
    info!("Authenticated to Tado API");

    // 5) Discover homes
    let me = client.get_me().map_err(|e| format!("get_me failed: {}", e))?;
    let mut target_homes = me
        .homes
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter_map(|hb| hb.id.map(|id| id.0))
        .collect::<Vec<_>>();
    target_homes.sort_unstable();
    target_homes.dedup();
    if target_homes.is_empty() {
        return Err("No homes found; ensure the account has homes".into());
    }
    info!("Discovered {} home(s)", target_homes.len());

    // 6) Sync reference data (users/homes/zones/devices/links)
    info!("Syncing reference data");
    refs::sync_all(&mut conn, &client, &me, &target_homes)?;
    info!("Reference data sync complete");

    // 7) Historical backfill
    if cfg.backfill_enabled {
        info!("Starting historical backfill for {} home(s)", target_homes.len());
        for home_id in &target_homes {
            backfill::run_for_home(
                &mut conn,
                &client,
                HomeId(*home_id),
                cfg.backfill_from_date,
                cfg.backfill_requests_per_second,
                cfg.backfill_sample_rate,
            )?;
            info!("Backfill completed for home {}", home_id);
        }
    } else {
        info!(
            "Historical backfill disabled via BACKFILL_ENABLED={}",
            cfg.backfill_enabled
        );
    }

    // 8) Realtime loop (steady cadence)
    if cfg.realtime_enabled {
        info!(
            "Starting realtime loop: homes={}, interval={}s",
            target_homes.len(),
            cfg.realtime_interval.as_secs()
        );
        realtime::run_loop(&mut conn, &client, &target_homes, cfg.realtime_interval)?;
    } else {
        info!("Realtime loop disabled via REALTIME_ENABLED={}", cfg.realtime_enabled);
    }

    Ok(())
}

fn configure_env_from_cli() -> Result<Option<LoadedEnvFile>, String> {
    let mut args = std::env::args_os();
    args.next(); // skip program name

    let mut env_file: Option<PathBuf> = None;

    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--env-file") => {
                if env_file.is_some() {
                    return Err("`--env-file` provided more than once".to_string());
                }
                let value = args
                    .next()
                    .ok_or_else(|| "`--env-file` requires a path argument".to_string())?;
                env_file = Some(PathBuf::from(value));
            }
            Some(s) if s.starts_with("--env-file=") => {
                if env_file.is_some() {
                    return Err("`--env-file` provided more than once".to_string());
                }
                let path_str = &s["--env-file=".len()..];
                if path_str.is_empty() {
                    return Err("`--env-file` requires a path argument".to_string());
                }
                env_file = Some(PathBuf::from(path_str));
            }
            Some("--") => break,
            Some(other) => return Err(format!("unrecognised argument: {}", other)),
            None => return Err("argument contains invalid UTF-8".to_string()),
        }
    }

    if let Some(path) = env_file {
        if !path.is_file() {
            return Err(format!("env file not found: {}", path.display()));
        }
        load_env_file(&path)?;
        Ok(Some(LoadedEnvFile { path, explicit: true }))
    } else {
        let cwd = std::env::current_dir().map_err(|e| format!("unable to read current directory: {}", e))?;
        let default_path = cwd.join(".env");
        if default_path.is_file() {
            load_env_file(&default_path)?;
            Ok(Some(LoadedEnvFile {
                path: default_path,
                explicit: false,
            }))
        } else {
            Ok(None)
        }
    }
}

fn load_env_file(path: &Path) -> Result<(), String> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let file = File::open(path).map_err(|e| format!("failed to open {}: {}", path.display(), e))?;
    let reader = BufReader::new(file);

    for (index, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| format!("failed to read {} at line {}: {}", path.display(), index + 1, e))?;
        match parse_env_assignment(&line) {
            Ok(Some((key, value))) => {
                // Preserve any value that was already supplied via the process environment.
                if std::env::var_os(&key).is_none() {
                    // Updating process-level environment variables is unsafe on some targets.
                    unsafe {
                        std::env::set_var(key, value);
                    }
                }
            }
            Ok(None) => {}
            Err(e) => {
                return Err(format!("{}:{}: {}", path.display(), index + 1, e));
            }
        }
    }

    Ok(())
}

fn parse_env_assignment(line: &str) -> Result<Option<(String, String)>, String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return Ok(None);
    }

    let without_export = trimmed
        .strip_prefix("export ")
        .map(|s| s.trim_start())
        .unwrap_or(trimmed);

    let mut parts = without_export.splitn(2, '=');
    let key = parts
        .next()
        .map(str::trim)
        .ok_or_else(|| "missing environment variable name".to_string())?;
    let value_part = parts.next().ok_or_else(|| "missing '=' in assignment".to_string())?;

    if key.is_empty() {
        return Err("environment variable name cannot be empty".to_string());
    }
    if key.chars().any(|c| c.is_whitespace()) {
        return Err(format!("environment variable name contains whitespace: {}", key));
    }

    let value = parse_env_value(value_part)?;
    Ok(Some((key.to_string(), value)))
}

fn parse_env_value(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }

    if let Some(rest) = trimmed.strip_prefix('"') {
        parse_double_quoted(rest)
    } else if let Some(rest) = trimmed.strip_prefix('\'') {
        parse_single_quoted(rest)
    } else {
        let value = trimmed.splitn(2, '#').next().unwrap_or_default().trim_end();
        Ok(value.to_string())
    }
}

fn parse_double_quoted(input: &str) -> Result<String, String> {
    let mut result = String::new();
    let mut chars = input.chars();
    let mut escape = false;

    while let Some(ch) = chars.next() {
        if escape {
            let value = match ch {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                '\\' => '\\',
                '"' => '"',
                other => other,
            };
            result.push(value);
            escape = false;
            continue;
        }

        match ch {
            '\\' => escape = true,
            '"' => {
                let remainder = chars.as_str().trim();
                if remainder.is_empty() || remainder.starts_with('#') {
                    return Ok(result);
                } else {
                    return Err("unexpected characters after closing double quote".to_string());
                }
            }
            other => result.push(other),
        }
    }

    if escape {
        Err("unterminated escape sequence in double-quoted value".to_string())
    } else {
        Err("unterminated double-quoted value".to_string())
    }
}

fn parse_single_quoted(input: &str) -> Result<String, String> {
    let mut result = String::new();
    let mut chars = input.chars();

    while let Some(ch) = chars.next() {
        if ch == '\'' {
            let remainder = chars.as_str().trim();
            if remainder.is_empty() || remainder.starts_with('#') {
                return Ok(result);
            } else {
                return Err("unexpected characters after closing single quote".to_string());
            }
        } else {
            result.push(ch);
        }
    }

    Err("unterminated single-quoted value".to_string())
}

fn main() {
    let loaded_env = match configure_env_from_cli() {
        Ok(info) => info,
        Err(err) => {
            eprintln!("fatal: {}", err);
            std::process::exit(1);
        }
    };

    // Init logging after environment so RUST_LOG from .env is respected.
    let default_filter = env_logger::Env::default().default_filter_or("info");
    env_logger::Builder::from_env(default_filter)
        .format_timestamp_secs()
        .init();

    if let Some(info) = loaded_env.as_ref() {
        let origin = if info.explicit { "CLI-specified" } else { "default" };
        info!("Environment loaded from {} .env file: {}", origin, info.path.display());
    }

    info!(
        "tado-timescale {} (git {}) starting",
        env!("CARGO_PKG_VERSION"),
        env!("BUILD_TIME_GIT_HASH")
    );
    if let Err(e) = run() {
        error!("fatal: {}", e);
        std::process::exit(1);
    }
}
