use crate::db::models::{event_source, NewClimateMeasurement, NewHome, NewWeatherMeasurement, NewZone};
use crate::schema;
use crate::services::ingest::{insert_climate_measurements, insert_weather_measurements};
use chrono::{DateTime, Datelike, Duration, Timelike, Utc, Weekday};
use diesel::prelude::*;
use diesel::PgConnection;
use log::info;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::f64::consts::PI;

const HOME_TADO_ID: i64 = 4_201_337;
const STEP_MINUTES: i64 = 15;
const ZONE_NAMES: [&str; 8] = [
    "Living Room",
    "Kitchen",
    "Bedroom 1",
    "Bedroom 2",
    "Home Office",
    "Bathroom",
    "Hallway",
    "Nursery",
];

pub fn run(conn: &mut PgConnection) -> Result<(), String> {
    let db_home_id = ensure_home(conn)?;
    let now = Utc::now();
    let start = align_to_step(now - Duration::days(365 * 5));
    let end = align_to_step(now);
    if start >= end {
        return Err("Fake data generator requires start earlier than end".to_string());
    }

    let zone_ids = ensure_zones(conn, db_home_id, start)?;
    let mut rng = SmallRng::seed_from_u64(0x0420_1337_DEAD_BEEFu64);

    info!(
        "Fake data: generating synthetic history for home {} from {} to {} (zones={})",
        HOME_TADO_ID,
        start,
        end,
        zone_ids.len()
    );

    let mut climate_batch = Vec::with_capacity(zone_ids.len() * samples_per_day());
    let mut weather_batch = Vec::with_capacity(samples_per_day());
    let mut inserted_climate: usize = 0;
    let mut inserted_weather: usize = 0;
    let mut ts = start;
    let mut current_day = start.date_naive();
    let step = Duration::minutes(STEP_MINUTES);
    let mut last_logged_month: Option<(i32, u32)> = None;

    while ts < end {
        if ts.hour() == 0 && ts.minute() == 0 {
            let month_key = (ts.year(), ts.month());
            if last_logged_month != Some(month_key) {
                let remaining_days = (end - ts).num_days().max(0);
                info!(
                    "Fake data: processing {:04}-{:02} (≈{} day(s) remaining)",
                    month_key.0, month_key.1, remaining_days
                );
                last_logged_month = Some(month_key);
            }
        }

        if ts.date_naive() != current_day {
            flush_batches(
                conn,
                &mut climate_batch,
                &mut weather_batch,
                &mut inserted_climate,
                &mut inserted_weather,
            )?;
            current_day = ts.date_naive();
        }

        let day_fraction = ts.time().num_seconds_from_midnight() as f64 / 86_400.0;
        let annual_fraction = ts.ordinal0() as f64 / 365.0;
        let monthly_fraction = (ts.day0() % 30) as f64 / 30.0;
        let weekday = ts.weekday();

        let outside_temp = compute_outside_temp(day_fraction, annual_fraction, monthly_fraction, weekday, &mut rng);
        let solar_intensity = compute_solar_intensity(day_fraction, annual_fraction, weekday, &mut rng);
        let weather_state = classify_weather(outside_temp, solar_intensity, &mut rng);

        let mut weather_row = NewWeatherMeasurement::new(ts, db_home_id, event_source::HISTORICAL);
        weather_row.outside_temp_c = Some(outside_temp);
        weather_row.solar_intensity_pct = Some(solar_intensity);
        weather_row.weather_state = Some(weather_state.clone());
        weather_batch.push(weather_row);

        for (index, zone_id) in zone_ids.iter().enumerate() {
            let zone_index = index as f64;
            let setpoint = compute_setpoint(zone_index, monthly_fraction, day_fraction, weekday, &mut rng);
            let inside_temp = compute_inside_temp(setpoint, outside_temp, day_fraction, zone_index, weekday, &mut rng);
            let humidity = compute_humidity(outside_temp, annual_fraction, zone_index, weekday, &mut rng);
            let heating_power_pct =
                compute_heating_power(setpoint, inside_temp, solar_intensity, day_fraction, weekday, &mut rng);

            let mut row = NewClimateMeasurement::new(ts, db_home_id, Some(*zone_id), None, event_source::HISTORICAL);
            row.inside_temp_c = Some(inside_temp);
            row.humidity_pct = Some(humidity);
            row.setpoint_temp_c = Some(setpoint);
            row.heating_power_pct = Some(heating_power_pct);
            row.connection_up = Some(true);
            row.battery_low = Some(false);

            climate_batch.push(row);
        }

        ts += step;
    }

    flush_batches(
        conn,
        &mut climate_batch,
        &mut weather_batch,
        &mut inserted_climate,
        &mut inserted_weather,
    )?;

    let total_days = (end - start).num_days();
    info!(
        "Fake data: complete (days={}, climate_inserts={}, weather_inserts={})",
        total_days, inserted_climate, inserted_weather
    );

    Ok(())
}

