#!/usr/bin/env bash
# This script can go in ~/bin/dylint and be used as a wrapper
# to automatically check and update nightly daily before running.
# It passes some default args to cargo dylint as well.

UPDATE_FILE="$HOME/.rustup_last_update"
# Check if we've updated today
if [[ ! -f "$UPDATE_FILE" || $(date +%Y-%m-%d) != $(date -r "$UPDATE_FILE" +%Y-%m-%d) ]]; then
    echo "Checking for nightly updates..."
    rustup update nightly
    touch "$UPDATE_FILE"
fi

# Split args by --
before_args=()
after_args=()
found_separator=false

for arg in "$@"; do
    if [[ "$arg" == "--" ]]; then
        found_separator=true
    elif [[ "$found_separator" == false ]]; then
        before_args+=("$arg")
    else
        after_args+=("$arg")
    fi
done

# incremental builds don't currently work with dylint!
set -x
CARGO_INCREMENTAL=0 cargo dylint "${before_args[@]}" --all --keep-going --no-deps -- --all-features --all-targets "${after_args[@]}"
