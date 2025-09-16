//! Models derived from `tado-openapi.yml` components.schemas.
//!
//! Scope: types only — no API client/server code.
//!
//! Notes
//! - All object schemas are modeled as strongly typed Rust structs/enums.
//! - Date/time fields use `chrono` (`DateTime<Utc>`). Time-of-day fields remain strings.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

// =====================
// Scalar ID newtype wrappers
// =====================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BridgeId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeviceId(pub String);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HeatingCircuitId(pub i64);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HomeId(pub i64);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstallationId(pub i64);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MobileDeviceId(pub i64);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum TimetableTypeId {
    // 0=ONE_DAY, 1=THREE_DAY, 2=SEVEN_DAY
    OneDay,
    ThreeDay,
    SevenDay,
}

impl serde::Serialize for TimetableTypeId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let n: i32 = match self {
            TimetableTypeId::OneDay => 0,
            TimetableTypeId::ThreeDay => 1,
            TimetableTypeId::SevenDay => 2,
        };
        serializer.serialize_i32(n)
    }
}

impl<'de> serde::Deserialize<'de> for TimetableTypeId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = TimetableTypeId;

            fn expecting(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                write!(f, "an integer 0, 1 or 2 for TimetableTypeId")
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value {
                    0 => Ok(TimetableTypeId::OneDay),
                    1 => Ok(TimetableTypeId::ThreeDay),
                    2 => Ok(TimetableTypeId::SevenDay),
                    other => Err(E::invalid_value(serde::de::Unexpected::Signed(other), &self)),
                }
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value {
                    0 => Ok(TimetableTypeId::OneDay),
                    1 => Ok(TimetableTypeId::ThreeDay),
                    2 => Ok(TimetableTypeId::SevenDay),
                    other => Err(E::invalid_value(serde::de::Unexpected::Unsigned(other), &self)),
                }
            }
        }

        deserializer.deserialize_any(V)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ZoneId(pub i64);

