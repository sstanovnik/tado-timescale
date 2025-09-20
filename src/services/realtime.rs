use crate::client::TadoClient;
use crate::db::models::{NewClimateMeasurement, NewWeatherMeasurement};
use crate::models::tado::{self, HomeId};
use crate::schema;
use crate::utils::serde_enum_name;
use chrono::Utc;
use diesel::prelude::*;
use diesel::PgConnection;
use std::collections::BTreeMap;
use std::thread;
use std::time::{Duration, Instant};

pub fn run_loop(
    conn: &mut PgConnection,
    client: &TadoClient,
    home_ids: &[i64],
    interval: Duration,
) -> Result<(), String> {
    loop {
        let tick_start = Instant::now();

        for home_id in home_ids {
            let zones = client
                .get_zones(HomeId(*home_id))
                .map_err(|e| format!("get_zones({home_id}) failed: {}", e))?;
            collect_home(conn, client, *home_id, &zones)?;
        }

        // Maintain steady cadence
        let elapsed = tick_start.elapsed();
        if elapsed < interval {
            thread::sleep(interval - elapsed);
        }
    }
}

fn collect_home(
    conn: &mut PgConnection,
    client: &TadoClient,
    home_id: i64,
    zones: &[tado::Zone],
) -> Result<(), String> {
    use schema::climate_measurements::dsl as C;
    use schema::devices::dsl as D;
    use schema::homes::dsl as H;
    use schema::weather_measurements::dsl as W;
    use schema::zones::dsl as Z;

    let db_home_id: i64 = H::homes
        .filter(H::tado_home_id.eq(home_id))
        .select(H::id)
        .first(conn)
        .map_err(|e| format!("fetch db_home_id failed: {}", e))?;

    // zone -> db id map
    let mut zone_id_map = BTreeMap::new();
    for z in zones {
        if let Some(zid) = z.id {
            let db_zone_id: i64 = Z::zones
                .filter(Z::home_id.eq(db_home_id).and(Z::tado_zone_id.eq(zid.0)))
                .select(Z::id)
                .first(conn)
                .map_err(|e| format!("fetch db_zone_id failed: {}", e))?;
            zone_id_map.insert(zid.0, db_zone_id);
        }
    }

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
            .and_then(|v| serde_enum_name(v));

        let row = NewWeatherMeasurement {
            time: ts,
            home_id: db_home_id,
            source: "realtime".into(),
            outside_temp_c: weather.outside_temperature.as_ref().and_then(|t| t.celsius),
            solar_intensity_pct: weather.solar_intensity.as_ref().and_then(|s| s.percentage),
            weather_state,
        };
        let _ = diesel::insert_into(W::weather_measurements)
            .values(&row)
            .on_conflict((W::home_id, W::time, W::source))
            .do_nothing()
            .execute(conn);
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
            Err(_) => continue,
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
            .and_then(|set| set.mode.as_ref().and_then(|m| serde_enum_name(m)));

        let row = NewClimateMeasurement {
            time: ts,
            home_id: db_home_id,
            zone_id: Some(db_zone_id),
            device_id: None,
            source: "realtime".into(),
            inside_temp_c,
            humidity_pct,
            setpoint_temp_c,
            heating_power_pct,
            ac_power_on,
            ac_mode,
            window_open: state.open_window.as_ref().map(|_| true),
            battery_low: None,
            connection_up: None,
        };
        let _ = diesel::insert_into(C::climate_measurements)
            .values(&row)
            .on_conflict((C::time, C::home_id, C::source, C::zone_id, C::device_id))
            .do_nothing()
            .execute(conn);
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
                Err(_) => continue,
            };
            let ts = d
                .connection_state
                .as_ref()
                .and_then(|cs| cs.timestamp)
                .unwrap_or_else(Utc::now);
            let conn_up = d.connection_state.as_ref().and_then(|cs| cs.value);
            let battery_low = d.battery_state.map(|b| matches!(b, tado::BatteryState::Low));

            let row = NewClimateMeasurement {
                time: ts,
                home_id: db_home_id,
                zone_id: None,
                device_id: Some(db_device_id),
                source: "realtime".into(),
                inside_temp_c: None,
                humidity_pct: None,
                setpoint_temp_c: None,
                heating_power_pct: None,
                ac_power_on: None,
                ac_mode: None,
                window_open: None,
                battery_low,
                connection_up: conn_up,
            };
            let _ = diesel::insert_into(C::climate_measurements)
                .values(&row)
                .on_conflict((C::time, C::home_id, C::source, C::zone_id, C::device_id))
                .do_nothing()
                .execute(conn);
        }
    }

    Ok(())
}
