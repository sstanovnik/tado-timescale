extern crate core;

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

pub fn run() -> Result<(), String> {
    // 1) Load config
    let cfg = Config::from_env()?;

    // 2) Connect DB
    let mut conn = PgConnection::establish(&cfg.database_url).map_err(|e| format!("DB connection failed: {}", e))?;

    // 3) Init Tado client
    let client = TadoClient::new(cfg.tado_username.clone(), cfg.tado_password.clone())
        .map_err(|e| format!("Tado auth failed: {}", e))?;

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

    // 5) Sync reference data (users/homes/zones/devices/links)
    refs::sync_all(&mut conn, &client, &me, &target_homes)?;

    // 6) Historical backfill
    if cfg.backfill_enabled {
        for home_id in &target_homes {
            backfill::run_for_home(&mut conn, &client, HomeId(*home_id))?;
        }
    }

    // 7) Realtime loop (steady cadence)
    realtime::run_loop(&mut conn, &client, &target_homes, cfg.realtime_interval)?;

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("fatal: {}", e);
        std::process::exit(1);
    }
}
