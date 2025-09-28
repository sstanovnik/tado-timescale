//! Minimal runtime configuration helpers.
//! Defaults align with docker-compose (localhost TimescaleDB).

use chrono::{Duration as ChronoDuration, NaiveDate};
use std::env::{self, VarError};
use std::num::NonZeroU32;
use std::time::Duration;
use std::{fs, path::PathBuf};

pub const DEFAULT_DATABASE_URL: &str = "postgres://postgres:postgres@localhost:5432/tado";
pub const DEFAULT_REALTIME_SECS: u64 = 60;
pub const DEFAULT_REFRESH_TOKEN_FILE: &str = "token.txt";
pub const DEFAULT_MAX_REQUEST_RETRIES: u32 = 3;

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
    /// Allow skipping the realtime polling loop on startup.
    pub realtime_enabled: bool,
    /// Allow skipping the historical backfill on startup.
    pub backfill_enabled: bool,
    /// Optional lower bound for historical backfill (UTC date at 00:00:00).
    /// When set, the backfill will not request data prior to this date.
    pub backfill_from_date: Option<NaiveDate>,
    /// Optional cap on Tado historical day report requests per second.
    pub backfill_requests_per_second: Option<NonZeroU32>,
    /// Optional sampling rate for day reports during historical backfill (1/N days).
    pub backfill_sample_rate: Option<NonZeroU32>,
    /// Number of retries to perform after the initial request when a server-side error (5xx) occurs.
    pub max_request_retries: NonZeroU32,
    /// Minimum gap size that qualifies for historical backfill.
    pub backfill_min_gap: ChronoDuration,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string());
        let tado_refresh_token_file = env::var("TADO_REFRESH_TOKEN_PERSISTENCE_FILE")
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
            let env_value = env::var("INITIAL_TADO_REFRESH_TOKEN").map_err(|_| {
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

        let realtime_secs = env_u64("REALTIME_INTERVAL_SECS", DEFAULT_REALTIME_SECS)?;
        if realtime_secs == 0 {
            return Err("REALTIME_INTERVAL_SECS must be at least 1".to_string());
        }

        let tado_firefox_version = env::var("TADO_FIREFOX_VERSION").unwrap_or_else(|_| "143.0".to_string());

        let realtime_enabled = env_bool("REALTIME_ENABLED", true)?;

        let backfill_enabled = env_bool("BACKFILL_ENABLED", true)?;

        let backfill_from_date = match env_var_trimmed("BACKFILL_FROM_DATE")? {
            Some(value) => Some(
                NaiveDate::parse_from_str(&value, "%Y-%m-%d")
                    .map_err(|_| "BACKFILL_FROM_DATE must be in YYYY-MM-DD format".to_string())?,
            ),
            None => None,
        };

        let backfill_requests_per_second = env_nonzero_u32("BACKFILL_REQUESTS_PER_SECOND")?;

        let backfill_sample_rate = match env_var_trimmed("BACKFILL_SAMPLE_RATE")? {
            Some(trimmed) => {
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
            None => None,
        };

        let backfill_min_gap_minutes = env_nonzero_u32_with_default(
            "BACKFILL_MIN_GAP_MINUTES",
            NonZeroU32::new(240).expect("default backfill gap minutes > 0"),
        )?;

        let max_request_retries = env_nonzero_u32_with_default(
            "MAX_REQUEST_RETRIES",
            NonZeroU32::new(DEFAULT_MAX_REQUEST_RETRIES)
                .expect("DEFAULT_MAX_REQUEST_RETRIES must be greater than zero"),
        )?;

        Ok(Config {
            database_url,
            tado_refresh_token,
            tado_refresh_token_file,
            tado_firefox_version,
            realtime_interval: Duration::from_secs(realtime_secs),
            realtime_enabled,
            backfill_enabled,
            backfill_from_date,
            backfill_requests_per_second,
            backfill_sample_rate,
            max_request_retries,
            backfill_min_gap: ChronoDuration::minutes(backfill_min_gap_minutes.get() as i64),
        })
    }
}

fn env_var_trimmed(name: &str) -> Result<Option<String>, String> {
    match env::var(name) {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(trimmed.to_string()))
            }
        }
        Err(VarError::NotPresent) => Ok(None),
        Err(VarError::NotUnicode(_)) => Err(format!("{} contains invalid UTF-8", name)),
    }
}

fn env_bool(name: &str, default: bool) -> Result<bool, String> {
    match env_var_trimmed(name)? {
        None => Ok(default),
        Some(value) => match parse_bool(&value) {
            Some(result) => Ok(result),
            None => Err(format!("{} must be a boolean (use true/false/1/0)", name)),
        },
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    if value.eq_ignore_ascii_case("true") || value == "1" {
        Some(true)
    } else if value.eq_ignore_ascii_case("false") || value == "0" {
        Some(false)
    } else {
        None
    }
}

fn env_nonzero_u32(name: &str) -> Result<Option<NonZeroU32>, String> {
    match env_var_trimmed(name)? {
        None => Ok(None),
        Some(value) => {
            let parsed: u32 = value
                .parse()
                .map_err(|_| format!("{} must be a positive integer", name))?;
            NonZeroU32::new(parsed)
                .ok_or_else(|| format!("{} must be greater than zero", name))
                .map(Some)
        }
    }
}

fn env_nonzero_u32_with_default(name: &str, default: NonZeroU32) -> Result<NonZeroU32, String> {
    Ok(env_nonzero_u32(name)?.unwrap_or(default))
}

fn env_u64(name: &str, default: u64) -> Result<u64, String> {
    match env_var_trimmed(name)? {
        None => Ok(default),
        Some(value) => value
            .parse::<u64>()
            .map_err(|_| format!("{} must be a non-negative integer", name)),
    }
}
