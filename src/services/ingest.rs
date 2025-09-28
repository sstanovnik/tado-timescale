use crate::db::models::{NewClimateMeasurement, NewWeatherMeasurement};
use crate::schema;
use diesel::prelude::*;
use diesel::PgConnection;

pub fn insert_climate_measurements(conn: &mut PgConnection, rows: &[NewClimateMeasurement]) -> Result<usize, String> {
    if rows.is_empty() {
        return Ok(0);
    }

    use schema::climate_measurements::dsl as C;

    diesel::insert_into(C::climate_measurements)
        .values(rows)
        .on_conflict((C::time, C::home_id, C::source, C::zone_id, C::device_id))
        .do_nothing()
        .execute(conn)
        .map(|count| count as usize)
        .map_err(|e| format!("insert climate rows failed: {}", e))
}

pub fn insert_weather_measurements(conn: &mut PgConnection, rows: &[NewWeatherMeasurement]) -> Result<usize, String> {
    if rows.is_empty() {
        return Ok(0);
    }

    use schema::weather_measurements::dsl as W;

    diesel::insert_into(W::weather_measurements)
        .values(rows)
        .on_conflict((W::home_id, W::time, W::source))
        .do_nothing()
        .execute(conn)
        .map(|count| count as usize)
        .map_err(|e| format!("insert weather rows failed: {}", e))
}
