use crate::client::TadoClient;
use crate::db::models::NewClimateMeasurement;
use crate::models::tado::{self, HomeId, ZoneId};
use crate::schema;
use crate::utils::{determine_zone_start_time, serde_enum_name};
use chrono::{DateTime, NaiveDate, Utc};
use diesel::dsl::{max, min};
use diesel::prelude::*;
use diesel::PgConnection;
use std::collections::BTreeMap;

pub fn run_for_home(conn: &mut PgConnection, client: &TadoClient, home_id: HomeId) -> Result<(), String> {
    // Fetch zones to decide backfill per zone
    let zones = client
        .get_zones(home_id)
        .map_err(|e| format!("get_zones({}) failed: {}", home_id.0, e))?;

    // Resolve DB home id
    let db_home_id: i64 = schema::homes::dsl::homes
        .filter(schema::homes::dsl::tado_home_id.eq(home_id.0))
        .select(schema::homes::dsl::id)
        .first(conn)
        .map_err(|e| format!("fetch db_home_id failed: {}", e))?;

    // Map of tado zone id -> db zone id (only those with date_created)
    let mut zone_id_map = BTreeMap::new();
    for z in &zones {
        if let (Some(zid), Some(_)) = (z.id, z.date_created) {
            let db_zone_id: i64 = schema::zones::dsl::zones
                .filter(
                    schema::zones::dsl::home_id
                        .eq(db_home_id)
                        .and(schema::zones::dsl::tado_zone_id.eq(zid.0)),
                )
                .select(schema::zones::dsl::id)
                .first(conn)
                .map_err(|e| format!("fetch db_zone_id failed: {}", e))?;
            zone_id_map.insert(zid.0, db_zone_id);
        }
    }

    for z in &zones {
        let (Some(zone_id), Some(_)) = (z.id, z.date_created) else {
            continue;
        };
        let db_zone_id = match zone_id_map.get(&zone_id.0) {
            Some(v) => *v,
            None => continue,
        };
        let start = determine_zone_start_time(client, home_id, zone_id)
            .map_err(|e| format!("determine start time failed for zone {}: {}", zone_id.0, e))?;
        let (from, to) = compute_backfill_window(conn, db_home_id, db_zone_id, start)?;
        if from >= to {
            continue;
        }
        backfill_zone_range(conn, client, home_id, db_home_id, zone_id, db_zone_id, from, to)?;
    }

    Ok(())
}

fn compute_backfill_window(
    conn: &mut PgConnection,
    db_home_id: i64,
    db_zone_id: i64,
    start: DateTime<Utc>,
) -> Result<(DateTime<Utc>, DateTime<Utc>), String> {
    use schema::climate_measurements::dsl as C;
    let last_hist: Option<DateTime<Utc>> = schema::climate_measurements::dsl::climate_measurements
        .filter(
            C::home_id
                .eq(db_home_id)
                .and(C::zone_id.eq(db_zone_id))
                .and(C::source.eq("historical")),
        )
        .select(max(C::time))
        .first(conn)
        .map_err(|e| format!("query last historical failed: {}", e))?;
    let earliest_realtime: Option<DateTime<Utc>> = schema::climate_measurements::dsl::climate_measurements
        .filter(
            C::home_id
                .eq(db_home_id)
                .and(C::zone_id.eq(db_zone_id))
                .and(C::source.eq("realtime")),
        )
        .select(min(C::time))
        .first(conn)
        .map_err(|e| format!("query earliest realtime failed: {}", e))?;

    let from = last_hist.map(|t| t + chrono::Duration::seconds(1)).unwrap_or(start);
    let to = earliest_realtime.unwrap_or_else(Utc::now);
    Ok((from, to))
}