// =====================
// Core enums (string enums in OpenAPI)
// =====================

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AirConditioningMode {
    Auto,
    Cool,
    Heat,
    Dry,
    Fan,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AirFreshness {
    Fair,
    Fresh,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BatteryState {
    Low,
    Normal,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CallForHeatValue {
    #[serde(rename = "NONE")]
    None_,
    #[serde(rename = "LOW")]
    Low,
    #[serde(rename = "MEDIUM")]
    Medium,
    #[serde(rename = "HIGH")]
    High,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DayType {
    MondayToSunday,
    MondayToFriday,
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FanLevel {
    Auto,
    Level1,
    Level2,
    Level3,
    Level4,
    Level5,
    Silent,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HomePresence {
    Home,
    Away,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HorizontalSwing {
    #[serde(rename = "OFF")]
    Off,
    #[serde(rename = "ON")]
    On,
    #[serde(rename = "RIGHT")]
    Right,
    #[serde(rename = "LEFT")]
    Left,
    #[serde(rename = "MID_RIGHT")]
    MidRight,
    #[serde(rename = "MID_LEFT")]
    MidLeft,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HumidityLevel {
    Humid,
    Comfy,
    Dry,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Light {
    #[serde(rename = "OFF")]
    Off,
    #[serde(rename = "ON")]
    On,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Orientation {
    Horizontal,
    Vertical,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Power {
    #[serde(rename = "ON")]
    On,
    #[serde(rename = "OFF")]
    Off,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TemperatureLevel {
    Cold,
    Comfy,
    Warm,
    Hot,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TemperatureUnit {
    Celsius,
    Fahrenheit,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TimetableTypeType {
    OneDay,
    ThreeDay,
    SevenDay,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VerticalSwing {
    #[serde(rename = "OFF")]
    Off,
    #[serde(rename = "MID_UP")]
    MidUp,
    #[serde(rename = "MID_DOWN")]
    MidDown,
    #[serde(rename = "ON")]
    On,
    #[serde(rename = "DOWN")]
    Down,
    #[serde(rename = "UP")]
    Up,
    #[serde(rename = "MID")]
    Mid,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WeatherState {
    Cloudy,
    CloudyMostly,
    CloudyPartly,
    Drizzle,
    Foggy,
    NightClear,
    NightCloudy,
    Rain,
    ScatteredRain,
    ScatteredRainSnow,
    ScatteredSnow,
    Snow,
    Sun,
    Thunderstorm,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ZoneOverlayTerminationType {
    Manual,
    TadoMode,
    Timer,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ZoneOverlayTerminationTypeSkillBasedApp {
    Manual,
    TadoMode,
    Timer,
    NextTimeBlock,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ZoneType {
    AirConditioning,
    Heating,
    HotWater,
}

// =====================
// Core datapoint structs
// =====================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Temperature {
    pub celsius: Option<f64>,
    pub fahrenheit: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TemperaturePrecision {
    pub celsius: Option<f64>,
    pub fahrenheit: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TemperatureDataPoint {
    // From Temperature (allOf)
    pub celsius: Option<f64>,
    pub fahrenheit: Option<f64>,
    // Extra properties
    pub timestamp: Option<DateTime<Utc>>,
    #[serde(rename = "type")]
    pub r#type: Option<String>,
    pub precision: Option<TemperaturePrecision>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PercentageDataPoint {
    #[serde(rename = "type")]
    pub r#type: Option<String>,
    pub percentage: Option<f64>,
    pub timestamp: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PowerDataPoint {
    #[serde(rename = "type")]
    pub r#type: Option<String>,
    pub value: Option<Power>,
    pub timestamp: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SensorDataPoints {
    pub inside_temperature: Option<TemperatureDataPoint>,
    pub humidity: Option<PercentageDataPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActivityDataPoints {
    pub heating_power: Option<PercentageDataPoint>,
    pub ac_power: Option<PowerDataPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WeatherStateDataPoint {
    #[serde(rename = "type")]
    pub r#type: Option<String>,
    pub value: Option<WeatherState>,
    pub timestamp: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TemperatureCapability {
    pub celsius: Option<TemperatureRange>,
    pub fahrenheit: Option<TemperatureRange>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TemperatureRange {
    pub min: Option<i32>,
    pub max: Option<i32>,
    pub step: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DataInterval {
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

// =====================
// Simple string newtype wrappers (non-enum)
// =====================
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeviceType(pub String); // documented known values; not declared as enum in spec

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ZoneOverlayType(pub String); // only known value MANUAL per spec notes

// =====================
// Additional strongly typed schemas
// =====================

// Air conditioning settings/capabilities

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AirConditioningZoneSettingsBase {
    pub fan_level: Option<FanLevel>,
    pub vertical_swing: Option<VerticalSwing>,
    pub horizontal_swing: Option<HorizontalSwing>,
    pub light: Option<Light>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AirConditioningZoneSettings {
    #[serde(flatten)]
    pub base: AirConditioningZoneSettingsBase,
    pub temperature: Option<Temperature>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AirConditioningModeCapabilitiesBase {
    pub fan_level: Option<Vec<FanLevel>>,
    pub vertical_swing: Option<Vec<VerticalSwing>>,
    pub horizontal_swing: Option<Vec<HorizontalSwing>>,
    pub light: Option<Vec<Light>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AirConditioningModeCapabilities {
    #[serde(flatten)]
    pub base: AirConditioningModeCapabilitiesBase,
    pub temperatures: Option<TemperatureCapability>,
}

// Air comfort

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AirComfort {
    pub freshness: Option<AirComfortFreshness>,
    pub comfort: Option<Vec<AirComfortRoomComfort>>, // empty when no connection
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AirComfortFreshness {
    pub value: Option<AirFreshness>,
    pub last_open_window: Option<DateTime<Utc>>,
    pub ac_powered_on: Option<bool>,
    pub last_ac_power_off: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AirComfortRoomComfort {
    pub room_id: Option<ZoneId>,
    pub temperature_level: Option<TemperatureLevel>,
    pub humidity_level: Option<HumidityLevel>,
    pub coordinate: Option<AirComfortCoordinate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AirComfortCoordinate {
    pub radial: Option<f64>,
    pub angular: Option<i64>,
}

// Misc simple inputs

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AwayRadiusInput {
    pub away_radius_in_meters: Option<f64>,
}

// Boiler / heating

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Boiler1 {
    pub present: Option<bool>,
    pub id: Option<i64>,
    pub found: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Boiler2 {
    pub boiler_present: Option<bool>,
    pub boiler_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BoilerMaxOutputTemperature {
    pub boiler_max_output_temperature_in_celsius: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BoilerWiringInstallationState {
    pub state: Option<String>,
    pub device_wired_to_boiler: Option<DeviceWiredToBoiler>,
    pub bridge_connected: Option<bool>,
    pub hot_water_zone_present: Option<bool>,
    pub boiler: Option<BoilerWiringInstallationStateBoiler>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeviceWiredToBoiler {
    pub r#type: Option<String>,
    pub serial_no: Option<String>,
    pub therm_interface_type: Option<String>,
    pub connected: Option<bool>,
    pub last_request_timestamp: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BoilerWiringInstallationStateBoiler {
    pub output_temperature: Option<BoilerOutputTemperature>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BoilerOutputTemperature {
    pub celsius: Option<f64>,
    pub timestamp: Option<DateTime<Utc>>,
}

// Boolean series

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BooleanDataInterval {
    #[serde(flatten)]
    pub interval: DataInterval,
    pub value: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BooleanTimeSeries {
    pub time_series_type: Option<String>,
    pub value_type: Option<String>,
    pub data_intervals: Option<Vec<BooleanDataInterval>>,
}

// Bridge

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Bridge {
    pub partner: Option<Value>,
    pub home_id: Option<HomeId>,
}

// Call for heat series

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CallForHeatDataInterval {
    #[serde(flatten)]
    pub interval: DataInterval,
    pub value: Option<CallForHeatValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CallForHeatTimeSeries {
    pub time_series_type: Option<String>,
    pub value_type: Option<String>,
    pub data_intervals: Option<Vec<CallForHeatDataInterval>>,
}

// ChildLock

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ChildLock {
    pub child_lock_enabled: Option<bool>,
}

// Day report and related

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DayReportInterval {
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DayReportMeasuredData {
    pub measuring_device_connected: Option<BooleanTimeSeries>,
    pub inside_temperature: Option<TemperatureTimeSeries>,
    pub humidity: Option<PercentageTimeSeries>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DayReportWeather {
    pub condition: Option<WeatherConditionTimeSeries>,
    pub sunny: Option<BooleanTimeSeries>,
    pub slots: Option<WeatherSlotTimeSeries>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DayReport {
    pub zone_type: Option<ZoneType>,
    pub interval: Option<DayReportInterval>,
    pub hours_in_day: Option<i64>,
    pub measured_data: Option<DayReportMeasuredData>,
    pub stripes: Option<StripesTimeSeries>,
    pub settings: Option<ZoneSettingTimeSeries>,
    pub call_for_heat: Option<CallForHeatTimeSeries>,
    pub hot_water_production: Option<BooleanTimeSeries>,
    pub ac_activity: Option<PowerTimeSeries>,
    pub weather: Option<DayReportWeather>,
}

// Dazzle

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DazzleInput {
    pub enabled: Option<bool>,
}

// Default overlay

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DefaultZoneOverlay {
    pub termination_condition: Option<DefaultOverlayTerminationCondition>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DefaultOverlayTerminationCondition {
    pub r#type: Option<ZoneOverlayTerminationType>,
    pub duration_in_seconds: Option<i64>,
}

// Devices

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeviceConnectionState {
    pub value: Option<bool>,
    pub timestamp: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCharacteristics {
    pub capabilities: Option<Vec<String>>, // unknown set
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeviceMountingState {
    pub value: Option<String>,
    pub timestamp: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeviceAccessPointWifi {
    pub ssid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub device_type: Option<DeviceType>,
    pub serial_no: Option<DeviceId>,
    pub short_serial_no: Option<String>,
    pub current_fw_version: Option<String>,
    pub connection_state: Option<DeviceConnectionState>,
    pub characteristics: Option<DeviceCharacteristics>,
    pub mounting_state: Option<DeviceMountingState>,
    pub mounting_state_with_error: Option<String>,
    pub battery_state: Option<BatteryState>,
    pub orientation: Option<Orientation>,
    pub child_lock_enabled: Option<bool>,
    pub is_driver_configured: Option<bool>,
    pub in_pairing_mode: Option<bool>,
    pub access_point_wi_fi: Option<DeviceAccessPointWifi>,
    pub command_table_upload_state: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeviceExtra {
    #[serde(flatten)]
    pub device: Device,
    pub duties: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeviceListItemZoneInfo {
    pub discriminator: Option<ZoneId>,
    pub duties: Option<Vec<String>>, // e.g. UI
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeviceListItem {
    pub r#type: Option<DeviceType>,
    pub device: Option<Device>,
    pub zone: Option<DeviceListItemZoneInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeviceList {
    pub entries: Option<Vec<DeviceListItem>>,
}

// Early start

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EarlyStart {
    pub enabled: Option<bool>,
}

// Error models

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    pub code: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Error422 {
    #[serde(flatten)]
    pub base: Error,
    pub zone_type: Option<ZoneType>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub errors: Option<Vec<Error>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse422 {
    pub errors: Option<Vec<Error422>>,
}

// Flow temperature optimization

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FlowTemperatureOptimizationConstraints {
    pub min: Option<i64>,
    pub max: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FlowTemperatureOptimizationAutoAdaptation {
    pub enabled: Option<bool>,
    pub max_flow_temperature: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FlowTemperatureOptimization {
    pub has_multiple_boiler_control_devices: Option<bool>,
    pub max_flow_temperature: Option<i64>,
    pub max_flow_temperature_constraints: Option<FlowTemperatureOptimizationConstraints>,
    pub auto_adaptation: Option<FlowTemperatureOptimizationAutoAdaptation>,
    pub open_therm_device_serial_number: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FlowTemperatureOptimizationInput {
    pub max_flow_temperature: Option<f64>,
}

// Heating Circuit/System

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HeatingCircuit {
    pub number: Option<HeatingCircuitId>,
    pub driver_serial_no: Option<String>,
    pub driver_short_serial_no: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HeatingCircuitInput {
    pub circuit_number: Option<HeatingCircuitId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UnderfloorHeating {
    pub present: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HeatingSystem {
    pub boiler: Option<Boiler1>,
    pub underfloor_heating: Option<UnderfloorHeating>,
}

// Home models

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HomeBase {
    pub id: Option<HomeId>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HomeContactDetails {
    pub name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HomeAddress {
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub zip_code: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub country: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HomeGeolocation {
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HomeDetails {
    #[serde(flatten)]
    pub base: HomeBase,
    pub contact_details: Option<HomeContactDetails>,
    pub address: Option<HomeAddress>,
    pub geolocation: Option<HomeGeolocation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IncidentDetection {
    pub enabled: Option<bool>,
    pub supported: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IncidentDetectionInput {
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Home {
    #[serde(flatten)]
    pub details: HomeDetails,
    pub date_time_zone: Option<String>,
    pub date_created: Option<DateTime<Utc>>,
    pub temperature_unit: Option<TemperatureUnit>,
    pub partner: Option<Value>,
    pub simple_smart_schedule_enabled: Option<bool>,
    pub away_radius_in_meters: Option<f64>,
    pub installation_completed: Option<bool>,
    pub incident_detection: Option<IncidentDetection>,
    pub generation: Option<String>,
    pub zones_count: Option<i64>,
    pub language: Option<String>,
    pub skills: Option<Vec<String>>, // often empty
    pub christmas_mode_enabled: Option<bool>,
    pub show_auto_assist_reminders: Option<bool>,
    pub consent_grant_skippable: Option<bool>,
    pub enabled_features: Option<Vec<String>>,
    pub is_air_comfort_eligible: Option<bool>,
    pub is_balance_ac_eligible: Option<bool>,
    pub is_energy_iq_eligible: Option<bool>,
    pub is_heat_source_installed: Option<bool>,
    pub is_balance_hp_eligible: Option<bool>,
    pub is_heat_pump_installed: Option<bool>,
    pub supports_flow_temperature_optimization: Option<bool>,
}

// Home presence/state

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HomeState {
    pub presence: Option<HomePresence>,
    pub presence_locked: Option<bool>,
    pub show_home_presence_switch_button: Option<bool>,
}

// Installation (AC)

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstallationAcSpecsRemoteControl {
    pub command_type: Option<String>,
    pub temperature_unit: Option<TemperatureUnit>,
    pub model_name: Option<String>,
    pub photo_s3_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstallationAcSpecsManufacturer {
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstallationAcSpecs {
    pub ac_unit_displays_set_point_temperature: Option<bool>,
    pub remote_control: Option<InstallationAcSpecsRemoteControl>,
    pub manufacturer: Option<InstallationAcSpecsManufacturer>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstallationCreatedZone {
    pub id: Option<ZoneId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstallationAcInformation {
    pub wireless_remote_has_required_firmware: Option<bool>,
    pub ac_specs: Option<InstallationAcSpecs>,
    pub key_command_set_recording: Option<Value>,
    pub ac_setting_command_set_recording: Option<Value>,
    pub created_zone: Option<InstallationCreatedZone>,
    pub selected_setup_branch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Installation {
    pub id: Option<InstallationId>,
    pub r#type: Option<String>,
    pub revision: Option<i64>,
    pub state: Option<String>,
    pub devices: Option<Vec<Device>>,
    pub ac_installation_information: Option<InstallationAcInformation>,
}

// Invitation

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InvitationToken(pub String);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Invitation {
    pub token: Option<InvitationToken>,
    pub email: Option<String>,
    pub first_sent: Option<DateTime<Utc>>,
    pub last_sent: Option<DateTime<Utc>>,
    pub inviter: Option<InvitationInviter>,
    pub home: Option<Home>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InvitationInviter {
    pub name: Option<String>,
    pub email: Option<String>,
    pub username: Option<String>,
    pub enabled: Option<bool>,
    pub id: Option<String>,
    pub home_id: Option<HomeId>,
    pub locale: Option<String>,
    pub r#type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InvitationRequest {
    pub email: Option<String>,
}

// Mobile devices

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MobileDeviceSettingsPushNotifications {
    pub low_battery_reminder: Option<bool>,
    pub away_mode_reminder: Option<bool>,
    pub home_mode_reminder: Option<bool>,
    pub open_window_reminder: Option<bool>,
    pub energy_savings_report_reminder: Option<bool>,
    pub incident_detection: Option<bool>,
    pub energy_iq_reminder: Option<bool>,
    pub tariff_high_price_alert: Option<bool>,
    pub tariff_low_price_alert: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MobileDeviceSettings {
    pub geo_tracking_enabled: Option<bool>,
    pub special_offers_enabled: Option<bool>,
    pub on_demand_log_retrieval_enabled: Option<bool>,
    pub push_notifications: Option<MobileDeviceSettingsPushNotifications>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MobileDeviceLocationBearing {
    pub degrees: Option<f64>,
    pub radians: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MobileDeviceLocation {
    pub stale: Option<bool>,
    pub at_home: Option<bool>,
    pub bearing_from_home: Option<MobileDeviceLocationBearing>,
    pub relative_distance_from_home_fence: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MobileDeviceMetadata {
    pub platform: Option<String>,
    pub os_version: Option<String>,
    pub model: Option<String>,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MobileDevice {
    pub id: Option<MobileDeviceId>,
    pub name: Option<String>,
    pub settings: Option<MobileDeviceSettings>,
    pub location: Option<MobileDeviceLocation>,
    pub device_metadata: Option<MobileDeviceMetadata>,
}

// Open window detection input

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OpenWindowDetectionInput {
    pub room_id: Option<ZoneId>,
    pub enabled: Option<bool>,
    pub timeout_in_seconds: Option<i64>,
}

// Percentage series

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PercentageDataPointInTimeSeries {
    pub timestamp: Option<DateTime<Utc>>,
    pub value: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PercentageTimeSeries {
    pub time_series_type: Option<String>,
    pub value_type: Option<String>,
    pub percentage_unit: Option<String>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub data_points: Option<Vec<PercentageDataPointInTimeSeries>>,
}

// Power series

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PowerDataInterval {
    #[serde(flatten)]
    pub interval: DataInterval,
    pub value: Option<Power>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PowerTimeSeries {
    pub time_series_type: Option<String>,
    pub value_type: Option<String>,
    pub data_intervals: Option<Vec<PowerDataInterval>>,
}

// Presence lock

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PresenceLock {
    pub home_presence: Option<HomePresence>,
}

// Stripes series

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StripesValue {
    pub stripe_type: Option<String>,
    pub setting: Option<ZoneSetting>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StripesDataInterval {
    #[serde(flatten)]
    pub interval: DataInterval,
    pub value: Option<StripesValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StripesTimeSeries {
    pub time_series_type: Option<String>,
    pub value_type: Option<String>,
    pub data_intervals: Option<Vec<StripesDataInterval>>,
}

// Temperature series

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TemperatureDataPointInTimeSeries {
    pub timestamp: Option<DateTime<Utc>>,
    pub value: Option<Temperature>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TemperatureTimeSeries {
    pub time_series_type: Option<String>,
    pub value_type: Option<String>,
    pub min: Option<Temperature>,
    pub max: Option<Temperature>,
    pub data_points: Option<Vec<TemperatureDataPointInTimeSeries>>,
}

// Timetable

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TimetableBlock {
    pub day_type: Option<DayType>,
    pub start: Option<String>, // HH:MM
    pub end: Option<String>,   // HH:MM
    pub geolocation_override: Option<bool>,
    pub setting: Option<ZoneSetting>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TimetableType {
    pub id: Option<TimetableTypeId>,
    pub r#type: Option<TimetableTypeType>,
}

// User

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub name: Option<String>,
    pub email: Option<String>,
    pub username: Option<String>,
    pub id: Option<String>,
    pub locale: Option<String>,
    pub homes: Option<Vec<HomeBase>>,
    pub mobile_devices: Option<Vec<MobileDevice>>,
}

// Weather

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Weather {
    pub solar_intensity: Option<PercentageDataPoint>,
    pub outside_temperature: Option<TemperatureDataPoint>,
    pub weather_state: Option<WeatherStateDataPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WeatherConditionValue {
    pub state: Option<WeatherState>,
    pub temperature: Option<Temperature>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WeatherConditionDataInterval {
    #[serde(flatten)]
    pub interval: DataInterval,
    pub value: Option<WeatherConditionValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WeatherConditionTimeSeries {
    pub time_series_type: Option<String>,
    pub value_type: Option<String>,
    pub data_intervals: Option<Vec<WeatherConditionDataInterval>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WeatherSlot {
    pub state: Option<WeatherState>,
    pub temperature: Option<Temperature>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WeatherSlotTimeSeries {
    pub time_series_type: Option<String>,
    pub value_type: Option<String>,
    pub slots: Option<BTreeMap<String, WeatherSlot>>, // keyed by HH:MM
}

// Zone and related

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Zone {
    pub id: Option<ZoneId>,
    pub name: Option<String>,
    pub r#type: Option<ZoneType>,
    pub date_created: Option<DateTime<Utc>>,
    pub device_types: Option<Vec<DeviceType>>,
    pub devices: Option<Vec<DeviceExtra>>,
    pub report_available: Option<bool>,
    pub show_schedule_setup: Option<bool>,
    pub supports_dazzle: Option<bool>,
    pub dazzle_enabled: Option<bool>,
    pub dazzle_mode: Option<ZoneDazzleMode>,
    pub open_window_detection: Option<ZoneOpenWindowDetection>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneDazzleMode {
    pub supported: Option<bool>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneOpenWindowDetection {
    pub supported: Option<bool>,
    pub enabled: Option<bool>,
    pub timeout_in_seconds: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneAwayConfiguration {
    pub r#type: Option<ZoneType>,
    pub auto_adjust: Option<bool>,
    pub comfort_level: Option<String>,
    pub setting: Option<ZoneSetting>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneCapabilitiesInitialStatesModes {
    #[serde(rename = "COOL")]
    pub cool: Option<AirConditioningZoneSettings>,
    #[serde(rename = "HEAT")]
    pub heat: Option<AirConditioningZoneSettings>,
    #[serde(rename = "DRY")]
    pub dry: Option<AirConditioningZoneSettings>,
    #[serde(rename = "FAN")]
    pub fan: Option<AirConditioningZoneSettings>,
    #[serde(rename = "AUTO")]
    pub auto: Option<AirConditioningZoneSettingsBase>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneCapabilitiesInitialStates {
    pub mode: Option<AirConditioningMode>,
    pub modes: Option<ZoneCapabilitiesInitialStatesModes>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneCapabilities {
    pub r#type: Option<ZoneType>,
    pub temperatures: Option<TemperatureCapability>,
    pub can_set_temperature: Option<bool>,
    #[serde(rename = "AUTO")]
    pub auto: Option<AirConditioningModeCapabilitiesBase>,
    #[serde(rename = "HEAT")]
    pub heat: Option<AirConditioningModeCapabilities>,
    #[serde(rename = "FAN")]
    pub fan: Option<AirConditioningModeCapabilities>,
    #[serde(rename = "COOL")]
    pub cool: Option<AirConditioningModeCapabilities>,
    #[serde(rename = "DRY")]
    pub dry: Option<AirConditioningModeCapabilities>,
    pub initial_states: Option<ZoneCapabilitiesInitialStates>,
}

// Zone create

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneCreateDevice {
    pub serial_no: Option<DeviceId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneCreate {
    pub r#type: Option<String>, // e.g. IMPLICIT_CONTROL
    pub zone_type: Option<ZoneType>,
    pub devices: Option<Vec<ZoneCreateDevice>>,
}

// Zone control

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneControlDuties {
    pub r#type: Option<ZoneType>,
    pub driver: Option<Device>,
    pub drivers: Option<Vec<Device>>,
    pub leader: Option<Device>,
    pub leaders: Option<Vec<Device>>,
    pub ui: Option<Device>,
    pub uis: Option<Vec<Device>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneControl {
    pub r#type: Option<ZoneType>,
    pub early_start_enabled: Option<bool>,
    pub heating_circuit: Option<HeatingCircuitId>,
    pub duties: Option<ZoneControlDuties>,
}

// Zone details input

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneDetailsInput {
    pub name: Option<String>,
}

// Zone open window

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneOpenWindow {
    pub detected_time: Option<DateTime<Utc>>,
    pub duration_in_seconds: Option<i64>,
    pub expiry: Option<DateTime<Utc>>,
    pub remaining_time_in_seconds: Option<i64>,
}

// Zone overlay and related

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneOverlayTermination {
    pub r#type: Option<ZoneOverlayTerminationType>,
    pub duration_in_seconds: Option<i64>,
    pub remaining_time_in_seconds: Option<i64>,
    pub type_skill_based_app: Option<ZoneOverlayTerminationTypeSkillBasedApp>,
    pub expiry: Option<DateTime<Utc>>,
    pub projected_expiry: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneOverlay {
    pub r#type: Option<ZoneOverlayType>,
    pub setting: Option<ZoneSetting>,
    pub termination: Option<ZoneOverlayTermination>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneOverlayEntry {
    pub room: Option<ZoneId>,
    pub overlay: Option<ZoneOverlay>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneOverlays {
    pub overlays: Option<Vec<ZoneOverlayEntry>>,
}

// Zone setting and series

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneSetting {
    #[serde(flatten)]
    pub base: AirConditioningZoneSettingsBase,
    pub r#type: Option<ZoneType>,
    pub power: Option<Power>,
    pub temperature: Option<Temperature>,
    pub mode: Option<AirConditioningMode>,
    pub is_boost: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneSettingDataInterval {
    #[serde(flatten)]
    pub interval: DataInterval,
    pub value: Option<ZoneSetting>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneSettingTimeSeries {
    pub time_series_type: Option<String>,
    pub value_type: Option<String>,
    pub data_intervals: Option<Vec<ZoneSettingDataInterval>>,
}

// Zone state(s)

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneStateNextScheduleChange {
    pub start: Option<DateTime<Utc>>,
    pub setting: Option<ZoneSetting>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneStateNextTimeBlock {
    pub start: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneStateLinkReason {
    pub code: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneStateLink {
    pub state: Option<String>, // ONLINE/OFFLINE
    pub reason: Option<ZoneStateLinkReason>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneState {
    pub tado_mode: Option<HomePresence>,
    pub geolocation_override: Option<bool>,
    pub geolocation_override_disable_time: Option<DateTime<Utc>>,
    pub preparation: Option<Value>,
    pub setting: Option<ZoneSetting>,
    pub overlay_type: Option<ZoneOverlayType>,
    pub overlay: Option<ZoneOverlay>,
    pub open_window: Option<ZoneOpenWindow>,
    pub next_schedule_change: Option<ZoneStateNextScheduleChange>,
    pub next_time_block: Option<ZoneStateNextTimeBlock>,
    pub link: Option<ZoneStateLink>,
    pub running_offline_schedule: Option<bool>,
    pub activity_data_points: Option<ActivityDataPoints>,
    pub sensor_data_points: Option<SensorDataPoints>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZoneStates {
    pub zone_states: Option<BTreeMap<String, ZoneState>>, // keyed by zone id string
}
