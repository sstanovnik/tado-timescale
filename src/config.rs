//! Minimal runtime configuration helpers.
//! Defaults align with docker-compose (localhost TimescaleDB).

use chrono::NaiveDate;
use std::num::NonZeroU32;
use std::time::Duration;
use std::{fs, path::PathBuf};

pub const DEFAULT_DATABASE_URL: &str = "postgres://postgres:postgres@localhost:5432/tado";
pub const DEFAULT_REALTIME_SECS: u64 = 60;
pub const DEFAULT_REFRESH_TOKEN_FILE: &str = "token.txt";

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    /// Initial Tado OAuth refresh token obtained via browser login.
    pub tado_refresh_token: String,
    /// Path used to persist rotated refresh tokens.
    pub tado_refresh_token_file: PathBuf,
    /// Firefox version to spoof in the User-Agent (e.g. "143.0").
    pub tado_firefox_version: String,
    /// Realtime polling cadence.
    pub realtime_interval: Duration,
    /// Allow skipping the historical backfill on startup.
    pub backfill_enabled: bool,
    /// Optional lower bound for historical backfill (UTC date at 00:00:00).
    /// When set, the backfill will not request data prior to this date.
    pub backfill_from_date: Option<NaiveDate>,
    /// Optional cap on Tado historical day report requests per second.
    pub backfill_requests_per_second: Option<NonZeroU32>,
    /// Optional sampling rate for day reports during historical backfill (1/N days).
    pub backfill_sample_rate: Option<NonZeroU32>,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string());
        let tado_refresh_token_file = std::env::var("TADO_REFRESH_TOKEN_PERSISTENCE_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_REFRESH_TOKEN_FILE));

        let tado_refresh_token = if tado_refresh_token_file.is_file() {
            let contents = fs::read_to_string(&tado_refresh_token_file).map_err(|e| {
                format!(
                    "Failed to read refresh token from {}: {}",
                    tado_refresh_token_file.display(),
                    e
                )
            })?;
            let trimmed = contents.trim();
            if trimmed.is_empty() {
                return Err(format!(
                    "Refresh token file {} is empty; delete it or populate it with a valid token",
                    tado_refresh_token_file.display()
                ));
            }
            trimmed.to_string()
        } else {
            let env_value = std::env::var("INITIAL_TADO_REFRESH_TOKEN").map_err(|_| {
                format!(
                    "Missing refresh token: set INITIAL_TADO_REFRESH_TOKEN or provide {}",
                    tado_refresh_token_file.display()
                )
            })?;
            let trimmed = env_value.trim();
            if trimmed.is_empty() {
                return Err("INITIAL_TADO_REFRESH_TOKEN must not be empty".to_string());
            }
            trimmed.to_string()
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

        let backfill_requests_per_second = match std::env::var("BACKFILL_REQUESTS_PER_SECOND") {
            Ok(raw) if !raw.trim().is_empty() => {
                let parsed: u32 = raw
                    .trim()
                    .parse()
                    .map_err(|_| "BACKFILL_REQUESTS_PER_SECOND must be a positive integer".to_string())?;
                Some(
                    NonZeroU32::new(parsed)
                        .ok_or_else(|| "BACKFILL_REQUESTS_PER_SECOND must be greater than zero".to_string())?,
                )
            }
            _ => None,
        };

        let backfill_sample_rate = match std::env::var("BACKFILL_SAMPLE_RATE") {
            Ok(raw) if !raw.trim().is_empty() => {
                let trimmed = raw.trim();
                let mut parts = trimmed.split('/');
                let numerator = parts.next().unwrap_or_default().trim();
                let denominator = parts.next().map(str::trim);

                if parts.next().is_some() || numerator != "1" {
                    return Err("BACKFILL_SAMPLE_RATE must be in the form 1/N".to_string());
                }

                let denom_str =
                    denominator.ok_or_else(|| "BACKFILL_SAMPLE_RATE must be in the form 1/N".to_string())?;
                let denom: u32 = denom_str
                    .parse()
                    .map_err(|_| "BACKFILL_SAMPLE_RATE denominator must be a positive integer".to_string())?;
                Some(
                    NonZeroU32::new(denom)
                        .ok_or_else(|| "BACKFILL_SAMPLE_RATE denominator must be greater than zero".to_string())?,
                )
            }
            _ => None,
        };

        Ok(Config {
            database_url,
            tado_refresh_token,
            tado_refresh_token_file,
            tado_firefox_version,
            realtime_interval: Duration::from_secs(realtime_secs),
            backfill_enabled,
            backfill_from_date,
            backfill_requests_per_second,
            backfill_sample_rate,
        })
    }
}
