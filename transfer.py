#!/usr/bin/env python3
"""Bulk importer for historical Tado data exported from InfluxDB."""

from __future__ import annotations

import argparse
import csv
import io
import os
import sys
from dataclasses import dataclass
from datetime import datetime
from typing import Dict, Iterable, List, Optional, Tuple, Any

from typing import TYPE_CHECKING

if TYPE_CHECKING:  # pragma: no cover - typing only
    import psycopg  # type: ignore

_psycopg = None


@dataclass
class ClimateRecord:
    time: str
    home_tado_id: int
    zone_tado_id: int
    device_tado_id: Optional[str]
    inside_temp_c: Optional[float]
    humidity_pct: Optional[float]
    setpoint_temp_c: Optional[float]
    heating_power_pct: Optional[float]
    source: str


@dataclass
class WeatherRecord:
    time: str
    home_tado_id: int
    outside_temp_c: Optional[float]
    weather_state: Optional[str]
    source: str


def get_psycopg():
    global _psycopg
    if _psycopg is None:
        try:
            import psycopg as _mod  # type: ignore
        except ImportError:  # pragma: no cover - import guard
            print(
                "psycopg (v3) is required: install with `pip install psycopg[binary]`",
                file=sys.stderr,
            )
            sys.exit(1)
        _psycopg = _mod
    return _psycopg


HEATING_LEVELS = {
    "NONE": 0.0,
    "LOW": 33.0,
    "MEDIUM": 66.0,
    "HIGH": 100.0,
}


def parse_int(value: Optional[str]) -> Optional[int]:
    if value is None:
        return None
    text = value.strip()
    if not text:
        return None
    try:
        return int(text)
    except ValueError:
        return None


def parse_float(value: Optional[str]) -> Optional[float]:
    if value is None:
        return None
    text = value.strip()
    if not text:
        return None
    try:
        return float(text)
    except ValueError:
        return None


def translate_heating(level: str, numeric_text: Optional[str]) -> Optional[float]:
    if level:
        mapped = HEATING_LEVELS.get(level.strip().upper())
        if mapped is not None:
            return mapped
    raw = parse_float(numeric_text)
    if raw is None:
        return None
    if raw <= 0:
        return 0.0
    if raw == 1:
        return 33.0
    if raw == 2:
        return 66.0
    if raw >= 3:
        return 100.0
    return None



def determine_source(timestamp: str) -> str:
    try:
        dt = datetime.fromisoformat(timestamp.replace('Z', '+00:00'))
    except ValueError:
        return 'realtime'
    if dt.second == 0 and dt.microsecond == 0 and dt.minute % 15 == 0:
        return 'historical'
    return 'realtime'


def iter_influx_rows(path: str) -> Iterable[Dict[str, str]]:
    def parse_line(line: str) -> List[str]:
        return next(csv.reader([line]))

    with open(path, newline="") as handle:
        fieldnames: Optional[List[str]] = None
        for raw in handle:
            if not raw.strip():
                continue
            if raw.startswith("#"):
                if raw.startswith("#group"):
                    fieldnames = None
                continue
            if fieldnames is None:
                fieldnames = parse_line(raw)
                continue
            values = parse_line(raw)
            if len(values) < len(fieldnames):
                values.extend([""] * (len(fieldnames) - len(values)))
            elif len(values) > len(fieldnames):
                values = values[: len(fieldnames)]
            row = {name: value for name, value in zip(fieldnames, values) if name}
            yield row


@dataclass
class ParseSummary:
    climate_rows: List[ClimateRecord]
    weather_rows: List[WeatherRecord]
    skipped_missing_home: int
    skipped_missing_zone: int


