use crate::client::TadoClient;
use crate::db::models as dbm;
use crate::models::tado;
use crate::schema;
use crate::utils::{describe_device_type, serde_enum_name};
use chrono::Utc;
use diesel::prelude::*;
use diesel::PgConnection;
use log::{debug, info, warn};
use std::collections::BTreeMap;

pub fn sync_all(conn: &mut PgConnection, client: &TadoClient, me: &tado::User, home_ids: &[i64]) -> Result<(), String> {
    info!("Syncing references for {} home(s)", home_ids.len());
    let db_user_id = upsert_user(conn, me)?;
    for home_id in home_ids {
        info!("Refs: syncing home {}", home_id);
        let home = client
            .get_home(tado::HomeId(*home_id))
            .map_err(|e| format!("get_home({home_id}) failed: {}", e))?;
        let db_home_id = upsert_home(conn, &home)?;
        upsert_user_home(conn, db_user_id, db_home_id)?;

        let zones = client
            .get_zones(tado::HomeId(*home_id))
            .map_err(|e| format!("get_zones({home_id}) failed: {}", e))?;
        let zone_map = upsert_zones(conn, db_home_id, &zones)?;

        let devices = client
            .get_devices(tado::HomeId(*home_id))
            .map_err(|e| format!("get_devices({home_id}) failed: {}", e))?;
        let device_map = upsert_devices(conn, db_home_id, &devices)?;

        debug!(
            "Refs: fetched home {} (zones={}, devices={})",
            home_id,
            zones.len(),
            devices.len()
        );

        let device_list = client
            .get_device_list(tado::HomeId(*home_id))
            .map_err(|e| format!("get_device_list({home_id}) failed: {}", e))?;
        upsert_zone_devices(conn, &zone_map, &device_map, device_list)?;
        info!("Refs: home {} complete", home_id);
    }
    Ok(())
}

fn upsert_user(conn: &mut PgConnection, me: &tado::User) -> Result<i64, String> {
    use schema::users::dsl as U;

    let tado_user_id = me
        .id
        .clone()
        .ok_or_else(|| "user id missing in /me response".to_string())?;
    let new_row = dbm::NewUser {
        tado_user_id,
        email: me.email.clone(),
        username: me.username.clone(),
        name: me.name.clone(),
        locale: me.locale.clone(),
    };

    diesel::insert_into(U::users)
        .values(&new_row)
        .on_conflict(U::tado_user_id)
        .do_update()
        .set((
            U::email.eq(new_row.email.clone()),
            U::username.eq(new_row.username.clone()),
            U::name.eq(new_row.name.clone()),
            U::locale.eq(new_row.locale.clone()),
            U::updated_at.eq(Utc::now()),
        ))
        .execute(conn)
        .map_err(|e| format!("upsert user failed: {}", e))?;

    let user: dbm::User = U::users
        .filter(U::tado_user_id.eq(new_row.tado_user_id))
        .first(conn)
        .map_err(|e| format!("fetch user failed: {}", e))?;
    Ok(user.id)
}

fn upsert_home(conn: &mut PgConnection, home: &tado::Home) -> Result<i64, String> {
    use schema::homes::dsl as H;

    let (tado_home_id, name) = (
        home.details.base.id.map(|h| h.0).unwrap_or_default(),
        home.details.base.name.clone(),
    );
    let new_row = dbm::NewHome {
        tado_home_id,
        name,
        timezone: home.date_time_zone.clone(),
        temperature_unit: home.temperature_unit.as_ref().and_then(serde_enum_name),
        address_line1: home.details.address.as_ref().and_then(|a| a.address_line1.clone()),
        address_line2: home.details.address.as_ref().and_then(|a| a.address_line2.clone()),
        zip_code: home.details.address.as_ref().and_then(|a| a.zip_code.clone()),
        city: home.details.address.as_ref().and_then(|a| a.city.clone()),
        state: home.details.address.as_ref().and_then(|a| a.state.clone()),
        country: home.details.address.as_ref().and_then(|a| a.country.clone()),
        latitude: home.details.geolocation.as_ref().and_then(|g| g.latitude),
        longitude: home.details.geolocation.as_ref().and_then(|g| g.longitude),
    };

    diesel::insert_into(H::homes)
        .values(&new_row)
        .on_conflict(H::tado_home_id)
        .do_update()
        .set((
            H::name.eq(new_row.name.clone()),
            H::timezone.eq(new_row.timezone.clone()),
            H::temperature_unit.eq(new_row.temperature_unit.clone()),
            H::address_line1.eq(new_row.address_line1.clone()),
            H::address_line2.eq(new_row.address_line2.clone()),
            H::zip_code.eq(new_row.zip_code.clone()),
            H::city.eq(new_row.city.clone()),
            H::state.eq(new_row.state.clone()),
            H::country.eq(new_row.country.clone()),
            H::latitude.eq(new_row.latitude),
            H::longitude.eq(new_row.longitude),
            H::updated_at.eq(Utc::now()),
        ))
        .execute(conn)
        .map_err(|e| format!("upsert home failed: {}", e))?;

    let row: dbm::Home = H::homes
        .filter(H::tado_home_id.eq(new_row.tado_home_id))
        .first(conn)
        .map_err(|e| format!("fetch home failed: {}", e))?;
    Ok(row.id)
}

