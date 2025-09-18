-- Core entities: users, homes, user_homes, zones, devices, zone_devices

create table if not exists users (
    id              bigserial primary key,
    tado_user_id    text not null,
    email           text,
    username        text,
    name            text,
    locale          text,
    created_at      timestamptz not null default now(),
    updated_at      timestamptz not null default now()
);

create unique index if not exists users_tado_user_id_uq on users (tado_user_id);

create table if not exists homes (
    id                  bigserial primary key,
    tado_home_id        bigint not null,
    name                text,
    timezone            text,
    temperature_unit    text,
    address_line1       text,
    address_line2       text,
    zip_code            text,
    city                text,
    state               text,
    country             text,
    latitude            double precision,
    longitude           double precision,
    created_at          timestamptz not null default now(),
    updated_at          timestamptz not null default now()
);

create unique index if not exists homes_tado_home_id_uq on homes (tado_home_id);

create table if not exists user_homes (
    user_id     bigint not null references users(id) on delete cascade,
    home_id     bigint not null references homes(id) on delete cascade,
    role        text,
    joined_at   timestamptz,
    primary key (user_id, home_id)
);

create index if not exists user_homes_home_id_idx on user_homes (home_id);

create table if not exists zones (
    id              bigserial primary key,
    home_id         bigint not null references homes(id) on delete cascade,
    tado_zone_id    bigint not null,
    name            text,
    zone_type       text,
    date_created    timestamptz,
    created_at      timestamptz not null default now(),
    updated_at      timestamptz not null default now()
);

create unique index if not exists zones_home_zone_uq on zones (home_id, tado_zone_id);
create index if not exists zones_home_id_idx on zones (home_id);

create table if not exists devices (
    id                  bigserial primary key,
    home_id             bigint not null references homes(id) on delete cascade,
    tado_device_id      text not null,
    short_serial_no     text,
    device_type         text,
    firmware_version    text,
    orientation         text,
    battery_state       text,
    characteristics     jsonb,
    created_at          timestamptz not null default now(),
    updated_at          timestamptz not null default now()
);

create unique index if not exists devices_home_device_uq on devices (home_id, tado_device_id);
create index if not exists devices_home_id_idx on devices (home_id);

create table if not exists zone_devices (
    zone_id     bigint not null references zones(id) on delete cascade,
    device_id   bigint not null references devices(id) on delete cascade,
    linked_at   timestamptz,
    primary key (zone_id, device_id)
);

create index if not exists zone_devices_device_id_idx on zone_devices (device_id);