def parse_csv(path: str) -> ParseSummary:
    reader = iter_influx_rows(path)
    climate: Dict[Tuple[str, int, int, Optional[str]], Dict[str, Optional[float]]] = {}
    weather: Dict[Tuple[str, int], Dict[str, Optional[float]]] = {}
    skipped_home = 0
    skipped_zone = 0

    for row in reader:
        measurement = (row.get("_measurement") or "").strip()
        field = (row.get("_field") or "").strip()
        if not measurement or not field:
            continue
        if measurement == "_measurement" and field == "_field":
            continue
        if "intervalCumulativeHelper" in (measurement, field):
            continue
        if measurement == "weather_sunny" or field == "sunny":
            continue

        timestamp = (row.get("_time") or "").strip()
        if not timestamp:
            continue
        home_id = parse_int(row.get("homeId"))
        if home_id is None:
            skipped_home += 1
            continue

        zone_id = parse_int(row.get("zoneId"))
        device_id = (row.get("deviceId") or "").strip() or None
        level_text = (row.get("callForHeatLevel") or "").strip()
        raw_value = row.get("_value")
        numeric_value = parse_float(raw_value)

        key = (timestamp, home_id, zone_id, device_id)

        if measurement == "humidity" and field == "humidity":
            if numeric_value is None:
                continue
            entry = climate.setdefault(key, {})
            entry.setdefault('source', determine_source(timestamp))
            entry.setdefault("humidity_pct", max(0.0, min(100.0, numeric_value * 100.0)))
        elif measurement == "call_for_heat" and field == "numericLevel":
            pct = translate_heating(level_text, raw_value)
            if pct is None:
                continue
            entry = climate.setdefault(key, {})
            entry.setdefault('source', determine_source(timestamp))
            entry.setdefault("heating_power_pct", pct)
        elif measurement == "temperature" and field == "temperature":
            if numeric_value is None:
                continue
            entry = climate.setdefault(key, {})
            entry.setdefault('source', determine_source(timestamp))
            entry.setdefault("inside_temp_c", numeric_value)
        elif measurement == "heating" and field == "temperature":
            if numeric_value is None:
                continue
            entry = climate.setdefault(key, {})
            entry.setdefault('source', determine_source(timestamp))
            entry.setdefault("setpoint_temp_c", numeric_value)
        elif measurement == "weather" and field == "temperature":
            if numeric_value is None and not level_text:
                continue
            wkey = (timestamp, home_id)
            entry = weather.setdefault(wkey, {})
            entry.setdefault('source', determine_source(timestamp))
            if numeric_value is not None:
                entry.setdefault("outside_temp_c", numeric_value)
            if level_text:
                entry.setdefault("weather_state", level_text)
        else:
            continue

    climate_rows: List[ClimateRecord] = []
    for (timestamp, home_id, zone_id, device_id), payload in sorted(
        climate.items(), key=lambda item: (item[0][0], item[0][1], item[0][2], item[0][3] or "")
    ):
        if zone_id is None:
            skipped_zone += 1
            continue
        climate_rows.append(
            ClimateRecord(
                time=timestamp,
                home_tado_id=home_id,
                zone_tado_id=zone_id,
                device_tado_id=device_id,
                inside_temp_c=payload.get("inside_temp_c"),
                humidity_pct=payload.get("humidity_pct"),
                setpoint_temp_c=payload.get("setpoint_temp_c"),
                heating_power_pct=payload.get("heating_power_pct"),
                source=payload.get('source', 'historical'),
            )
        )

    weather_rows: List[WeatherRecord] = [
        WeatherRecord(
            time=timestamp,
            home_tado_id=home_id,
            outside_temp_c=payload.get("outside_temp_c"),
            weather_state=payload.get("weather_state"),
            source=payload.get('source', 'historical'),
        )
        for (timestamp, home_id), payload in sorted(weather.items(), key=lambda item: (item[0][0], item[0][1]))
    ]

    return ParseSummary(
        climate_rows=climate_rows,
        weather_rows=weather_rows,
        skipped_missing_home=skipped_home,
        skipped_missing_zone=skipped_zone,
    )


def write_summary(summary: ParseSummary) -> None:
    print(f"Climate rows prepared: {len(summary.climate_rows)}", file=sys.stderr)
    print(f"Weather rows prepared: {len(summary.weather_rows)}", file=sys.stderr)
    if summary.skipped_missing_home:
        print(
            f"Skipped rows missing homeId: {summary.skipped_missing_home}",
            file=sys.stderr,
        )
    if summary.skipped_missing_zone:
        print(
            f"Skipped climate points missing zoneId: {summary.skipped_missing_zone}",
            file=sys.stderr,
        )





def display_sample(summary: ParseSummary, *, max_rows: int = 3) -> None:
    def fmt_float(value: Optional[float]) -> str:
        return '-' if value is None else f'{value:.2f}'

    print('Sample climate rows:', file=sys.stderr)
    if summary.climate_rows:
        for row in summary.climate_rows[:max_rows]:
            device = row.device_tado_id or '-'
            print(
                f"  {row.time} home={row.home_tado_id} zone={row.zone_tado_id} "
                f"device={device} inside={fmt_float(row.inside_temp_c)} "
                f"humidity={fmt_float(row.humidity_pct)} "
                f"setpoint={fmt_float(row.setpoint_temp_c)} "
                f"heating={fmt_float(row.heating_power_pct)} source={row.source}",
                file=sys.stderr,
            )
    else:
        print('  (none)', file=sys.stderr)

    print('Sample weather rows:', file=sys.stderr)
    if summary.weather_rows:
        for row in summary.weather_rows[:max_rows]:
            state = row.weather_state or '-'
            print(
                f"  {row.time} home={row.home_tado_id} outside={fmt_float(row.outside_temp_c)} state={state} source={row.source}",
                file=sys.stderr,
            )
    else:
        print('  (none)', file=sys.stderr)