fn upsert_user_home(conn: &mut PgConnection, user_id: i64, home_id: i64) -> Result<(), String> {
    use schema::user_homes::dsl as UH;

    let new_row = dbm::NewUserHome {
        user_id,
        home_id,
        role: None,
        joined_at: None,
    };
    diesel::insert_into(UH::user_homes)
        .values(&new_row)
        .on_conflict((UH::user_id, UH::home_id))
        .do_update()
        .set((UH::role.eq(new_row.role.clone()), UH::joined_at.eq(new_row.joined_at)))
        .execute(conn)
        .map_err(|e| format!("upsert user_homes failed: {}", e))?;
    Ok(())
}

fn upsert_zones(conn: &mut PgConnection, db_home_id: i64, zones: &[tado::Zone]) -> Result<BTreeMap<i64, i64>, String> {
    use schema::zones::dsl as Z;
    let mut map = BTreeMap::new();

    for z in zones {
        let tado_zone_id = z.id.map(|id| id.0).ok_or_else(|| "zone id missing".to_string())?;
        let new_row = dbm::NewZone {
            home_id: db_home_id,
            tado_zone_id,
            name: z.name.clone(),
            zone_type: z.r#type.as_ref().and_then(serde_enum_name),
            date_created: z.date_created,
        };
        diesel::insert_into(Z::zones)
            .values(&new_row)
            .on_conflict((Z::home_id, Z::tado_zone_id))
            .do_update()
            .set((
                Z::name.eq(new_row.name.clone()),
                Z::zone_type.eq(new_row.zone_type.clone()),
                Z::date_created.eq(new_row.date_created),
                Z::updated_at.eq(Utc::now()),
            ))
            .execute(conn)
            .map_err(|e| format!("upsert zone failed: {}", e))?;

        let row: dbm::Zone = Z::zones
            .filter(Z::home_id.eq(db_home_id).and(Z::tado_zone_id.eq(tado_zone_id)))
            .first(conn)
            .map_err(|e| format!("fetch zone failed: {}", e))?;
        map.insert(tado_zone_id, row.id);
    }
    Ok(map)
}

fn upsert_devices(
    conn: &mut PgConnection,
    db_home_id: i64,
    devices: &[tado::Device],
) -> Result<BTreeMap<String, i64>, String> {
    use schema::devices::dsl as D;
    let mut map = BTreeMap::new();
    for d in devices {
        let tado_device_id = match d.serial_no.as_ref().map(|s| s.0.clone()) {
            Some(s) if !s.is_empty() => s,
            _ => {
                warn!("Refs: skipping device without serial number");
                continue;
            }
        };
        let new_row = dbm::NewDevice {
            home_id: db_home_id,
            tado_device_id: tado_device_id.clone(),
            short_serial_no: d.short_serial_no.clone(),
            device_type: d.device_type.as_ref().map(|t| t.0.clone()),
            device_type_desc: d
                .device_type
                .as_ref()
                .and_then(|t| describe_device_type(&t.0).map(|s| s.to_string())),
            firmware_version: d.current_fw_version.clone(),
            orientation: d.orientation.as_ref().and_then(serde_enum_name),
            battery_state: d.battery_state.as_ref().and_then(serde_enum_name),
            characteristics: serde_json::to_value(&d.characteristics).ok(),
        };
        diesel::insert_into(D::devices)
            .values(&new_row)
            .on_conflict((D::home_id, D::tado_device_id))
            .do_update()
            .set((
                D::short_serial_no.eq(new_row.short_serial_no.clone()),
                D::device_type.eq(new_row.device_type.clone()),
                D::device_type_desc.eq(new_row.device_type_desc.clone()),
                D::firmware_version.eq(new_row.firmware_version.clone()),
                D::orientation.eq(new_row.orientation.clone()),
                D::battery_state.eq(new_row.battery_state.clone()),
                D::characteristics.eq(new_row.characteristics.clone()),
                D::updated_at.eq(Utc::now()),
            ))
            .execute(conn)
            .map_err(|e| format!("upsert device failed: {}", e))?;

        let row: dbm::Device = D::devices
            .filter(D::home_id.eq(db_home_id).and(D::tado_device_id.eq(&tado_device_id)))
            .select(dbm::Device::as_select())
            .first(conn)
            .map_err(|e| format!("fetch device failed: {}", e))?;
        map.insert(tado_device_id, row.id);
    }
    Ok(map)
}

fn upsert_zone_devices(
    conn: &mut PgConnection,
    zone_map: &BTreeMap<i64, i64>,
    device_map: &BTreeMap<String, i64>,
    device_list: tado::DeviceList,
) -> Result<(), String> {
    use schema::zone_devices::dsl as ZD;

    let entries = device_list.entries.unwrap_or_default();
    for e in entries {
        let device = match e.device.and_then(|d| d.serial_no.map(|s| s.0)) {
            Some(s) => s,
            None => continue,
        };
        let zone_tado_id = e.zone.and_then(|z| z.discriminator.map(|zid| zid.0));
        let (zone_db_id, device_db_id) = match zone_tado_id
            .and_then(|z| zone_map.get(&z).copied())
            .zip(device_map.get(&device).copied())
        {
            Some(pair) => pair,
            None => continue,
        };
        let link = dbm::NewZoneDevice {
            zone_id: zone_db_id,
            device_id: device_db_id,
            linked_at: None,
        };
        diesel::insert_into(ZD::zone_devices)
            .values(&link)
            .on_conflict((ZD::zone_id, ZD::device_id))
            .do_nothing()
            .execute(conn)
            .map_err(|e| format!("upsert zone_device failed: {}", e))?;
    }
    Ok(())
}
