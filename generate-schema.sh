#!/bin/sh

# Simple end-to-end helper to:
# - Start TimescaleDB (docker compose)
# - Apply Diesel migrations
# - Build the crate (type-check models)
# - Generate src/schema.rs from the live DB
# - Show TimescaleDB hypertables
#
# Assumptions: docker, docker compose v2, psql, cargo, and ./diesel.sh are available.

set -eu

echo "=== 1) Start TimescaleDB container ==="
docker compose up -d db

echo "=== 2) Configure DATABASE_URL ==="
: "${DATABASE_URL:=postgres://postgres:postgres@localhost:5432/tado}"
echo "Using DATABASE_URL=$DATABASE_URL"

echo "=== 3) Apply Diesel migrations ==="
DATABASE_URL="$DATABASE_URL" ./diesel.sh migration run

echo "=== 4) Generate schema.rs from DB ==="
mkdir -p src/db
DATABASE_URL="$DATABASE_URL" ./diesel.sh print-schema > src/schema.rs
echo "Wrote schema to src/schema.rs"

echo "=== 5) Build crate (type-check models) ==="
cargo build

echo "=== 6) Verify hypertables ==="
psql "$DATABASE_URL" -c "select hypertable_name from timescaledb_information.hypertables order by 1;"

echo "=== Done ==="
