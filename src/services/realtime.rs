use crate::client::TadoClient;
use crate::db::models::event_source;
use crate::db::models::{NewClimateMeasurement, NewWeatherMeasurement};
use crate::models::tado::{self, HomeId};
use crate::schema;
use crate::utils::serde_enum_name;
use chrono::Utc;
use diesel::prelude::*;
use diesel::PgConnection;
use log::{debug, info, warn};
use std::collections::BTreeMap;
use std::thread;
use std::time::{Duration, Instant};

pub fn run_loop(
    conn: &mut PgConnection,
    client: &TadoClient,
    home_ids: &[i64],
    interval: Duration,
) -> Result<(), String> {
    info!(
        "Realtime loop started (homes={}, interval={}s)",
        home_ids.len(),
        interval.as_secs()
    );
    // Build caches for DB identifiers used every tick
    use schema::homes::dsl as H;
    use schema::zones::dsl as Z;

    // Cache: tado_home_id -> db_home_id
    let mut home_db_ids: BTreeMap<i64, i64> = BTreeMap::new();
    // Cache: tado_home_id -> (tado_zone_id -> db_zone_id)
    let mut zone_maps: BTreeMap<i64, BTreeMap<i64, i64>> = BTreeMap::new();

    for home_id in home_ids {
        let db_home_id: i64 = H::homes
            .filter(H::tado_home_id.eq(*home_id))
            .select(H::id)
            .first(conn)
            .map_err(|e| format!("fetch db_home_id failed: {}", e))?;
        home_db_ids.insert(*home_id, db_home_id);

        // Build zone map once per home from DB state
        let rows: Vec<(i64, i64)> = Z::zones
            .filter(Z::home_id.eq(db_home_id))
            .select((Z::tado_zone_id, Z::id))
            .load(conn)
            .map_err(|e| format!("fetch zone map failed: {}", e))?;
        let zmap: BTreeMap<i64, i64> = rows.into_iter().collect();
        zone_maps.insert(*home_id, zmap);
    }

    loop {
        let tick_start = Instant::now();

        for home_id in home_ids {
            let db_home_id = match home_db_ids.get(home_id).copied() {
                Some(id) => id,
                None => continue,
            };
            let Some(zone_map) = zone_maps.get(home_id) else {
                continue;
            };
            let zones = client
                .get_zones(HomeId(*home_id))
                .map_err(|e| format!("get_zones({home_id}) failed: {}", e))?;
            debug!("Realtime: collecting home {} ({} zones)", home_id, zones.len());
            collect_home(conn, client, db_home_id, *home_id, &zones, zone_map)?;
        }

        // Maintain steady cadence
        let elapsed = tick_start.elapsed();
        if elapsed < interval {
            thread::sleep(interval - elapsed);
        }
        debug!("Realtime tick completed in {} ms", tick_start.elapsed().as_millis());
    }
}

