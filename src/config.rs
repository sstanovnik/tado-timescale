//! Minimal runtime configuration helpers.
//! Defaults align with docker-compose (localhost TimescaleDB).

use std::time::Duration;

pub const DEFAULT_DATABASE_URL: &str = "postgres://postgres:postgres@localhost:5432/tado";
pub const DEFAULT_REALTIME_SECS: u64 = 60;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    /// Initial Tado OAuth refresh token obtained via browser login.
    pub tado_refresh_token: String,
    /// Firefox version to spoof in the User-Agent (e.g. "140.0").
    pub tado_firefox_version: String,
    /// Realtime polling cadence.
    pub realtime_interval: Duration,
    /// Allow skipping the historical backfill on startup.
    pub backfill_enabled: bool,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string());

        let tado_refresh_token =
            std::env::var("TADO_REFRESH_TOKEN").map_err(|_| "Missing TADO_REFRESH_TOKEN in environment".to_string())?;

        let realtime_secs = std::env::var("REALTIME_INTERVAL_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(DEFAULT_REALTIME_SECS);

        let tado_firefox_version = std::env::var("TADO_FIREFOX_VERSION").unwrap_or_else(|_| "140.0".to_string());

        let backfill_enabled = std::env::var("BACKFILL_ENABLED")
            .ok()
            .map(|s| matches!(s.as_str(), "1" | "true" | "TRUE"))
            .unwrap_or(true);

        Ok(Config {
            database_url,
            tado_refresh_token,
            tado_firefox_version,
            realtime_interval: Duration::from_secs(realtime_secs),
            backfill_enabled,
        })
    }
}
