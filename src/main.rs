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
use log::{error, info};

pub fn run() -> Result<(), String> {
    // 1) Load config
    let cfg = Config::from_env()?;
    info!(
        "Config loaded (realtime_interval={}s, backfill_enabled={})",
        cfg.realtime_interval.as_secs(),
        cfg.backfill_enabled
    );

    // 2) Connect DB
    let mut conn = PgConnection::establish(&cfg.database_url).map_err(|e| format!("DB connection failed: {}", e))?;
    info!("Connected to database");

    // 3) Init Tado client
    let client = TadoClient::new(cfg.tado_refresh_token.clone(), cfg.tado_firefox_version.clone())
        .map_err(|e| format!("Tado auth failed (refresh token invalid/expired?): {}", e))?;
    info!("Authenticated to Tado API");

    // 4) Discover homes
    let me = client.get_me().map_err(|e| format!("get_me failed: {}", e))?;
    let mut target_homes = me
        .homes
        .as_ref()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|hb| hb.id.map(|id| id.0))
        .collect::<Vec<_>>();
    target_homes.sort_unstable();
    target_homes.dedup();
    if target_homes.is_empty() {
        return Err("No homes found; ensure the account has homes".into());
    }
    info!("Discovered {} home(s)", target_homes.len());

    // 5) Sync reference data (users/homes/zones/devices/links)
    info!("Syncing reference data");
    refs::sync_all(&mut conn, &client, &me, &target_homes)?;
    info!("Reference data sync complete");

    // 6) Historical backfill
    if cfg.backfill_enabled {
        info!("Starting historical backfill for {} home(s)", target_homes.len());
        for home_id in &target_homes {
            backfill::run_for_home(&mut conn, &client, HomeId(*home_id))?;
            info!("Backfill completed for home {}", home_id);
        }
    }

    // 7) Realtime loop (steady cadence)
    info!(
        "Starting realtime loop: homes={}, interval={}s",
        target_homes.len(),
        cfg.realtime_interval.as_secs()
    );
    realtime::run_loop(&mut conn, &client, &target_homes, cfg.realtime_interval)?;

    Ok(())
}

fn main() {
    // Init logging early; default to debug if RUST_LOG not set
    let default_filter = env_logger::Env::default().default_filter_or("debug");
    env_logger::Builder::from_env(default_filter)
        .format_timestamp_secs()
        .init();

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
