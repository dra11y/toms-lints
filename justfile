set shell := ["bash", "-eu", "-o", "pipefail", "-c"]
set positional-arguments
set export
color := "always"

# Default recipe
default: help

# Show available recipes
help:
  just --list

# Build the crate
build:
  cargo build

# Run tests
[no-cd]
test *args="":
  # cargo test {{args}} 2>&1 | grep -v '$message_type' | grep -E '^identifier:'
  cargo test {{args}} 2>&1 | grep -v '$message_type'

# Run clippy with warnings as errors
clippy:
  cargo clippy --all-targets -- -D warnings

# Format
fmt:
  cargo fmt --all

# Bless UI tests: update *.stderr files from the latest failing run output.
# Usage:
#   just bless              - In workspace: bless all crates. In crate: bless current crate.
#   just bless <crate_name> - bless crate (from anywhere)
#   just bless all          - bless all crates (from anywhere)
[no-cd]
bless *crate_name:
  #!/usr/bin/env bash
  set -euo pipefail
  cargo test 2>&1 | grep -v '$message_type' | while IFS= read -r line; do
    [[ "$line" != *"Actual stderr saved to "* ]] && { echo "$line"; continue; }
    path=$(printf '%s' "$line" | sed 's/.*Actual stderr saved to \([^ ]*\).*/\1/')
    [[ ! -f "$path" ]] && continue
    test_name="${path##*/}"; test_name="${test_name%.stage-id.stderr}"
    test_rs=$(find . -name "$test_name.rs" -path "*/ui/*" -print -quit)
    [[ -z "$test_rs" ]] && { echo "No matching test file for: $test_name"; exit 1; }
    [[ -n "$test_rs" ]] && cp "$path" "$(dirname "$test_rs")/$test_name.stderr" && echo "Blessed $(dirname "$test_rs")/$test_name.stderr"
  done

# Clean build artifacts
clean:
  cargo clean
