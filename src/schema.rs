// @generated automatically by Diesel CLI.

diesel::table! {
    climate_measurements (id, time) {
        id -> Int8,
        time -> Timestamptz,
        home_id -> Int8,
        zone_id -> Nullable<Int8>,
        device_id -> Nullable<Int8>,
        source -> Text,
        inside_temp_c -> Nullable<Float8>,
        humidity_pct -> Nullable<Float8>,
        setpoint_temp_c -> Nullable<Float8>,
        heating_power_pct -> Nullable<Float8>,
        ac_power_on -> Nullable<Bool>,
        ac_mode -> Nullable<Text>,
        window_open -> Nullable<Bool>,
        battery_low -> Nullable<Bool>,
        connection_up -> Nullable<Bool>,
    }
}

diesel::table! {
    devices (id) {
        id -> Int8,
        home_id -> Int8,
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

diesel::table! {
    events (id, time) {
        id -> Int8,
        time -> Timestamptz,
        home_id -> Int8,
        zone_id -> Nullable<Int8>,
        device_id -> Nullable<Int8>,
        source -> Nullable<Text>,
        event_type -> Text,
        payload -> Nullable<Jsonb>,
    }
}

diesel::table! {
    homes (id) {
        id -> Int8,
        tado_home_id -> Int8,
        name -> Nullable<Text>,
        timezone -> Nullable<Text>,
        temperature_unit -> Nullable<Text>,
        address_line1 -> Nullable<Text>,
        address_line2 -> Nullable<Text>,
        zip_code -> Nullable<Text>,
        city -> Nullable<Text>,
        state -> Nullable<Text>,
        country -> Nullable<Text>,
        latitude -> Nullable<Float8>,
        longitude -> Nullable<Float8>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    user_homes (user_id, home_id) {
        user_id -> Int8,
        home_id -> Int8,
        role -> Nullable<Text>,
        joined_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    users (id) {
        id -> Int8,
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
    weather_measurements (id, time) {
        id -> Int8,
        time -> Timestamptz,
        home_id -> Int8,
        source -> Text,
        outside_temp_c -> Nullable<Float8>,
        solar_intensity_pct -> Nullable<Float8>,
        weather_state -> Nullable<Text>,
    }
}

diesel::table! {
    zone_devices (zone_id, device_id) {
        zone_id -> Int8,
        device_id -> Int8,
        linked_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    zones (id) {
        id -> Int8,
        home_id -> Int8,
        tado_zone_id -> Int8,
        name -> Nullable<Text>,
        zone_type -> Nullable<Text>,
        date_created -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::joinable!(climate_measurements -> devices (device_id));
diesel::joinable!(climate_measurements -> homes (home_id));
diesel::joinable!(climate_measurements -> zones (zone_id));
diesel::joinable!(devices -> homes (home_id));
diesel::joinable!(events -> devices (device_id));
diesel::joinable!(events -> homes (home_id));
diesel::joinable!(events -> zones (zone_id));
diesel::joinable!(user_homes -> homes (home_id));
diesel::joinable!(user_homes -> users (user_id));
diesel::joinable!(weather_measurements -> homes (home_id));
diesel::joinable!(zone_devices -> devices (device_id));
diesel::joinable!(zone_devices -> zones (zone_id));
diesel::joinable!(zones -> homes (home_id));

diesel::allow_tables_to_appear_in_same_query!(
    climate_measurements,
    devices,
    events,
    homes,
    user_homes,
    users,
    weather_measurements,
    zone_devices,
    zones,
);
