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

  workspace_root="{{justfile_directory()}}"

  if [[ $# -eq 0 ]]; then
    # No args - check if we're in a crate or workspace
    current_dir="$(pwd)"

    if [[ "$current_dir" == "$workspace_root" ]]; then
      just _bless
    else
      relative_path="${current_dir#$workspace_root/}"

      if [[ "$relative_path" == "$current_dir" ]]; then
        just _bless
        exit 0
      fi

      crate=$(echo "$relative_path" | cut -d'/' -f1)

      if [[ -d "$workspace_root/$crate" ]] && [[ -f "$workspace_root/$crate/Cargo.toml" ]]; then
        just _bless "$crate"
      else
        echo "Error: Not in a valid crate directory"
        exit 1
      fi
    fi
  else
    # Specific crate requested
    crate_name="$1"

    if [[ "$crate_name" == "all" ]]; then
      # Special case: force bless all crates regardless of current directory
      just _bless
    else
      # Normal crate name
      if [[ ! -d "$workspace_root/$crate_name" ]]; then
        echo "Error: Crate directory '$crate_name' not found in workspace"
        exit 1
      fi

      just _bless "$crate_name"
    fi
  fi

_bless crate_name="":
  #!/usr/bin/env bash
  set -euo pipefail
  if [[ -z "{{crate_name}}" ]]; then
    echo "Blessing workspace..."
  else
    echo "Blessing crate: {{crate_name}}"
  fi
  temp_file=$(mktemp)
  cd "{{crate_name}}"
  cargo test 2>&1 | tee "$temp_file" || true
  just _process_stderr_files "$temp_file"

# Internal: process stderr files from test output
_process_stderr_files temp_file:
  #!/usr/bin/env bash
  set -euo pipefail

  if [[ ! -f "{{temp_file}}" ]]; then
    echo "No test output found"
    exit 1
  fi

  if grep -q 'Actual stderr saved to ' "{{temp_file}}"; then
    grep 'Actual stderr saved to ' "{{temp_file}}" | while IFS= read -r line; do
      path=$(printf '%s' "$line" | sed 's/.*Actual stderr saved to \([^ ]*\).*/\1/')
      [[ -f "$path" ]] || continue

      base=$(basename "$path")
      test_name="${base%.stage-id.stderr}"
      test_rs=$(find . -type f -path "*/ui/$test_name.rs" -print -quit)

      if [[ -z "$test_rs" ]]; then
        echo "Skipping $test_name (no matching ui/$test_name.rs)" >&2
        continue
      fi

      dir=$(dirname "$test_rs")
      cp "$path" "$dir/$test_name.stderr"
      echo "Blessed $dir/$test_name.stderr"
    done
  else
    echo "No stderr files to bless (all tests passed or no UI tests found)"
  fi

  rm -f "{{temp_file}}"

# Clean build artifacts
clean:
  cargo clean
