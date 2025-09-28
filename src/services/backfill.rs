use crate::client::{TadoClient, TadoClientError};
use crate::db::models::event_source;
use crate::db::models::{NewClimateMeasurement, NewWeatherMeasurement};
use crate::models::tado::{self, HomeId, ZoneId};
use crate::schema;
use crate::utils::{determine_zone_start_time, serde_enum_name};
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveTime, SecondsFormat, Utc};
use diesel::dsl::max;
use diesel::prelude::*;
use diesel::PgConnection;
use log::{debug, info, warn};
use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::thread;
use std::time::{Duration as StdDuration, Instant};

const BOGUS_TEMP_C: f64 = 20.0;
const BOGUS_HUMIDITY_FRACTION: f64 = 0.5; // as delivered by the API (UNIT_INTERVAL)
const BOGUS_HUMIDITY_PERCENT: f64 = 50.0; // after we scale to percentages for inserts
const FLOAT_EPSILON: f64 = 1e-6;

fn approx_eq(lhs: f64, rhs: f64) -> bool {
    (lhs - rhs).abs() <= FLOAT_EPSILON
}

#[derive(Debug, Clone)]
struct Gap {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    start_inclusive: bool,
}

fn format_gap_range(gap: &Gap) -> String {
    let start_bracket = if gap.start_inclusive { '[' } else { '(' };
    let start = gap.start.to_rfc3339_opts(SecondsFormat::Secs, true);
    let end = gap.end.to_rfc3339_opts(SecondsFormat::Secs, true);
    format!("{open}{start}, {end})", open = start_bracket, start = start, end = end)
}

fn format_gap_ranges_for_log(gaps: &[Gap]) -> String {
    gaps.iter().map(format_gap_range).collect::<Vec<_>>().join(", ")
}

fn log_gap_summary(zone_id: ZoneId, gaps_by_day: &BTreeMap<NaiveDate, Vec<Gap>>) {
    for (day, ranges) in gaps_by_day {
        info!(
            "Backfill: zone {} day {} requires backfill for {}",
            zone_id.0,
            day,
            format_gap_ranges_for_log(ranges)
        );
    }
}

fn timestamp_in_any_gap(ts: DateTime<Utc>, gaps: &[Gap]) -> bool {
    gaps.iter().any(|gap| {
        let lower_ok = if gap.start_inclusive {
            ts >= gap.start
        } else {
            ts > gap.start
        };
        lower_ok && ts < gap.end
    })
}

fn is_day_report_bogus(report: &tado::DayReport) -> bool {
    let mut indoor_had_data = false;
    let mut indoor_has_real_signal = false;

    if let Some(md) = report.measured_data.as_ref() {
        if let Some(points) = md
            .inside_temperature
            .as_ref()
            .and_then(|series| series.data_points.as_ref())
        {
            for point in points {
                if let Some(value) = point.value.as_ref().and_then(|t| t.celsius) {
                    indoor_had_data = true;
                    if !approx_eq(value, BOGUS_TEMP_C) {
                        indoor_has_real_signal = true;
                        break;
                    }
                }
            }
        }

        if !indoor_has_real_signal
            && let Some(points) = md.humidity.as_ref().and_then(|series| series.data_points.as_ref())
        {
            for point in points {
                if let Some(value) = point.value {
                    indoor_had_data = true;
                    if !approx_eq(value, BOGUS_HUMIDITY_FRACTION) {
                        indoor_has_real_signal = true;
                        break;
                    }
                }
            }
        }
    }

    if indoor_has_real_signal {
        return false;
    }

    let mut outdoor_had_data = false;
    let mut outdoor_has_real_signal = false;

    if let Some(weather) = report.weather.as_ref() {
        if let Some(intervals) = weather
            .condition
            .as_ref()
            .and_then(|series| series.data_intervals.as_ref())
        {
            for interval in intervals {
                if interval.value.is_some() {
                    outdoor_had_data = true;
                    if let Some(value) = interval.value.as_ref() {
                        let has_temp = value.temperature.as_ref().and_then(|temp| temp.celsius).is_some();
                        let has_state = value.state.is_some();
                        if has_temp || has_state {
                            outdoor_has_real_signal = true;
                            break;
                        }
                    }
                } else {
                    outdoor_had_data = true;
                }
            }
        }

        if !outdoor_has_real_signal && let Some(slots) = weather.slots.as_ref().and_then(|series| series.slots.as_ref())
        {
            for slot in slots.values() {
                outdoor_had_data = true;
                if slot.is_some() {
                    outdoor_has_real_signal = true;
                    break;
                }
            }
        }
    }

    let indoor_bogus = indoor_had_data && !indoor_has_real_signal;
    let outdoor_bogus = outdoor_had_data && !outdoor_has_real_signal;

    indoor_bogus && outdoor_bogus
}