fn backfill_zone_range(
    conn: &mut PgConnection,
    client: &TadoClient,
    home_id: HomeId,
    db_home_id: i64,
    zone_id: ZoneId,
    db_zone_id: i64,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<(), String> {
    use schema::climate_measurements::dsl as C;

    // Iterate by day using local UTC date boundaries
    let mut cursor = from.date_naive();
    let end_date = to.date_naive();

    while cursor <= end_date {
        let report = client
            .get_zone_day_report(home_id, zone_id, Some(cursor))
            .map_err(|e| {
                format!(
                    "get_zone_day_report({}, {}, {}) failed: {}",
                    home_id.0, zone_id.0, cursor, e
                )
            })?;

        let mut by_ts: BTreeMap<DateTime<Utc>, NewClimateMeasurement> = BTreeMap::new();

        if let Some(md) = report.measured_data.clone() {
            if let Some(temp_series) = md.inside_temperature.and_then(|s| s.data_points) {
                for dp in temp_series {
                    if let (Some(ts), Some(val)) = (dp.timestamp, dp.value.and_then(|t| t.celsius)) {
                        if ts < from || ts >= to {
                            continue;
                        }
                        let entry = by_ts.entry(ts).or_insert_with(|| new_row(ts, db_home_id, db_zone_id));
                        entry.inside_temp_c = Some(val);
                    }
                }
            }
            if let Some(h_series) = md.humidity.and_then(|s| s.data_points) {
                for dp in h_series {
                    if let (Some(ts), Some(val)) = (dp.timestamp, dp.value) {
                        if ts < from || ts >= to {
                            continue;
                        }
                        let entry = by_ts.entry(ts).or_insert_with(|| new_row(ts, db_home_id, db_zone_id));
                        entry.humidity_pct = Some(val);
                    }
                }
            }
            if let Some(conn_series) = md.measuring_device_connected.and_then(|s| s.data_intervals) {
                for di in conn_series {
                    if let (Some(ts), Some(val)) = (di.interval.from, di.value) {
                        if ts < from || ts >= to {
                            continue;
                        }
                        let entry = by_ts.entry(ts).or_insert_with(|| new_row(ts, db_home_id, db_zone_id));
                        entry.connection_up = Some(val);
                    }
                }
            }
        }

        if let Some(cf) = report.call_for_heat.and_then(|s| s.data_intervals) {
            for di in cf {
                if let (Some(ts), Some(val)) = (di.interval.from, di.value) {
                    if ts < from || ts >= to {
                        continue;
                    }
                    let pct = match val {
                        tado::CallForHeatValue::None_ => 0.0,
                        tado::CallForHeatValue::Low => 33.0,
                        tado::CallForHeatValue::Medium => 66.0,
                        tado::CallForHeatValue::High => 100.0,
                    };
                    let entry = by_ts.entry(ts).or_insert_with(|| new_row(ts, db_home_id, db_zone_id));
                    entry.heating_power_pct = Some(pct);
                }
            }
        }

        if let Some(ac) = report.ac_activity.and_then(|s| s.data_intervals) {
            for di in ac {
                if let (Some(ts), Some(val)) = (di.interval.from, di.value) {
                    if ts < from || ts >= to {
                        continue;
                    }
                    let on = matches!(val, tado::Power::On);
                    let entry = by_ts.entry(ts).or_insert_with(|| new_row(ts, db_home_id, db_zone_id));
                    entry.ac_power_on = Some(on);
                }
            }
        }

        if let Some(settings) = report.settings.and_then(|s| s.data_intervals) {
            for di in settings {
                if let Some(ts) = di.interval.from {
                    if ts < from || ts >= to {
                        continue;
                    }
                    if let Some(val) = di.value {
                        let setpoint = val.temperature.and_then(|t| t.celsius);
                        let ac_mode = val.mode.as_ref().and_then(|m| serde_enum_name(m));
                        let ac_on = val.power.map(|p| matches!(p, tado::Power::On));
                        let entry = by_ts.entry(ts).or_insert_with(|| new_row(ts, db_home_id, db_zone_id));
                        if let Some(sp) = setpoint {
                            entry.setpoint_temp_c = Some(sp);
                        }
                        if let Some(m) = ac_mode {
                            entry.ac_mode = Some(m);
                        }
                        if let Some(on) = ac_on {
                            entry.ac_power_on = Some(on);
                        }
                    }
                }
            }
        }

        let rows: Vec<NewClimateMeasurement> = by_ts.into_values().collect();
        if !rows.is_empty() {
            diesel::insert_into(C::climate_measurements)
                .values(&rows)
                .on_conflict((C::time, C::home_id, C::source, C::zone_id, C::device_id))
                .do_nothing()
                .execute(conn)
                .map_err(|e| format!("insert historical climate rows failed: {}", e))?;
        }

        cursor = cursor.succ_opt().unwrap_or_else(|| NaiveDate::MAX);
    }

    Ok(())
}

fn new_row(ts: DateTime<Utc>, db_home_id: i64, db_zone_id: i64) -> NewClimateMeasurement {
    NewClimateMeasurement {
        time: ts,
        home_id: db_home_id,
        zone_id: Some(db_zone_id),
        device_id: None,
        source: "historical".to_string(),
        inside_temp_c: None,
        humidity_pct: None,
        setpoint_temp_c: None,
        heating_power_pct: None,
        ac_power_on: None,
        ac_mode: None,
        window_open: None,
        battery_low: None,
        connection_up: None,
    }
}
