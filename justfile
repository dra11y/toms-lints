set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

# Default recipe
default: help

# Show available recipes
help:
  just --list

# Build the crate
build:
  cargo build

# Run tests
test:
  cargo test

# Run clippy with warnings as errors
clippy:
  cargo clippy --all-targets -- -D warnings

# Format
fmt:
  cargo fmt --all

# Bless UI tests: update *.stderr files from the latest failing run output.
# Works across multiple lint crates if they live somewhere under the workspace root.
bless: && test
  #!/usr/bin/env bash
  set -euo pipefail

  out=$(mktemp) \
  && (cargo --color always test 2>&1 | tee "$out" || true) \
  && grep 'Actual stderr saved to ' "$out" | while IFS= read -r line; do \
       path=$(printf '%s' "$line" | sed 's/.*Actual stderr saved to \([^ ]*\).*/\1/'); \
       [ -f "$path" ] || continue; \
       base=$(basename "$path"); \
       test_name="${base%.stage-id.stderr}"; \
       test_rs=$(find . -type f -path "*/ui/$test_name.rs" -print -quit); \
       [ -n "$test_rs" ] || { echo "Skipping $test_name (no matching ui/$test_name.rs)" >&2; continue; }; \
       dir=$(dirname "$test_rs"); \
       cp "$path" "$dir/$test_name.stderr"; \
       echo "Blessed $dir/$test_name.stderr"; \
     done

# Bless a single test: just bless-one <name> (without .rs)
bless-one name:
  out=$(mktemp) \
  && (cargo test -- --test-args "{{name}}.rs" 2>&1 | tee "$out" || true) \
  && grep 'Actual stderr saved to ' "$out" | while IFS= read -r line; do \
       path=$(printf '%s' "$line" | sed 's/.*Actual stderr saved to \([^ ]*\).*/\1/'); \
       [ -f "$path" ] || continue; \
       base=$(basename "$path"); \
       test_name="${base%.stage-id.stderr}"; \
       test_rs=$(find . -type f -path "*/ui/$test_name.rs" -print -quit); \
       [ -n "$test_rs" ] || { echo "Skipping $test_name (no matching ui/$test_name.rs)" >&2; continue; }; \
       dir=$(dirname "$test_rs"); \
       cp "$path" "$dir/$test_name.stderr"; \
       echo "Blessed $dir/$test_name.stderr"; \
     done \
  && cargo test -- --test-args "{{name}}.rs"

# Clean build artifacts
clean:
  cargo clean