fn ensure_home(conn: &mut PgConnection) -> Result<i64, String> {
    use schema::homes::dsl as H;

    let new_home = NewHome {
        tado_home_id: HOME_TADO_ID,
        name: Some("Chez Villa".to_string()),
        timezone: Some("Etc/UTC".to_string()),
        temperature_unit: Some("CELSIUS".to_string()),
        address_line1: Some("123 Down Town Abbey".to_string()),
        address_line2: None,
        zip_code: Some("RAR".to_string()),
        city: Some("Townsville".to_string()),
        state: Some("Of Trance".to_string()),
        country: Some("Promised Land".to_string()),
        latitude: Some(51.5074),
        longitude: Some(-0.1278),
    };

    diesel::insert_into(H::homes)
        .values(&new_home)
        .on_conflict(H::tado_home_id)
        .do_update()
        .set((
            H::name.eq(new_home.name.clone()),
            H::timezone.eq(new_home.timezone.clone()),
            H::temperature_unit.eq(new_home.temperature_unit.clone()),
            H::address_line1.eq(new_home.address_line1.clone()),
            H::address_line2.eq(new_home.address_line2.clone()),
            H::zip_code.eq(new_home.zip_code.clone()),
            H::city.eq(new_home.city.clone()),
            H::state.eq(new_home.state.clone()),
            H::country.eq(new_home.country.clone()),
            H::latitude.eq(new_home.latitude),
            H::longitude.eq(new_home.longitude),
            H::updated_at.eq(Utc::now()),
        ))
        .execute(conn)
        .map_err(|e| format!("insert home failed: {}", e))?;

    H::homes
        .filter(H::tado_home_id.eq(HOME_TADO_ID))
        .select(H::id)
        .first(conn)
        .map_err(|e| format!("fetch home id failed: {}", e))
}

fn ensure_zones(conn: &mut PgConnection, db_home_id: i64, start: DateTime<Utc>) -> Result<Vec<i64>, String> {
    use schema::zones::dsl as Z;

    for (index, name) in ZONE_NAMES.iter().enumerate() {
        let zone_tado_id = (index as i64) + 1;
        let new_zone = NewZone {
            home_id: db_home_id,
            tado_zone_id: zone_tado_id,
            name: Some((*name).to_string()),
            zone_type: Some("HEATING".to_string()),
            date_created: Some(start),
        };

        diesel::insert_into(Z::zones)
            .values(&new_zone)
            .on_conflict((Z::home_id, Z::tado_zone_id))
            .do_update()
            .set((
                Z::name.eq(new_zone.name.clone()),
                Z::zone_type.eq(new_zone.zone_type.clone()),
                Z::date_created.eq(new_zone.date_created),
                Z::updated_at.eq(Utc::now()),
            ))
            .execute(conn)
            .map_err(|e| format!("insert zone failed: {}", e))?;
    }

    let rows: Vec<(i64, i64)> = Z::zones
        .filter(Z::home_id.eq(db_home_id))
        .select((Z::tado_zone_id, Z::id))
        .load(conn)
        .map_err(|e| format!("fetch zones failed: {}", e))?;

    let mut map = Vec::with_capacity(ZONE_NAMES.len());
    for (index, _) in ZONE_NAMES.iter().enumerate() {
        let zone_tado_id = (index as i64) + 1;
        let db_id = rows
            .iter()
            .find(|(tid, _)| *tid == zone_tado_id)
            .map(|(_, id)| *id)
            .ok_or_else(|| format!("zone {} missing after upsert", zone_tado_id))?;
        map.push(db_id);
    }
    Ok(map)
}

