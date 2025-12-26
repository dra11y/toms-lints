set shell := ["bash", "-eu", "-o", "pipefail", "-c"]
set positional-arguments
set export
color := "always"

# Show available recipes
help:
  just --list

# Update Rust toolchain to the latest nightly
bump-toolchain: && clean (test '--workspace')
  #!/usr/bin/env bash
  set -euo pipefail
  # get pinned nightly toolchain version from clippy
  toolchain=$(curl -s https://raw.githubusercontent.com/rust-lang/rust-clippy/master/rust-toolchain.toml)
  nightly=$(echo "$toolchain" | grep 'channel =' | sed -E 's/.*channel = "(nightly-[0-9]{4}-[0-9]{2}-[0-9]{2})".*/\1/')
  sed -i '' -E "s/^\s*channel =.+$/channel = \"$nightly\"/" rust-toolchain.toml
  cargo clean
  rm -f Cargo.lock
  cargo update
  which dylint-link || cargo binstall -y dylint-link
  echo "Updated rust-toolchain.toml to use $nightly"

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

check:
  cargo check

# Format
fmt:
  cargo fmt --all

# Clean build artifacts
clean:
  cargo clean
