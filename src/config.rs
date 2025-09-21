//! Minimal runtime configuration helpers.
//! Defaults align with docker-compose (localhost TimescaleDB).

use chrono::NaiveDate;
use std::time::Duration;
use std::{fs, path::Path};

pub const DEFAULT_DATABASE_URL: &str = "postgres://postgres:postgres@localhost:5432/tado";
pub const DEFAULT_REALTIME_SECS: u64 = 60;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    /// Initial Tado OAuth refresh token obtained via browser login.
    pub tado_refresh_token: String,
    /// Firefox version to spoof in the User-Agent (e.g. "143.0").
    pub tado_firefox_version: String,
    /// Realtime polling cadence.
    pub realtime_interval: Duration,
    /// Allow skipping the historical backfill on startup.
    pub backfill_enabled: bool,
    /// Optional lower bound for historical backfill (UTC date at 00:00:00).
    /// When set, the backfill will not request data prior to this date.
    pub backfill_from_date: Option<NaiveDate>,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string());
        // Prefer env var; fallback to token.txt in working directory
        let tado_refresh_token =
            match std::env::var("TADO_REFRESH_TOKEN") {
                Ok(v) if !v.trim().is_empty() => v,
                _ => {
                    let path = Path::new("token.txt");
                    match fs::read_to_string(path) {
                        Ok(s) if !s.trim().is_empty() => s.trim().to_string(),
                        _ => return Err(
                            "Missing refresh token: set TADO_REFRESH_TOKEN or provide token.txt in working directory"
                                .to_string(),
                        ),
                    }
                }
            };

        let realtime_secs = std::env::var("REALTIME_INTERVAL_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(DEFAULT_REALTIME_SECS);

        let tado_firefox_version = std::env::var("TADO_FIREFOX_VERSION").unwrap_or_else(|_| "143.0".to_string());

        let backfill_enabled = std::env::var("BACKFILL_ENABLED")
            .ok()
            .map(|s| matches!(s.as_str(), "1" | "true" | "TRUE"))
            .unwrap_or(true);

        let backfill_from_date = match std::env::var("BACKFILL_FROM_DATE") {
            Ok(s) if !s.trim().is_empty() => Some(
                NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d")
                    .map_err(|_| "BACKFILL_FROM_DATE must be in YYYY-MM-DD format".to_string())?,
            ),
            _ => None,
        };

        Ok(Config {
            database_url,
            tado_refresh_token,
            tado_firefox_version,
            realtime_interval: Duration::from_secs(realtime_secs),
            backfill_enabled,
            backfill_from_date,
        })
    }
}