fn measurement_is_leading_bogus(row: &NewClimateMeasurement) -> bool {
    match (row.inside_temp_c, row.humidity_pct) {
        (Some(temp), Some(humidity))
            if approx_eq(temp, BOGUS_TEMP_C) && approx_eq(humidity, BOGUS_HUMIDITY_PERCENT) => {}
        _ => return false,
    }

    row.setpoint_temp_c.is_none()
        && row.heating_power_pct.is_none()
        && row.ac_power_on.is_none()
        && row.ac_mode.is_none()
        && row.window_open.is_none()
        && row.battery_low.is_none()
        && row.connection_up.is_none()
}

fn remove_leading_bogus_rows(rows: &mut BTreeMap<DateTime<Utc>, NewClimateMeasurement>) {
    let mut to_remove = Vec::new();
    for (ts, row) in rows.iter() {
        if measurement_is_leading_bogus(row) {
            to_remove.push(*ts);
        } else {
            break;
        }
    }

    for ts in to_remove {
        rows.remove(&ts);
    }
}

pub fn run_for_home(
    conn: &mut PgConnection,
    client: &TadoClient,
    home_id: HomeId,
    backfill_from_date: Option<NaiveDate>,
    backfill_requests_per_second: Option<NonZeroU32>,
    backfill_sample_rate: Option<NonZeroU32>,
    min_gap: Duration,
) -> Result<(), String> {
    // Fetch zones to decide backfill per zone
    let zones = client
        .get_zones(home_id)
        .map_err(|e| format!("get_zones({}) failed: {}", home_id.0, e))?;
    info!("Backfill: home {} has {} zone(s)", home_id.0, zones.len());

    // Resolve DB home id
    let db_home_id: i64 = schema::homes::dsl::homes
        .filter(schema::homes::dsl::tado_home_id.eq(home_id.0))
        .select(schema::homes::dsl::id)
        .first(conn)
        .map_err(|e| format!("fetch db_home_id failed: {}", e))?;

    // Map of tado zone id -> db zone id (only those with date_created)
    let mut zone_id_map = BTreeMap::new();
    // Compute lower bound for historical collection for this home, if requested
    let min_start_dt_utc: Option<DateTime<Utc>> = backfill_from_date.map(|d| d.and_time(NaiveTime::MIN).and_utc());

    // Compute weather backfill window once per home (avoid extra API calls later),
    // clamping the start to the configured minimum date when provided.
    let reference_zone = select_reference_zone_and_start(&zones);
    if reference_zone.is_none() {
        warn!(
            "Backfill: home {} has no zone with a date_created timestamp; skipping weather backfill",
            home_id.0
        );
    }
    let weather_window = reference_zone
        .map(|(_, home_start)| {
            let effective_start = match min_start_dt_utc {
                Some(min_dt) if home_start < min_dt => min_dt,
                _ => home_start,
            };
            compute_weather_backfill_window(conn, db_home_id, effective_start)
        })
        .transpose()?;

    for z in &zones {
        let Some(zid) = z.id else {
            let name = z.name.as_deref().unwrap_or("-");
            warn!("Backfill: skipping zone without id (name=\"{}\")", name);
            continue;
        };
        if z.date_created.is_none() {
            warn!("Backfill: skipping zone {} (missing date_created timestamp)", zid.0);
            continue;
        }

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
    debug!(
        "Backfill: home {} eligible zones with date_created: {}",
        home_id.0,
        zone_id_map.len()
    );

    let day_report_spacing =
        backfill_requests_per_second.map(|limit| StdDuration::from_secs_f64(1.0 / limit.get() as f64));
    let day_report_sample_rate = backfill_sample_rate;

    for z in &zones {
        let Some(zone_id) = z.id else {
            continue;
        };
        if z.date_created.is_none() {
            continue;
        }
        let Some(db_zone_id) = zone_id_map.get(&zone_id.0).copied() else {
            warn!("Backfill: zone {} not found in database mapping; skipping", zone_id.0);
            continue;
        };
        let start = determine_zone_start_time(&zones, zone_id)
            .map_err(|e| format!("determine start time failed for zone {}: {}", zone_id.0, e))?;
        let start = match min_start_dt_utc {
            Some(min_dt) if start < min_dt => min_dt,
            _ => start,
        };
        let gaps_by_day = find_zone_gaps(conn, db_home_id, db_zone_id, start, min_gap)?;
        if gaps_by_day.is_empty() {
            debug!(
                "Backfill: zone {} has no >={}min gaps after {}",
                zone_id.0,
                min_gap.num_minutes(),
                start
            );
            continue;
        }

        let total_gap_hours: f64 = gaps_by_day
            .values()
            .flat_map(|ranges| ranges.iter())
            .map(|gap| (gap.end - gap.start).num_minutes() as f64 / 60.0)
            .sum();

        info!(
            "Backfill: home {} zone {} has {} day(s) with gaps (≈{:.1}h)",
            home_id.0,
            zone_id.0,
            gaps_by_day.len(),
            total_gap_hours
        );

        log_gap_summary(zone_id, &gaps_by_day);

        backfill_zone_range(
            conn,
            client,
            home_id,
            db_home_id,
            zone_id,
            db_zone_id,
            weather_window,
            day_report_spacing,
            day_report_sample_rate,
            &gaps_by_day,
        )?;
    }

    Ok(())
}

fn find_zone_gaps(
    conn: &mut PgConnection,
    db_home_id: i64,
    db_zone_id: i64,
    start: DateTime<Utc>,
    min_gap: Duration,
) -> Result<BTreeMap<NaiveDate, Vec<Gap>>, String> {
    use schema::climate_measurements::dsl as C;

    let mut gaps: BTreeMap<NaiveDate, Vec<Gap>> = BTreeMap::new();
    let now = Utc::now();
    if start >= now {
        return Ok(gaps);
    }

    let times: Vec<DateTime<Utc>> = C::climate_measurements
        .filter(
            C::home_id
                .eq(db_home_id)
                .and(C::zone_id.eq(db_zone_id))
                .and(C::time.ge(start))
                .and(C::time.lt(now)),
        )
        .select(C::time)
        .order(C::time.asc())
        .load(conn)
        .map_err(|e| format!("query measurement timestamps failed: {}", e))?;

    let mut cursor_date = start.date_naive();
    let end_date = now.date_naive();
    let mut idx = 0usize;

    while cursor_date <= end_date {
        let day_start = cursor_date.and_time(NaiveTime::MIN).and_utc();
        let mut effective_start = day_start;
        if cursor_date == start.date_naive() && start > day_start {
            effective_start = start;
        }

        let day_end = if cursor_date == end_date {
            now
        } else {
            cursor_date
                .succ_opt()
                .unwrap_or(NaiveDate::MAX)
                .and_time(NaiveTime::MIN)
                .and_utc()
        };

        if day_end <= effective_start {
            if let Some(next) = cursor_date.succ_opt() {
                cursor_date = next;
                continue;
            } else {
                break;
            }
        }

        while idx < times.len() && times[idx] < effective_start {
            idx += 1;
        }

        let day_start_idx = idx;
        while idx < times.len() && times[idx] < day_end {
            idx += 1;
        }
        let day_times = &times[day_start_idx..idx];

        let mut day_gaps: Vec<Gap> = Vec::new();

        if day_times.is_empty() {
            if day_end - effective_start >= min_gap {
                day_gaps.push(Gap {
                    start: effective_start,
                    end: day_end,
                    start_inclusive: true,
                });
            }
        } else {
            let first = day_times[0];
            if first - effective_start >= min_gap {
                day_gaps.push(Gap {
                    start: effective_start,
                    end: first,
                    start_inclusive: true,
                });
            }

            let mut prev = first;
            for &ts in &day_times[1..] {
                if ts - prev >= min_gap {
                    day_gaps.push(Gap {
                        start: prev,
                        end: ts,
                        start_inclusive: false,
                    });
                }
                prev = ts;
            }

            if day_end - prev >= min_gap {
                day_gaps.push(Gap {
                    start: prev,
                    end: day_end,
                    start_inclusive: false,
                });
            }
        }

        if !day_gaps.is_empty() {
            gaps.insert(cursor_date, day_gaps);
        }

        match cursor_date.succ_opt() {
            Some(next) => cursor_date = next,
            None => break,
        }
    }

    Ok(gaps)
}

fn compute_weather_backfill_window(
    conn: &mut PgConnection,
    db_home_id: i64,
    start: DateTime<Utc>,
) -> Result<(DateTime<Utc>, DateTime<Utc>), String> {
    use schema::weather_measurements::dsl as W;
    let last_any: Option<DateTime<Utc>> = W::weather_measurements
        .filter(W::home_id.eq(db_home_id))
        .select(max(W::time))
        .first(conn)
        .map_err(|e| format!("query last weather timestamp failed: {}", e))?;
    let base_from = last_any.map(|t| t + chrono::Duration::seconds(1)).unwrap_or(start);
    let from = base_from.max(start);
    let to = Utc::now();
    Ok((from, to))
}

fn find_first_non_bogus_day(
    client: &TadoClient,
    home_id: HomeId,
    zone_id: ZoneId,
    start: NaiveDate,
    end: NaiveDate,
    min_spacing: Option<StdDuration>,
) -> Result<Option<NaiveDate>, String> {
    if start > end {
        return Ok(None);
    }

    info!(
        "Backfill: probing historical signal for zone {} between {} and {}",
        zone_id.0, start, end
    );

    let total_days = end.signed_duration_since(start).num_days().max(0);

    let mut low: i64 = 0;
    let mut high: i64 = total_days;
    let mut candidate: Option<NaiveDate> = None;

    while low <= high {
        let mid = low + (high - low) / 2;
        let day = start + Duration::days(mid);
        let report = fetch_day_report_with_limit(client, home_id, zone_id, day, min_spacing).map_err(|e| {
            format!(
                "get_zone_day_report({}, {}, {}) failed: {}",
                home_id.0, zone_id.0, day, e
            )
        })?;

        if is_day_report_bogus(&report) {
            low = mid + 1;
        } else {
            candidate = Some(day);
            if mid == 0 {
                break;
            }
            high = mid - 1;
        }
    }

    match candidate {
        Some(day) => info!("Backfill: first non-bogus day for zone {} found at {}", zone_id.0, day),
        None => info!(
            "Backfill: no non-bogus historical data detected for zone {} in requested range",
            zone_id.0
        ),
    }

    Ok(candidate)
}

fn select_reference_zone_and_start(zones: &[tado::Zone]) -> Option<(ZoneId, DateTime<Utc>)> {
    // Choose the zone with the earliest creation date as reference; ensures widest history.
    let mut best: Option<(ZoneId, DateTime<Utc>)> = None;
    for z in zones {
        if let (Some(zid), Some(created)) = (z.id, z.date_created) {
            match best {
                None => best = Some((zid, created)),
                Some((_, best_created)) if created < best_created => best = Some((zid, created)),
                _ => {}
            }
        }
    }
    best
}

#[allow(clippy::too_many_arguments)]
fn backfill_zone_range(
    conn: &mut PgConnection,
    client: &TadoClient,
    home_id: HomeId,
    db_home_id: i64,
    zone_id: ZoneId,
    db_zone_id: i64,
    weather_window: Option<(DateTime<Utc>, DateTime<Utc>)>,
    day_report_spacing: Option<StdDuration>,
    day_report_sample_rate: Option<NonZeroU32>,
    gaps_by_day: &BTreeMap<NaiveDate, Vec<Gap>>,
) -> Result<(), String> {
    use schema::climate_measurements::dsl as C;
    use schema::weather_measurements::dsl as W;

    if gaps_by_day.is_empty() {
        return Ok(());
    }

    let first_gap_day = *gaps_by_day.keys().next().unwrap();
    let last_gap_day = *gaps_by_day.keys().next_back().unwrap();

    let first_valid_day = find_first_non_bogus_day(
        client,
        home_id,
        zone_id,
        first_gap_day,
        last_gap_day,
        day_report_spacing,
    )?;

    let Some(first_day) = first_valid_day else {
        info!(
            "Backfill: zone {} has only bogus historical data between {} and {}; skipping",
            zone_id.0, first_gap_day, last_gap_day
        );
        return Ok(());
    };

    let mut inserted_total: usize = 0;
    let mut processed_days: u64 = 0;

    for (day, gaps) in gaps_by_day {
        if *day < first_day {
            continue;
        }
        if gaps.is_empty() {
            continue;
        }

        if let Some(rate) = day_report_sample_rate
            && *day != first_day
            && day.ordinal() % rate.get() != 0
        {
            continue;
        }

        let report = fetch_day_report_with_limit(client, home_id, zone_id, *day, day_report_spacing).map_err(|e| {
            format!(
                "get_zone_day_report({}, {}, {}) failed: {}",
                home_id.0, zone_id.0, day, e
            )
        })?;
        processed_days += 1;

        let mut by_ts: BTreeMap<DateTime<Utc>, NewClimateMeasurement> = BTreeMap::new();
        let mut weather_by_ts: BTreeMap<DateTime<Utc>, NewWeatherMeasurement> = BTreeMap::new();

        if let Some(md) = report.measured_data.as_ref() {
            if let Some(temp_series) = md.inside_temperature.as_ref().and_then(|s| s.data_points.as_ref()) {
                for dp in temp_series {
                    if let (Some(ts), Some(val)) = (
                        dp.timestamp.as_ref().cloned(),
                        dp.value.as_ref().and_then(|t| t.celsius),
                    ) {
                        if !timestamp_in_any_gap(ts, gaps) {
                            continue;
                        }
                        let entry = by_ts.entry(ts).or_insert_with(|| {
                            NewClimateMeasurement::new(ts, db_home_id, Some(db_zone_id), None, event_source::HISTORICAL)
                        });
                        entry.inside_temp_c = Some(val);
                    }
                }
            }
            if let Some(h_series) = md.humidity.as_ref().and_then(|s| s.data_points.as_ref()) {
                for dp in h_series {
                    if let (Some(ts), Some(val)) = (dp.timestamp.as_ref().cloned(), dp.value) {
                        if !timestamp_in_any_gap(ts, gaps) {
                            continue;
                        }
                        let entry = by_ts.entry(ts).or_insert_with(|| {
                            NewClimateMeasurement::new(ts, db_home_id, Some(db_zone_id), None, event_source::HISTORICAL)
                        });
                        entry.humidity_pct = Some(val * 100.0);
                    }
                }
            }
            if let Some(conn_series) = md
                .measuring_device_connected
                .as_ref()
                .and_then(|s| s.data_intervals.as_ref())
            {
                for di in conn_series {
                    if let (Some(ts), Some(val)) = (di.interval.from.as_ref().cloned(), di.value) {
                        if !timestamp_in_any_gap(ts, gaps) {
                            continue;
                        }
                        let entry = by_ts.entry(ts).or_insert_with(|| {
                            NewClimateMeasurement::new(ts, db_home_id, Some(db_zone_id), None, event_source::HISTORICAL)
                        });
                        entry.connection_up = Some(val);
                    }
                }
            }
        }

        if let Some(cf) = report.call_for_heat.as_ref().and_then(|s| s.data_intervals.as_ref()) {
            for di in cf {
                if let (Some(ts), Some(val)) = (di.interval.from.as_ref().cloned(), di.value) {
                    if !timestamp_in_any_gap(ts, gaps) {
                        continue;
                    }
                    let pct = match val {
                        tado::CallForHeatValue::None_ => 0.0,
                        tado::CallForHeatValue::Low => 33.0,
                        tado::CallForHeatValue::Medium => 66.0,
                        tado::CallForHeatValue::High => 100.0,
                    };
                    let entry = by_ts.entry(ts).or_insert_with(|| {
                        NewClimateMeasurement::new(ts, db_home_id, Some(db_zone_id), None, event_source::HISTORICAL)
                    });
                    entry.heating_power_pct = Some(pct);
                }
            }
        }

        if let Some(ac) = report.ac_activity.as_ref().and_then(|s| s.data_intervals.as_ref()) {
            for di in ac {
                if let (Some(ts), Some(val)) = (di.interval.from.as_ref().cloned(), di.value) {
                    if !timestamp_in_any_gap(ts, gaps) {
                        continue;
                    }
                    let on = matches!(val, tado::Power::On);
                    let entry = by_ts.entry(ts).or_insert_with(|| {
                        NewClimateMeasurement::new(ts, db_home_id, Some(db_zone_id), None, event_source::HISTORICAL)
                    });
                    entry.ac_power_on = Some(on);
                }
            }
        }

        if let Some(settings) = report.settings.as_ref().and_then(|s| s.data_intervals.as_ref()) {
            for di in settings {
                if let Some(ts) = di.interval.from.as_ref().cloned() {
                    if !timestamp_in_any_gap(ts, gaps) {
                        continue;
                    }
                    if let Some(val) = di.value.as_ref() {
                        let setpoint = val.temperature.as_ref().and_then(|t| t.celsius);
                        let ac_mode = val.mode.as_ref().and_then(serde_enum_name);
                        let ac_on = val.power.map(|p| matches!(p, tado::Power::On));
                        let entry = by_ts.entry(ts).or_insert_with(|| {
                            NewClimateMeasurement::new(ts, db_home_id, Some(db_zone_id), None, event_source::HISTORICAL)
                        });
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

        if let Some((w_from, w_to)) = weather_window
            && let Some(w) = report.weather.as_ref()
            && let Some(cond) = w.condition.as_ref().and_then(|ts| ts.data_intervals.as_ref())
        {
            for di in cond {
                if let Some(ts) = di.interval.from.as_ref().cloned() {
                    if ts < w_from || ts >= w_to || !timestamp_in_any_gap(ts, gaps) {
                        continue;
                    }
                    let entry = weather_by_ts
                        .entry(ts)
                        .or_insert_with(|| NewWeatherMeasurement::new(ts, db_home_id, event_source::HISTORICAL));
                    if let Some(v) = di.value.as_ref() {
                        if let Some(temp) = v.temperature.as_ref().and_then(|t| t.celsius) {
                            entry.outside_temp_c = Some(temp);
                        }
                        if let Some(state) = v.state.as_ref().and_then(serde_enum_name) {
                            entry.weather_state = Some(state);
                        }
                    }
                }
            }
        }

        remove_leading_bogus_rows(&mut by_ts);

        let rows: Vec<NewClimateMeasurement> = by_ts.into_values().collect();
        if !rows.is_empty() {
            let inserted = diesel::insert_into(C::climate_measurements)
                .values(&rows)
                .on_conflict((C::time, C::home_id, C::source, C::zone_id, C::device_id))
                .do_nothing()
                .execute(conn)
                .map_err(|e| format!("insert historical climate rows failed: {}", e))?;
            inserted_total += inserted as usize;
        }

        if !weather_by_ts.is_empty() {
            let rows: Vec<NewWeatherMeasurement> = weather_by_ts.into_values().collect();
            let _ = diesel::insert_into(W::weather_measurements)
                .values(&rows)
                .on_conflict((W::home_id, W::time, W::source))
                .do_nothing()
                .execute(conn)
                .map_err(|e| format!("insert historical weather rows failed: {}", e))?;
        }
    }

    info!(
        "Backfill: zone {} complete ({} day(s), {} row(s) inserted)",
        zone_id.0, processed_days, inserted_total
    );

    Ok(())
}

fn fetch_day_report_with_limit(
    client: &TadoClient,
    home_id: HomeId,
    zone_id: ZoneId,
    day: NaiveDate,
    min_spacing: Option<StdDuration>,
) -> Result<tado::DayReport, TadoClientError> {
    let start = Instant::now();
    let result = client.get_zone_day_report(home_id, zone_id, Some(day));
    if let Some(required) = min_spacing {
        let elapsed = start.elapsed();
        if elapsed < required {
            thread::sleep(required - elapsed);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::collections::BTreeMap;

    fn load_bogus_fixture() -> tado::DayReport {
        let json = std::fs::read_to_string("tests/data/day-report.json").expect("fixture present");
        serde_json::from_str(&json).expect("parse day report")
    }

    #[test]
    fn detects_bogus_fixture() {
        let report = load_bogus_fixture();
        assert!(is_day_report_bogus(&report));
    }

    #[test]
    fn detects_non_bogus_when_indoor_changes() {
        let mut report = load_bogus_fixture();
        let md = report
            .measured_data
            .as_mut()
            .and_then(|m| m.inside_temperature.as_mut())
            .and_then(|series| series.data_points.as_mut())
            .expect("fixture has inside temp data");
        if let Some(first) = md.first_mut()
            && let Some(temp) = first.value.as_mut()
        {
            temp.celsius = Some(19.5);
        }

        assert!(!is_day_report_bogus(&report));
    }

    #[test]
    fn removes_leading_bogus_rows_only() {
        let mut rows = BTreeMap::new();
        let ts1 = Utc.with_ymd_and_hms(2023, 1, 1, 0, 0, 0).unwrap();
        let mut first = NewClimateMeasurement::new(ts1, 1, Some(1), None, event_source::HISTORICAL);
        first.inside_temp_c = Some(BOGUS_TEMP_C);
        first.humidity_pct = Some(BOGUS_HUMIDITY_PERCENT);
        rows.insert(ts1, first);

        let ts2 = Utc.with_ymd_and_hms(2023, 1, 1, 0, 15, 0).unwrap();
        let mut second = NewClimateMeasurement::new(ts2, 1, Some(1), None, event_source::HISTORICAL);
        second.inside_temp_c = Some(20.8);
        second.humidity_pct = Some(55.0);
        rows.insert(ts2, second);

        remove_leading_bogus_rows(&mut rows);
        assert_eq!(rows.len(), 1);
        assert!(rows.contains_key(&ts2));
    }

    #[test]
    fn timestamp_gap_inclusion_rules() {
        let start = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let end = start + chrono::Duration::hours(2);

        let inclusive_gap = Gap {
            start,
            end,
            start_inclusive: true,
        };
        assert!(timestamp_in_any_gap(start, std::slice::from_ref(&inclusive_gap)));
        assert!(timestamp_in_any_gap(
            start + chrono::Duration::minutes(15),
            std::slice::from_ref(&inclusive_gap)
        ));
        assert!(!timestamp_in_any_gap(end, std::slice::from_ref(&inclusive_gap)));

        let exclusive_gap = Gap {
            start,
            end,
            start_inclusive: false,
        };
        assert!(!timestamp_in_any_gap(start, std::slice::from_ref(&exclusive_gap)));
        assert!(timestamp_in_any_gap(
            start + chrono::Duration::minutes(15),
            std::slice::from_ref(&exclusive_gap)
        ));
    }
}
