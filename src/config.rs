//! Minimal runtime configuration helpers.
//! Defaults align with docker-compose (localhost TimescaleDB).

use std::time::Duration;

pub const DEFAULT_DATABASE_URL: &str = "postgres://postgres:postgres@localhost:5432/tado";
pub const DEFAULT_REALTIME_SECS: u64 = 60;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub tado_username: String,
    pub tado_password: String,
    /// Realtime polling cadence.
    pub realtime_interval: Duration,
    /// Allow skipping the historical backfill on startup.
    pub backfill_enabled: bool,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string());

        let tado_username =
            std::env::var("TADO_USERNAME").map_err(|_| "Missing TADO_USERNAME in environment".to_string())?;
        let tado_password =
            std::env::var("TADO_PASSWORD").map_err(|_| "Missing TADO_PASSWORD in environment".to_string())?;

        let realtime_secs = std::env::var("REALTIME_INTERVAL_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(DEFAULT_REALTIME_SECS);

        let backfill_enabled = std::env::var("BACKFILL_ENABLED")
            .ok()
            .map(|s| matches!(s.as_str(), "1" | "true" | "TRUE"))
            .unwrap_or(true);

        Ok(Config {
            database_url,
            tado_username,
            tado_password,
            realtime_interval: Duration::from_secs(realtime_secs),
            backfill_enabled,
        })
    }
}