fn collect_home(
    conn: &mut PgConnection,
    client: &TadoClient,
    db_home_id: i64,
    home_id: i64,
    zones: &[tado::Zone],
    zone_id_map: &BTreeMap<i64, i64>,
) -> Result<(), String> {
    use schema::climate_measurements::dsl as C;
    use schema::devices::dsl as D;
    use schema::weather_measurements::dsl as W;

    // Weather (home-scoped)
    if let Ok(weather) = client.get_weather(HomeId(home_id)) {
        let now_ts = Utc::now();
        let ts = weather
            .outside_temperature
            .as_ref()
            .and_then(|t| t.timestamp)
            .or_else(|| weather.solar_intensity.as_ref().and_then(|s| s.timestamp))
            .unwrap_or(now_ts);
        let weather_state = weather
            .weather_state
            .as_ref()
            .and_then(|ws| ws.value.as_ref())
            .and_then(serde_enum_name);

        let mut row = NewWeatherMeasurement::new(ts, db_home_id, event_source::REALTIME);
        row.outside_temp_c = weather.outside_temperature.as_ref().and_then(|t| t.celsius);
        row.solar_intensity_pct = weather.solar_intensity.as_ref().and_then(|s| s.percentage);
        row.weather_state = weather_state;
        if let Err(e) = diesel::insert_into(W::weather_measurements)
            .values(&row)
            .on_conflict((W::home_id, W::time, W::source))
            .do_nothing()
            .execute(conn)
        {
            warn!("Realtime: insert weather row failed for home {}: {}", home_id, e);
        }
    }

    // Zones realtime
    for z in zones {
        let Some(zone_id) = z.id else { continue };
        let db_zone_id = match zone_id_map.get(&zone_id.0) {
            Some(v) => *v,
            None => continue,
        };
        let state = match client.get_zone_state(HomeId(home_id), zone_id) {
            Ok(s) => s,
            Err(e) => {
                debug!("Realtime: get_zone_state({}, {}) failed: {}", home_id, zone_id.0, e);
                continue;
            }
        };

        let now_ts = Utc::now();
        // pick the most precise timestamp available
        let ts = state
            .sensor_data_points
            .as_ref()
            .and_then(|s| s.inside_temperature.as_ref().and_then(|t| t.timestamp))
            .or_else(|| {
                state
                    .sensor_data_points
                    .as_ref()
                    .and_then(|s| s.humidity.as_ref().and_then(|h| h.timestamp))
            })
            .or_else(|| {
                state
                    .activity_data_points
                    .as_ref()
                    .and_then(|a| a.heating_power.as_ref().and_then(|p| p.timestamp))
            })
            .or_else(|| {
                state
                    .activity_data_points
                    .as_ref()
                    .and_then(|a| a.ac_power.as_ref().and_then(|p| p.timestamp))
            })
            .unwrap_or(now_ts);

        // Extract values without moving the state
        let inside_temp_c = state
            .sensor_data_points
            .as_ref()
            .and_then(|s| s.inside_temperature.as_ref().and_then(|t| t.celsius));
        let humidity_pct = state
            .sensor_data_points
            .as_ref()
            .and_then(|s| s.humidity.as_ref().and_then(|h| h.percentage));
        let setpoint_temp_c = state
            .setting
            .as_ref()
            .and_then(|set| set.temperature.as_ref().and_then(|t| t.celsius));
        let heating_power_pct = state
            .activity_data_points
            .as_ref()
            .and_then(|a| a.heating_power.as_ref().and_then(|p| p.percentage));
        let ac_power_on = state.activity_data_points.as_ref().and_then(|a| {
            a.ac_power
                .as_ref()
                .and_then(|p| p.value.map(|v| matches!(v, tado::Power::On)))
        });
        let ac_mode = state
            .setting
            .as_ref()
            .and_then(|set| set.mode.as_ref().and_then(serde_enum_name));

        let mut row = NewClimateMeasurement::new(ts, db_home_id, Some(db_zone_id), None, event_source::REALTIME);
        row.inside_temp_c = inside_temp_c;
        row.humidity_pct = humidity_pct;
        row.setpoint_temp_c = setpoint_temp_c;
        row.heating_power_pct = heating_power_pct;
        row.ac_power_on = ac_power_on;
        row.ac_mode = ac_mode;
        row.window_open = state.open_window.as_ref().map(|_| true);
        if let Err(e) = diesel::insert_into(C::climate_measurements)
            .values(&row)
            .on_conflict((C::time, C::home_id, C::source, C::zone_id, C::device_id))
            .do_nothing()
            .execute(conn)
        {
            warn!(
                "Realtime: insert climate row failed for home {}, zone {}: {}",
                home_id, zone_id.0, e
            );
        }
    }

    // Devices realtime (battery/connection)
    if let Ok(devs) = client.get_devices(HomeId(home_id)) {
        for d in devs {
            let Some(serial) = d.serial_no.as_ref().map(|s| s.0.clone()) else {
                continue;
            };
            let db_device_id: i64 = match D::devices
                .filter(D::home_id.eq(db_home_id).and(D::tado_device_id.eq(&serial)))
                .select(D::id)
                .first::<i64>(conn)
            {
                Ok(id) => id,
                Err(e) => {
                    debug!("Realtime: device {} not found in DB: {}", serial, e);
                    continue;
                }
            };
            let ts = d
                .connection_state
                .as_ref()
                .and_then(|cs| cs.timestamp)
                .unwrap_or_else(Utc::now);
            let conn_up = d.connection_state.as_ref().and_then(|cs| cs.value);
            let battery_low = d.battery_state.map(|b| matches!(b, tado::BatteryState::Low));

            let mut row = NewClimateMeasurement::new(ts, db_home_id, None, Some(db_device_id), event_source::REALTIME);
            row.battery_low = battery_low;
            row.connection_up = conn_up;
            if let Err(e) = diesel::insert_into(C::climate_measurements)
                .values(&row)
                .on_conflict((C::time, C::home_id, C::source, C::zone_id, C::device_id))
                .do_nothing()
                .execute(conn)
            {
                warn!(
                    "Realtime: insert device climate row failed for home {}, device {}: {}",
                    home_id, serial, e
                );
            }
        }
    }

    Ok(())
}
