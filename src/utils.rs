use crate::client::{TadoClient, TadoClientError};
use crate::models::tado::{HomeId, ZoneId};
use chrono::{DateTime, Utc};
use core::fmt;
use serde::Serialize;
use std::error::Error;
use std::fmt::{Display, Formatter};

/// Errors that can occur while determining a zone's historical start time.
#[derive(Debug)]
pub enum StartTimeError {
    /// Underlying API client error
    Api(TadoClientError),
    /// The specified zone ID was not found in the home
    ZoneNotFound(ZoneId),
    /// The zone exists but does not report a creation timestamp
    MissingDateCreated(ZoneId),
}

impl Display for StartTimeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            StartTimeError::Api(e) => write!(f, "api error: {}", e),
            StartTimeError::ZoneNotFound(z) => write!(f, "zone {} not found in home", z.0),
            StartTimeError::MissingDateCreated(z) => {
                write!(f, "zone {} missing date_created field", z.0)
            }
        }
    }
}

impl Error for StartTimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            StartTimeError::Api(e) => Some(e),
            _ => None,
        }
    }
}

impl From<TadoClientError> for StartTimeError {
    fn from(value: TadoClientError) -> Self {
        StartTimeError::Api(value)
    }
}

/// Determine the earliest timestamp to begin historical backfill for a zone.
///
/// Policy (per requirements):
/// - Use only the Tado endpoints (no database access).
/// - Use the zone's `date_created` as the start time.
/// - Return the raw creation timestamp without normalization.
///
/// Errors when the zone cannot be found or `date_created` is missing.
pub fn determine_zone_start_time(
    client: &TadoClient,
    home_id: HomeId,
    zone_id: ZoneId,
) -> Result<DateTime<Utc>, StartTimeError> {
    let zones = client.get_zones(home_id)?;
    let zone = zones
        .into_iter()
        .find(|z| z.id == Some(zone_id))
        .ok_or(StartTimeError::ZoneNotFound(zone_id))?;

    zone.date_created.ok_or(StartTimeError::MissingDateCreated(zone_id))
}

/// Serialize a serde-backed enum into its string name (e.g. SCREAMING_SNAKE_CASE).
pub fn serde_enum_name<T: Serialize>(val: &T) -> Option<String> {
    serde_json::to_value(val).ok()?.as_str().map(|s| s.to_string())
}

/// Map Tado device type codes to human-friendly descriptions.
///
/// Source: known values documented in `tado-openapi.yml` under `components/schemas/DeviceType`.
pub fn describe_device_type(code: &str) -> Option<&'static str> {
    match code {
        "GW01" => Some("Gateway V1"),
        "GW02" => Some("Bridge V2"),
        "IB01" => Some("Internet Bridge V3+"),
        "BX02" => Some("Box V1"),
        "BU01" => Some("Extension Kit UK"),
        "EK01" => Some("Extension Kit UK"),
        "BR02" => Some("Wireless Receiver V3+"),
        "BP02" => Some("Wireless Receiver UK V3+"),
        "RU01" => Some("Smart Thermostat V3"),
        "RU02" => Some("Wired Smart Thermostat V3+"),
        "TS02" => Some("Temp Sensor V1"),
        "SU02" => Some("Wireless Temperature Sensor V3+"),
        "VA01" => Some("Smart Radiator Thermostat V3"),
        "VA02" => Some("Smart Radiator Thermostat V3+"),
        "WR01" => Some("Smart AC Control V3"),
        "WR02" => Some("Smart AC Control V3+"),
        _ => None,
    }
}