def display_sql_preview(cursor: Any, *, preview_limit: int = 3) -> None:
    def fmt_float(value: Optional[float]) -> str:
        return '-' if value is None else f'{value:.2f}'

    print('Database preview (climate_measurements):', file=sys.stderr)
    cursor.execute(
        """
        SELECT
            c.time,
            h.id AS home_id,
            CASE WHEN c.tado_zone_id IS NULL THEN NULL ELSE z.id END AS zone_id,
            CASE WHEN NULLIF(c.tado_device_id, '') IS NULL THEN NULL ELSE d.id END AS device_id,
            c.inside_temp_c,
            c.humidity_pct,
            c.setpoint_temp_c,
            c.heating_power_pct,
            c.source
        FROM tmp_climate c
        JOIN homes h ON h.tado_home_id = c.tado_home_id
        LEFT JOIN zones z ON z.home_id = h.id AND c.tado_zone_id = z.tado_zone_id
        LEFT JOIN devices d ON d.home_id = h.id AND d.tado_device_id = NULLIF(c.tado_device_id, '')
        ORDER BY c.time
        LIMIT %s
        """
        , (preview_limit,)
    )
    rows = cursor.fetchall()
    if rows:
        for row in rows:
            time, home_id, zone_id, device_id, inside, humidity, setpoint, heating, source = row
            print(
                f"  {time} home={home_id} zone={zone_id if zone_id is not None else '-'} "
                f"device={device_id if device_id is not None else '-'} inside={fmt_float(inside)} "
                f"humidity={fmt_float(humidity)} setpoint={fmt_float(setpoint)} "
                f"heating={fmt_float(heating)} source={source}",
                file=sys.stderr,
            )
    else:
        print('  (none)', file=sys.stderr)

    print('Database preview (weather_measurements):', file=sys.stderr)
    cursor.execute(
        """
        SELECT
            w.time,
            h.id AS home_id,
            w.outside_temp_c,
            NULLIF(w.weather_state, '') AS weather_state,
            w.source
        FROM tmp_weather w
        JOIN homes h ON h.tado_home_id = w.tado_home_id
        ORDER BY w.time
        LIMIT %s
        """
        , (preview_limit,)
    )
    rows = cursor.fetchall()
    if rows:
        for time, home_id, outside, state, source in rows:
            print(
                f"  {time} home={home_id} outside={fmt_float(outside)} state={state or '-'} source={source}",
                file=sys.stderr,
            )
    else:
        print('  (none)', file=sys.stderr)



def copy_rows(cursor: Any, sql: str, headers: List[str], rows: Iterable[Iterable[object]]) -> None:
    buffer = io.StringIO()
    writer = csv.writer(buffer)
    writer.writerow(headers)
    for row in rows:
        writer.writerow(row)
    buffer.seek(0)
    with cursor.copy(sql) as copy:
        copy.write(buffer.read())


