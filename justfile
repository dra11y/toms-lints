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
  cargo test {{args}}

# Bless all outputs
[no-cd]
bless *args="":
  BLESS=1 cargo test {{args}}

# Format
fmt:
  cargo fmt --all

# Clean build artifacts
clean:
  cargo clean