fn flush_batches(
    conn: &mut PgConnection,
    climate_batch: &mut Vec<NewClimateMeasurement>,
    weather_batch: &mut Vec<NewWeatherMeasurement>,
    inserted_climate: &mut usize,
    inserted_weather: &mut usize,
) -> Result<(), String> {
    if !climate_batch.is_empty() {
        let inserted = insert_climate_measurements(conn, climate_batch)?;
        *inserted_climate += inserted;
        climate_batch.clear();
    }
    if !weather_batch.is_empty() {
        let inserted = insert_weather_measurements(conn, weather_batch)?;
        *inserted_weather += inserted;
        weather_batch.clear();
    }
    Ok(())
}

fn align_to_step(ts: DateTime<Utc>) -> DateTime<Utc> {
    let step_seconds = STEP_MINUTES * 60;
    let aligned = (ts.timestamp() / step_seconds) * step_seconds;
    DateTime::<Utc>::from_timestamp(aligned, 0).expect("valid timestamp")
}

fn samples_per_day() -> usize {
    (24 * 60 / STEP_MINUTES) as usize
}

fn compute_outside_temp(
    day_fraction: f64,
    annual_fraction: f64,
    monthly_fraction: f64,
    weekday: Weekday,
    rng: &mut SmallRng,
) -> f64 {
    let seasonal = (annual_fraction * 2.0 * PI).sin() * 12.0;
    let monthly = (monthly_fraction * 2.0 * PI).sin() * 3.0;
    let diurnal = ((day_fraction - 0.3) * 2.0 * PI).sin() * 5.0;
    let weekend_bias = if is_weekend(weekday) { 0.8 } else { 0.0 };
    let random_variation = rng.random_range(-1.8..=1.8);
    let cold_front = if rng.random_bool(0.02) {
        -rng.random_range(2.0..=5.0)
    } else {
        0.0
    };
    (8.5 + seasonal + monthly + diurnal + weekend_bias + random_variation + cold_front).clamp(-12.0, 34.0)
}

fn compute_solar_intensity(day_fraction: f64, annual_fraction: f64, weekday: Weekday, rng: &mut SmallRng) -> f64 {
    let daylight = ((day_fraction - 0.5) * PI * 2.0).cos().max(0.0);
    let seasonal = ((annual_fraction - 0.2) * 2.0 * PI).cos().max(0.15);
    let weekend_linger = if is_weekend(weekday) { 1.05 } else { 1.0 };
    let cloud_cover = rng.random_range(0.35..=1.0);
    (daylight * seasonal * 100.0 * weekend_linger * cloud_cover).clamp(0.0, 100.0)
}

fn classify_weather(outside_temp: f64, solar_intensity: f64, rng: &mut SmallRng) -> String {
    let precipitation_roll: f64 = rng.random_range(0.0..1.0);
    if solar_intensity > 70.0 && precipitation_roll > 0.25 {
        "SUN".to_string()
    } else if outside_temp < -1.5 {
        if precipitation_roll > 0.5 {
            "SCATTERED_SNOW".to_string()
        } else {
            "SNOW".to_string()
        }
    } else if solar_intensity < 22.0 {
        if precipitation_roll > 0.65 {
            "RAIN".to_string()
        } else if precipitation_roll > 0.35 {
            "SCATTERED_RAIN".to_string()
        } else {
            "CLOUDY".to_string()
        }
    } else if solar_intensity < 45.0 {
        "CLOUDY_MOSTLY".to_string()
    } else {
        "CLOUDY_PARTLY".to_string()
    }
}