def upsert_into_database(
    database_url: str, summary: ParseSummary, dry_run_rollback: bool
) -> Tuple[int, int, int, int]:
    psycopg = get_psycopg()
    try:
        with psycopg.connect(database_url) as conn:
            with conn.cursor() as cur:
                cur.execute(
                    """
                    CREATE TEMP TABLE tmp_climate (
                        time timestamptz NOT NULL,
                        tado_home_id bigint NOT NULL,
                        tado_zone_id bigint,
                        tado_device_id text,
                        inside_temp_c double precision,
                        humidity_pct double precision,
                        setpoint_temp_c double precision,
                        heating_power_pct double precision,
                        source text NOT NULL
                    )
                    """
                )

                copy_rows(
                    cur,
                    "COPY tmp_climate (time, tado_home_id, tado_zone_id, tado_device_id, inside_temp_c, "
                    "humidity_pct, setpoint_temp_c, heating_power_pct, source) FROM STDIN WITH (FORMAT CSV, HEADER true)",
                    [
                        "time",
                        "tado_home_id",
                        "tado_zone_id",
                        "tado_device_id",
                        "inside_temp_c",
                        "humidity_pct",
                        "setpoint_temp_c",
                        "heating_power_pct",
                        "source",
                    ],
                    (
                        (
                            row.time,
                            row.home_tado_id,
                            row.zone_tado_id,
                            row.device_tado_id or "",
                            row.inside_temp_c,
                            row.humidity_pct,
                            row.setpoint_temp_c,
                            row.heating_power_pct,
                            row.source,
                        )
                        for row in summary.climate_rows
                    ),
                )

                cur.execute(
                    """
                    CREATE TEMP TABLE tmp_weather (
                        time timestamptz NOT NULL,
                        tado_home_id bigint NOT NULL,
                        outside_temp_c double precision,
                        weather_state text,
                        source text NOT NULL
                    )
                    """
                )

                copy_rows(
                    cur,
                    "COPY tmp_weather (time, tado_home_id, outside_temp_c, weather_state, source) "
                    "FROM STDIN WITH (FORMAT CSV, HEADER true)",
                    ["time", "tado_home_id", "outside_temp_c", "weather_state", "source"],
                    (
                        (
                            row.time,
                            row.home_tado_id,
                            row.outside_temp_c,
                            row.weather_state or "",
                            row.source,
                        )
                        for row in summary.weather_rows
                    ),
                )

                cur.execute(
                    """
                    SELECT COUNT(*)
                    FROM tmp_climate c
                    LEFT JOIN homes h ON h.tado_home_id = c.tado_home_id
                    WHERE h.id IS NULL
                    """
                )
                if cur.fetchone()[0]:
                    raise RuntimeError("Missing homes for some climate rows")

                cur.execute(
                    """
                    SELECT COUNT(*)
                    FROM tmp_weather w
                    LEFT JOIN homes h ON h.tado_home_id = w.tado_home_id
                    WHERE h.id IS NULL
                    """
                )
                if cur.fetchone()[0]:
                    raise RuntimeError("Missing homes for some weather rows")

                cur.execute(
                    """
                    SELECT COUNT(*)
                    FROM tmp_climate c
                    JOIN homes h ON h.tado_home_id = c.tado_home_id
                    LEFT JOIN zones z ON z.home_id = h.id AND z.tado_zone_id = c.tado_zone_id
                    WHERE c.tado_zone_id IS NOT NULL AND z.id IS NULL
                    """
                )
                if cur.fetchone()[0]:
                    raise RuntimeError("Missing zones for some climate rows")

                display_sql_preview(cur)

                cur.execute(
                    """
                    WITH mapped AS (
                        SELECT
                            c.time,
                            h.id AS home_id,
                            CASE WHEN c.tado_zone_id IS NULL THEN NULL ELSE z.id END AS zone_id,
                            CASE WHEN NULLIF(c.tado_device_id, '') IS NULL
                                THEN NULL
                                ELSE d.id
                            END AS device_id,
                            c.inside_temp_c,
                            c.humidity_pct,
                            c.setpoint_temp_c,
                            c.heating_power_pct,
                            c.source
                        FROM tmp_climate c
                        JOIN homes h ON h.tado_home_id = c.tado_home_id
                        LEFT JOIN zones z ON z.home_id = h.id AND z.tado_zone_id = c.tado_zone_id
                        LEFT JOIN devices d ON d.home_id = h.id AND d.tado_device_id = NULLIF(c.tado_device_id, '')
                    ), upsert AS (
                        INSERT INTO climate_measurements (
                            time,
                            home_id,
                            zone_id,
                            device_id,
                            source,
                            inside_temp_c,
                            humidity_pct,
                            setpoint_temp_c,
                            heating_power_pct,
                            ac_power_on,
                            ac_mode,
                            window_open,
                            battery_low,
                            connection_up
                        )
                        SELECT
                            time,
                            home_id,
                            zone_id,
                            device_id,
                            source,
                            inside_temp_c,
                            humidity_pct,
                            setpoint_temp_c,
                            heating_power_pct,
                            NULL,
                            NULL,
                            NULL,
                            NULL,
                            NULL
                        FROM mapped
                        ON CONFLICT (time, home_id, source, zone_id, device_id)
                        DO UPDATE SET
                            inside_temp_c = COALESCE(EXCLUDED.inside_temp_c, climate_measurements.inside_temp_c),
                            humidity_pct = COALESCE(EXCLUDED.humidity_pct, climate_measurements.humidity_pct),
                            setpoint_temp_c = COALESCE(EXCLUDED.setpoint_temp_c, climate_measurements.setpoint_temp_c),
                            heating_power_pct = COALESCE(EXCLUDED.heating_power_pct, climate_measurements.heating_power_pct),
                            ac_power_on = COALESCE(EXCLUDED.ac_power_on, climate_measurements.ac_power_on),
                            ac_mode = COALESCE(EXCLUDED.ac_mode, climate_measurements.ac_mode),
                            window_open = COALESCE(EXCLUDED.window_open, climate_measurements.window_open),
                            battery_low = COALESCE(EXCLUDED.battery_low, climate_measurements.battery_low),
                            connection_up = COALESCE(EXCLUDED.connection_up, climate_measurements.connection_up)
                        RETURNING (xmax = 0) AS inserted
                    )
                    SELECT
                        COALESCE(SUM(CASE WHEN inserted THEN 1 ELSE 0 END), 0) AS inserted,
                        COALESCE(SUM(CASE WHEN inserted THEN 0 ELSE 1 END), 0) AS updated
                    FROM upsert
                    """
                )
                climate_inserted, climate_updated = cur.fetchone()

                cur.execute(
                    """
                    WITH mapped AS (
                        SELECT
                            w.time,
                            h.id AS home_id,
                            w.outside_temp_c,
                            NULLIF(w.weather_state, '') AS weather_state,
                            w.source
                        FROM tmp_weather w
                        JOIN homes h ON h.tado_home_id = w.tado_home_id
                    ), upsert AS (
                        INSERT INTO weather_measurements (
                            time,
                            home_id,
                            source,
                            outside_temp_c,
                            solar_intensity_pct,
                            weather_state
                        )
                        SELECT
                            time,
                            home_id,
                            source,
                            outside_temp_c,
                            NULL,
                            weather_state
                        FROM mapped
                        ON CONFLICT (home_id, time, source)
                        DO UPDATE SET
                            outside_temp_c = COALESCE(EXCLUDED.outside_temp_c, weather_measurements.outside_temp_c),
                            solar_intensity_pct = COALESCE(EXCLUDED.solar_intensity_pct, weather_measurements.solar_intensity_pct),
                            weather_state = COALESCE(EXCLUDED.weather_state, weather_measurements.weather_state)
                        RETURNING (xmax = 0) AS inserted
                    )
                    SELECT
                        COALESCE(SUM(CASE WHEN inserted THEN 1 ELSE 0 END), 0) AS inserted,
                        COALESCE(SUM(CASE WHEN inserted THEN 0 ELSE 1 END), 0) AS updated
                    FROM upsert
                    """
                )
                weather_inserted, weather_updated = cur.fetchone()

                if dry_run_rollback:
                    conn.rollback()
                else:
                    conn.commit()

                return climate_inserted, climate_updated, weather_inserted, weather_updated
    except psycopg.Error as exc:
        raise RuntimeError(f'Database error: {exc}')



