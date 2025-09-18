#!/bin/sh

set -eu

SCRIPT_DIR="$(dirname "$(realpath "$0")")"

utils_dir="$SCRIPT_DIR/target/utils/"
mkdir -p "$utils_dir"

if ! [ -f "$utils_dir/diesel" ]; then
    curl -fsSLo - https://github.com/diesel-rs/diesel/releases/download/v2.3.0/diesel_cli-x86_64-unknown-linux-gnu.tar.xz | tar xJf - -C "$utils_dir" --transform 's|.*/|/|' --show-transformed-names diesel_cli-x86_64-unknown-linux-gnu/diesel
fi

"$utils_dir/diesel" "$@"
