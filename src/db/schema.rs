//! Handwritten Diesel schema declarations used by model structs.
//!
//! Migrations will define the actual tables and constraints. This module only
//! provides `diesel::table!` declarations so we can derive Insertable/Queryable
//! in a type-safe way without running `diesel print-schema`.

diesel::table! {
    users (id) {
        id -> BigInt,
        tado_user_id -> Text,
        email -> Nullable<Text>,
        username -> Nullable<Text>,
        name -> Nullable<Text>,
        locale -> Nullable<Text>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    homes (id) {
        id -> BigInt,
        tado_home_id -> BigInt,
        name -> Nullable<Text>,
        timezone -> Nullable<Text>,
        temperature_unit -> Nullable<Text>,
        address_line1 -> Nullable<Text>,
        address_line2 -> Nullable<Text>,
        zip_code -> Nullable<Text>,
        city -> Nullable<Text>,
        state -> Nullable<Text>,
        country -> Nullable<Text>,
        latitude -> Nullable<Double>,
        longitude -> Nullable<Double>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

// User membership in homes (many-to-many)
diesel::table! {
    user_homes (user_id, home_id) {
        user_id -> BigInt,
        home_id -> BigInt,
        role -> Nullable<Text>,
        joined_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    zones (id) {
        id -> BigInt,
        home_id -> BigInt,
        tado_zone_id -> BigInt,
        name -> Nullable<Text>,
        zone_type -> Nullable<Text>,
        date_created -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    devices (id) {
        id -> BigInt,
        home_id -> BigInt,
        tado_device_id -> Text,
        short_serial_no -> Nullable<Text>,
        device_type -> Nullable<Text>,
        firmware_version -> Nullable<Text>,
        orientation -> Nullable<Text>,
        battery_state -> Nullable<Text>,
        characteristics -> Nullable<Jsonb>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

// Many-to-many relationship between zones and devices
diesel::table! {
    zone_devices (zone_id, device_id) {
        zone_id -> BigInt,
        device_id -> BigInt,
        // duties could be modeled as text[] later if needed
        linked_at -> Nullable<Timestamptz>,
    }
}

// TimescaleDB hypertable (intended): per-zone/per-device climate readings
diesel::table! {
    climate_measurements (id) {
        id -> BigInt,
        time -> Timestamptz,
        home_id -> BigInt,
        zone_id -> Nullable<BigInt>,
        device_id -> Nullable<BigInt>,
        source -> Nullable<Text>, // historical | realtime
        inside_temp_c -> Nullable<Double>,
        humidity_pct -> Nullable<Double>,
        setpoint_temp_c -> Nullable<Double>,
        heating_power_pct -> Nullable<Double>,
        ac_power_on -> Nullable<Bool>,
        ac_mode -> Nullable<Text>,
        window_open -> Nullable<Bool>,
        battery_low -> Nullable<Bool>,
        connection_up -> Nullable<Bool>,
    }
}

// TimescaleDB hypertable (intended): home-level weather readings
diesel::table! {
    weather_measurements (id) {
        id -> BigInt,
        time -> Timestamptz,
        home_id -> BigInt,
        outside_temp_c -> Nullable<Double>,
        solar_intensity_pct -> Nullable<Double>,
        weather_state -> Nullable<Text>,
    }
}

// TimescaleDB hypertable (intended): structured/unstructured domain events
diesel::table! {
    events (id) {
        id -> BigInt,
        time -> Timestamptz,
        home_id -> BigInt,
        zone_id -> Nullable<BigInt>,
        device_id -> Nullable<BigInt>,
        source -> Nullable<Text>, // historical | realtime | derived
        event_type -> Text,
        payload -> Nullable<Jsonb>,
    }
}

diesel::joinable!(user_homes -> homes (home_id));
diesel::joinable!(user_homes -> users (user_id));
diesel::joinable!(zones -> homes (home_id));
diesel::joinable!(devices -> homes (home_id));
diesel::joinable!(zone_devices -> zones (zone_id));
diesel::joinable!(zone_devices -> devices (device_id));
diesel::joinable!(climate_measurements -> homes (home_id));
diesel::joinable!(climate_measurements -> zones (zone_id));
diesel::joinable!(climate_measurements -> devices (device_id));
diesel::joinable!(weather_measurements -> homes (home_id));
diesel::joinable!(events -> homes (home_id));
diesel::joinable!(events -> zones (zone_id));
diesel::joinable!(events -> devices (device_id));

diesel::allow_tables_to_appear_in_same_query!(
    users,
    homes,
    user_homes,
    zones,
    devices,
    zone_devices,
    climate_measurements,
    weather_measurements,
    events,
);