def run(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="Transfer Influx historical export into PostgreSQL")
    parser.add_argument("csv", help="Path to the raw CSV exported via influx query")
    parser.add_argument(
        "--parse-only",
        action="store_true",
        help="Parse the CSV and report counts without touching the database",
    )
    parser.add_argument(
        "--rollback",
        action="store_true",
        help="Execute database writes inside a transaction that is rolled back at the end",
    )
    args = parser.parse_args(argv)

    if args.parse_only and args.rollback:
        print("--parse-only and --rollback cannot be combined", file=sys.stderr)
        return 1

    if not os.path.isfile(args.csv):
        print(f"Input file not found: {args.csv}", file=sys.stderr)
        return 1

    summary = parse_csv(args.csv)
    write_summary(summary)
    display_sample(summary)

    if args.parse_only:
        print("Dry run: CSV parsed; database untouched", file=sys.stderr)
        print("Parse-only dry run complete")
        return 0

    database_url = os.environ.get("DATABASE_URL")
    if not database_url:
        print("DATABASE_URL must be set", file=sys.stderr)
        return 1

    try:
        climate_inserted, climate_updated, weather_inserted, weather_updated = upsert_into_database(
            database_url, summary, args.rollback
        )
    except RuntimeError as exc:
        print(str(exc), file=sys.stderr)
        return 1

    print(
        f"Climate rows inserted: {climate_inserted}, updated: {climate_updated}",
        file=sys.stderr,
    )
    print(
        f"Weather rows inserted: {weather_inserted}, updated: {weather_updated}",
        file=sys.stderr,
    )
    if args.rollback:
        print("Dry run: transaction rolled back", file=sys.stderr)
        print("Rollback dry run complete")
    else:
        print("Import complete")
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entry point
    sys.exit(run())
