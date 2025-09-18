-- This file should not be run within a transaction
-- Time-series tables intended as TimescaleDB hypertables

-- Ensure TimescaleDB extension is available
create extension if not exists timescaledb;

-- Climate measurements: zone/device scoped interior data
create table if not exists climate_measurements (
    id                  bigserial not null,
    time                timestamptz not null,
    home_id             bigint not null references homes(id) on delete cascade,
    zone_id             bigint references zones(id) on delete set null,
    device_id           bigint references devices(id) on delete set null,
    source              text not null check (source in ('realtime','historical','derived')),
    inside_temp_c       double precision,
    humidity_pct        double precision,
    setpoint_temp_c     double precision,
    heating_power_pct   double precision,
    ac_power_on         boolean,
    ac_mode             text,
    window_open         boolean,
    battery_low         boolean,
    connection_up       boolean,
    primary key (id, time)
);

-- In case a previous failed run created the table with a primary key on (id),
-- normalize to a hypertable-compatible primary key including the partition key.
alter table if exists climate_measurements drop constraint if exists climate_measurements_pkey;
alter table if exists climate_measurements add primary key (id, time);

-- Uniqueness & query performance
-- Deduplication: treat NULLs as equal across zone_id/device_id
create unique index if not exists climate_measurements_dedupe_uq
    on climate_measurements (time, home_id, source, zone_id, device_id) nulls not distinct;
create index if not exists climate_measurements_home_time_idx
    on climate_measurements (home_id, time desc);
create index if not exists climate_measurements_zone_time_idx
    on climate_measurements (zone_id, time desc) where zone_id is not null;
create index if not exists climate_measurements_device_time_idx
    on climate_measurements (device_id, time desc) where device_id is not null;
create index if not exists climate_measurements_home_source_time_idx
    on climate_measurements (home_id, source, time desc);

-- Convert to hypertable
select create_hypertable('climate_measurements', 'time', if_not_exists => true, chunk_time_interval => interval '7 days');

-- Weather measurements: home-scoped outdoor data
create table if not exists weather_measurements (
    id                      bigserial not null,
    time                    timestamptz not null,
    home_id                 bigint not null references homes(id) on delete cascade,
    source                  text not null check (source in ('realtime','historical','derived')),
    outside_temp_c          double precision,
    solar_intensity_pct     double precision,
    weather_state           text,
    primary key (id, time)
);

-- Normalize primary key to include partition column if a previous attempt created (id) only.
alter table if exists weather_measurements drop constraint if exists weather_measurements_pkey;
alter table if exists weather_measurements add primary key (id, time);

create unique index if not exists weather_measurements_dedupe_uq
    on weather_measurements (home_id, time, source);
create index if not exists weather_measurements_home_time_idx
    on weather_measurements (home_id, time desc);
create index if not exists weather_measurements_home_source_time_idx
    on weather_measurements (home_id, source, time desc);

select create_hypertable('weather_measurements', 'time', if_not_exists => true, chunk_time_interval => interval '7 days');

-- Events: overlay/open-window/device lifecycle
create table if not exists events (
    id          bigserial not null,
    time        timestamptz not null,
    home_id     bigint not null references homes(id) on delete cascade,
    zone_id     bigint references zones(id) on delete set null,
    device_id   bigint references devices(id) on delete set null,
    source      text,
    event_type  text not null,
    payload     jsonb,
    primary key (id, time)
);

-- Normalize primary key to include partition column if a previous attempt created (id) only.
alter table if exists events drop constraint if exists events_pkey;
alter table if exists events add primary key (id, time);

create index if not exists events_home_time_idx on events (home_id, time desc);
create index if not exists events_zone_time_idx on events (zone_id, time desc) where zone_id is not null;
create index if not exists events_device_time_idx on events (device_id, time desc) where device_id is not null;
create index if not exists events_type_time_idx on events (event_type, time desc);
-- Optional: GIN index for payload if ad-hoc JSON queries are needed
-- create index if not exists events_payload_gin on events using gin (payload);

select create_hypertable('events', 'time', if_not_exists => true, chunk_time_interval => interval '30 days');