fn compute_setpoint(
    zone_index: f64,
    monthly_fraction: f64,
    day_fraction: f64,
    weekday: Weekday,
    rng: &mut SmallRng,
) -> f64 {
    let base = 20.2 + zone_index * 0.35;
    let monthly = (monthly_fraction * 2.0 * PI).sin() * 0.8;
    let routine = routine_profile(day_fraction, weekday) * 1.2;
    let weekend_bonus = if is_weekend(weekday) { 0.7 } else { 0.0 };
    let random = rng.random_range(-0.45..=0.45);
    (base + monthly + weekend_bonus + routine + random).clamp(17.5, 24.5)
}

fn compute_inside_temp(
    setpoint: f64,
    outside_temp: f64,
    day_fraction: f64,
    zone_index: f64,
    weekday: Weekday,
    rng: &mut SmallRng,
) -> f64 {
    let infiltration = (setpoint - outside_temp).max(0.0) * rng.random_range(0.12..=0.22);
    let diurnal = ((day_fraction - 0.1) * 2.0 * PI).sin() * 0.8;
    let zone_bias = ((zone_index + 1.0).sin()) * 0.5;
    let routine_gain = routine_profile(day_fraction, weekday) * 0.6;
    let random = rng.random_range(-0.6..=0.6);
    let weekend_relax = if is_weekend(weekday) { 0.35 } else { 0.0 };
    (setpoint - infiltration + diurnal + zone_bias + routine_gain + weekend_relax + random).clamp(15.0, 26.5)
}

fn compute_humidity(
    outside_temp: f64,
    annual_fraction: f64,
    zone_index: f64,
    weekday: Weekday,
    rng: &mut SmallRng,
) -> f64 {
    let seasonal = ((annual_fraction + 0.1) * 2.0 * PI).cos() * 10.0;
    let temperature_component = (18.0 - outside_temp).clamp(-12.0, 12.0) * 0.8;
    let zone_bias = ((zone_index * 1.7).sin()) * 3.0;
    let weekend_shift = if is_weekend(weekday) { 3.0 } else { -1.0 };
    let random = rng.random_range(-6.0..=6.0);
    (50.0 + seasonal + temperature_component + zone_bias + weekend_shift + random).clamp(30.0, 75.0)
}

fn compute_heating_power(
    setpoint: f64,
    inside_temp: f64,
    solar_intensity: f64,
    day_fraction: f64,
    weekday: Weekday,
    rng: &mut SmallRng,
) -> f64 {
    let deficit = (setpoint - inside_temp).max(0.0);
    let baseline = deficit * 55.0;
    let routine = routine_profile(day_fraction, weekday) * 12.0;
    let solar_relief = solar_intensity * 0.18;
    let random = rng.random_range(0.0..=12.0);
    (baseline + routine + random - solar_relief).clamp(0.0, 100.0)
}

fn routine_profile(day_fraction: f64, weekday: Weekday) -> f64 {
    let morning_peak = gaussian(day_fraction, 0.27, 0.045) * 1.8;
    let evening_peak = gaussian(day_fraction, 0.77, 0.05) * 2.0;
    let commute_gap = if is_weekend(weekday) {
        0.5 * gaussian(day_fraction, 0.45, 0.09)
    } else {
        -0.4 * gaussian(day_fraction, 0.5, 0.12)
    };
    let weekend_brunch = if is_weekend(weekday) {
        gaussian(day_fraction, 0.35, 0.08) * 1.0
    } else {
        0.0
    };
    (morning_peak + evening_peak + commute_gap + weekend_brunch).max(0.0)
}

fn gaussian(x: f64, center: f64, width: f64) -> f64 {
    let exponent = -((x - center) * (x - center)) / (2.0 * width * width);
    exponent.exp()
}

fn is_weekend(weekday: Weekday) -> bool {
    matches!(weekday, Weekday::Sat | Weekday::Sun)
}
