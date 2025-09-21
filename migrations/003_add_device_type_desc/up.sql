-- Add a human-readable device type description to devices
alter table if exists devices
    add column if not exists device_type_desc text;

