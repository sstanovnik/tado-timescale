//! Diesel model structs representing application entities and time-series data.
//!
//! Important: Migrations will set up TimescaleDB hypertables for
//! `climate_measurements`, `weather_measurements`, and `events`.

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::schema;

// Useful constants for standardizing `events.event_type` and measurement `source`.
pub mod event_types {
    // Overlay lifecycle
    pub const OVERLAY_SET: &str = "OVERLAY_SET";
    pub const OVERLAY_CLEARED: &str = "OVERLAY_CLEARED";
    pub const OVERLAY_UPDATED: &str = "OVERLAY_UPDATED";

    // Open window lifecycle
    pub const OPEN_WINDOW_DETECTED: &str = "OPEN_WINDOW_DETECTED";
    pub const OPEN_WINDOW_CLOSED: &str = "OPEN_WINDOW_CLOSED";

    // Device lifecycle/health
    pub const DEVICE_CONNECTED: &str = "DEVICE_CONNECTED";
    pub const DEVICE_DISCONNECTED: &str = "DEVICE_DISCONNECTED";
    pub const DEVICE_BATTERY_LOW: &str = "DEVICE_BATTERY_LOW";
    pub const DEVICE_BATTERY_NORMAL: &str = "DEVICE_BATTERY_NORMAL";
    pub const DEVICE_FIRMWARE_UPDATED: &str = "DEVICE_FIRMWARE_UPDATED";
    pub const DEVICE_ADDED: &str = "DEVICE_ADDED";
    pub const DEVICE_REMOVED: &str = "DEVICE_REMOVED";
    pub const DEVICE_TEMPERATURE_OFFSET_CHANGED: &str = "DEVICE_TEMPERATURE_OFFSET_CHANGED";
    pub const DEVICE_MOUNTING_STATE_CHANGED: &str = "DEVICE_MOUNTING_STATE_CHANGED";
}

pub mod event_source {
    pub const REALTIME: &str = "realtime";
    pub const HISTORICAL: &str = "historical";
    pub const DERIVED: &str = "derived";
}

#[derive(Debug, Clone, Queryable, Identifiable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = schema::users)]
pub struct User {
    pub id: i64,
    pub tado_user_id: String,
    pub email: Option<String>,
    pub username: Option<String>,
    pub name: Option<String>,
    pub locale: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Insertable, Serialize, Deserialize)]
#[diesel(table_name = schema::users)]
pub struct NewUser {
    pub tado_user_id: String,
    pub email: Option<String>,
    pub username: Option<String>,
    pub name: Option<String>,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Queryable, Identifiable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = schema::homes)]
pub struct Home {
    pub id: i64,
    pub tado_home_id: i64,
    pub name: Option<String>,
    pub timezone: Option<String>,
    pub temperature_unit: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub zip_code: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub country: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Insertable, Serialize, Deserialize)]
#[diesel(table_name = schema::homes)]
pub struct NewHome {
    pub tado_home_id: i64,
    pub name: Option<String>,
    pub timezone: Option<String>,
    pub temperature_unit: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub zip_code: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub country: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

#[derive(Debug, Clone, Queryable, Identifiable, Associations, Selectable, Serialize, Deserialize)]
#[diesel(table_name = schema::user_homes)]
#[diesel(primary_key(user_id, home_id))]
#[diesel(belongs_to(User))]
#[diesel(belongs_to(Home))]
pub struct UserHome {
    pub user_id: i64,
    pub home_id: i64,
    pub role: Option<String>,
    pub joined_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Insertable, Serialize, Deserialize)]
#[diesel(table_name = schema::user_homes)]
pub struct NewUserHome {
    pub user_id: i64,
    pub home_id: i64,
    pub role: Option<String>,
    pub joined_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Queryable, Identifiable, Associations, Selectable, Serialize, Deserialize)]
#[diesel(table_name = schema::zones)]
#[diesel(belongs_to(Home))]
pub struct Zone {
    pub id: i64,
    pub home_id: i64,
    pub tado_zone_id: i64,
    pub name: Option<String>,
    pub zone_type: Option<String>,
    pub date_created: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Insertable, Serialize, Deserialize)]
#[diesel(table_name = schema::zones)]
pub struct NewZone {
    pub home_id: i64,
    pub tado_zone_id: i64,
    pub name: Option<String>,
    pub zone_type: Option<String>,
    pub date_created: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Queryable, Identifiable, Associations, Selectable, Serialize, Deserialize)]
#[diesel(table_name = schema::devices)]
#[diesel(belongs_to(Home))]
pub struct Device {
    pub id: i64,
    pub home_id: i64,
    pub tado_device_id: String,
    pub short_serial_no: Option<String>,
    pub device_type: Option<String>,
    pub device_type_desc: Option<String>,
    pub firmware_version: Option<String>,
    pub orientation: Option<String>,
    pub battery_state: Option<String>,
    pub characteristics: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Insertable, Serialize, Deserialize)]
