-- Drop hypertables/tables in reverse order
drop table if exists events;
drop index if exists weather_measurements_home_source_time_idx;
drop index if exists weather_measurements_home_time_idx;
drop index if exists weather_measurements_dedupe_uq;
drop table if exists weather_measurements;
drop index if exists climate_measurements_home_source_time_idx;
drop index if exists climate_measurements_device_time_idx;
drop index if exists climate_measurements_zone_time_idx;
drop index if exists climate_measurements_home_time_idx;
drop index if exists climate_measurements_dedupe_uq;
drop table if exists climate_measurements;

-- Note: we don't drop the extension automatically here.