#[diesel(table_name = schema::devices)]
pub struct NewDevice {
    pub home_id: i64,
    pub tado_device_id: String,
    pub short_serial_no: Option<String>,
    pub device_type: Option<String>,
    pub device_type_desc: Option<String>,
    pub firmware_version: Option<String>,
    pub orientation: Option<String>,
    pub battery_state: Option<String>,
    pub characteristics: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Queryable, Identifiable, Associations, Selectable, Serialize, Deserialize)]
#[diesel(table_name = schema::zone_devices)]
#[diesel(primary_key(zone_id, device_id))]
#[diesel(belongs_to(Zone))]
#[diesel(belongs_to(Device))]
pub struct ZoneDevice {
    pub zone_id: i64,
    pub device_id: i64,
    pub linked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Insertable, Serialize, Deserialize)]
#[diesel(table_name = schema::zone_devices)]
pub struct NewZoneDevice {
    pub zone_id: i64,
    pub device_id: i64,
    pub linked_at: Option<DateTime<Utc>>,
}

// Hypertable: climate_measurements
#[derive(Debug, Clone, Queryable, Identifiable, Associations, Selectable, Serialize, Deserialize)]
#[diesel(table_name = schema::climate_measurements)]
#[diesel(primary_key(id, time))]
#[diesel(belongs_to(Home))]
#[diesel(belongs_to(Zone))]
#[diesel(belongs_to(Device))]
pub struct ClimateMeasurement {
    pub id: i64,
    pub time: DateTime<Utc>,
    pub home_id: i64,
    pub zone_id: Option<i64>,
    pub device_id: Option<i64>,
    pub source: String,
    pub inside_temp_c: Option<f64>,
    pub humidity_pct: Option<f64>,
    pub setpoint_temp_c: Option<f64>,
    pub heating_power_pct: Option<f64>,
    pub ac_power_on: Option<bool>,
    pub ac_mode: Option<String>,
    pub window_open: Option<bool>,
    pub battery_low: Option<bool>,
    pub connection_up: Option<bool>,
}

#[derive(Debug, Clone, Insertable, Serialize, Deserialize)]
#[diesel(table_name = schema::climate_measurements)]
pub struct NewClimateMeasurement {
    pub time: DateTime<Utc>,
    pub home_id: i64,
    pub zone_id: Option<i64>,
    pub device_id: Option<i64>,
    pub source: String,
    pub inside_temp_c: Option<f64>,
    pub humidity_pct: Option<f64>,
    pub setpoint_temp_c: Option<f64>,
    pub heating_power_pct: Option<f64>,
    pub ac_power_on: Option<bool>,
    pub ac_mode: Option<String>,
    pub window_open: Option<bool>,
    pub battery_low: Option<bool>,
    pub connection_up: Option<bool>,
}

// Hypertable: weather_measurements
#[derive(Debug, Clone, Queryable, Identifiable, Associations, Selectable, Serialize, Deserialize)]
#[diesel(table_name = schema::weather_measurements)]
#[diesel(primary_key(id, time))]
#[diesel(belongs_to(Home))]
pub struct WeatherMeasurement {
    pub id: i64,
    pub time: DateTime<Utc>,
    pub home_id: i64,
    pub source: String,
    pub outside_temp_c: Option<f64>,
    pub solar_intensity_pct: Option<f64>,
    pub weather_state: Option<String>,
}

#[derive(Debug, Clone, Insertable, Serialize, Deserialize)]
#[diesel(table_name = schema::weather_measurements)]
pub struct NewWeatherMeasurement {
    pub time: DateTime<Utc>,
    pub home_id: i64,
    pub source: String,
    pub outside_temp_c: Option<f64>,
    pub solar_intensity_pct: Option<f64>,
    pub weather_state: Option<String>,
}

// Hypertable: events (non-climate and general lifecycle events)
#[derive(Debug, Clone, Queryable, Identifiable, Associations, Selectable, Serialize, Deserialize)]
#[diesel(table_name = schema::events)]
#[diesel(primary_key(id, time))]
#[diesel(belongs_to(Home))]
#[diesel(belongs_to(Zone))]
#[diesel(belongs_to(Device))]
pub struct Event {
    pub id: i64,
    pub time: DateTime<Utc>,
    pub home_id: i64,
    pub zone_id: Option<i64>,
    pub device_id: Option<i64>,
    pub source: Option<String>,
    pub event_type: String,
    pub payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Insertable, Serialize, Deserialize)]
#[diesel(table_name = schema::events)]
pub struct NewEvent {
    pub time: DateTime<Utc>,
    pub home_id: i64,
    pub zone_id: Option<i64>,
    pub device_id: Option<i64>,
    pub source: Option<String>,
    pub event_type: String,
    pub payload: Option<serde_json::Value>,
}
