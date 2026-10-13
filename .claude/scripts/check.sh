#!/usr/bin/env bash
# The checks CI runs, with output sized for agents: "OK - ..." on success,
# otherwise the first failing step's diagnostics. VERBOSE=1 prints cargo's
# full output instead.
#
# Usage: .claude/scripts/check.sh [all|lint|test] [CRATE...|ui] [-- TEST_ARGS...]
#   all (default)  lint, then tests; stops at the first failure
#   lint           cargo fmt, clippy, then the UI's Biome, ESLint and tsc
#   test           tests only, without formatting
#   CRATE          a directory under crates/ (core, cli, ...); limits clippy
#                  and tests to those crates
#   ui             the UI in ui/; checked when no CRATE is named, skipped
#                  then if pnpm is missing
#   TEST_ARGS      passed to the test binaries, e.g. a test name filter
# Exit codes: 0 ok, 1 a check failed, 64 usage.

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/../.."
VERBOSE="${VERBOSE:-0}"

# rustup's proxies pick the toolchain from rust-toolchain.toml. Homebrew's
# rustup is keg-only, so they are often not on PATH.
if ! command -v cargo >/dev/null; then
  for dir in "$HOME/.cargo/bin" /opt/homebrew/opt/rustup/bin /usr/local/opt/rustup/bin; do
    if [ -x "$dir/cargo" ]; then
      PATH="$dir:$PATH"
      break
    fi
  done
fi
if ! command -v cargo >/dev/null; then
  echo "SETUP_ERROR: cargo not found; install rustup"
  exit 1
fi

mode=all
case "${1:-}" in
  all | lint | test) mode="$1"; shift ;;
esac

packages=()
# ui: 0 skip, 1 named (pnpm required), auto (no target named)
ui=auto
while [ $# -gt 0 ] && [ "$1" != "--" ]; do
  if [ "$ui" = auto ]; then ui=0; fi
  if [ "$1" = ui ]; then
    ui=1
    shift
    continue
  fi
  manifest="crates/$1/Cargo.toml"
  if [ ! -f "$manifest" ]; then
    echo "USAGE_ERROR: no crate at crates/$1"
    exit 64
  fi
  packages+=(-p "$(sed -n 's/^name = "\(.*\)"$/\1/p' "$manifest" | head -1)")
  shift
done
if [ $# -gt 0 ]; then shift; fi
rust=1
if [ "$ui" = auto ]; then
  packages=(--workspace)
elif [ ${#packages[@]} -eq 0 ]; then
  rust=0
fi
ui_skipped=0
if [ "$ui" = auto ]; then
  if command -v pnpm >/dev/null; then ui=1; else ui=0 ui_skipped=1; fi
fi
if [ "$ui" = 1 ] && ! command -v pnpm >/dev/null; then
  echo "SETUP_ERROR: pnpm not found; install it (mise use pnpm, or corepack enable)"
  exit 1
fi
# bash 3.2 (macOS) treats an empty array as unset under `set -u`.
test_args=("--" "$@")

output=""
toolchain_retried=0

# Runs a command, keeping its combined output in $output. A fresh checkout
# may lack the pinned toolchain; install it and retry once.
run() {
  if output="$("$@" 2>&1)"; then
    return 0
  fi
  if [ "$toolchain_retried" = 0 ] && grep -q "is not installed" <<<"$output"; then
    toolchain_retried=1
    if ! rustup toolchain install >/dev/null 2>&1; then
      echo "SETUP_ERROR: rustup toolchain install failed; run it to see why"
      exit 1
    fi
    run "$@"
    return
  fi
  return 1
}

# Compiler and clippy diagnostics, one line each (--message-format=short).
# $1 picks the severities: clippy denies warnings, so they are its errors;
# under cargo test they are noise next to a failing test.
diagnostics() {
  grep -E "^[^ ]+:[0-9]+:[0-9]+: ($1)|^error(\[|:)" <<<"$output" |
    grep -v -E 'could not compile|aborting due to|test failed, to rerun' |
    head -10 || true
}

fail() {
  local label="$1" detail="$2"
  if [ "$VERBOSE" = 1 ]; then
    echo "$output"
  elif [ -n "$detail" ]; then
    echo "$label: $detail"
  else
    echo "ERROR_EXTRACTION_FAILED: $label; last lines follow, rerun with VERBOSE=1 for all"
    tail -15 <<<"$output"
  fi
  exit 1
}

summary=()
# Unlike CI, no --locked: a new dependency should update Cargo.lock here, and
# the summary says so, so the lockfile gets committed and CI's --locked passes.
lock_before="$(cksum Cargo.lock 2>/dev/null || true)"

if [ "$mode" != test ] && [ "$rust" = 1 ]; then
  run cargo fmt --all -- -l || fail FMT_ERROR "$(grep -E '^error' -A3 <<<"$output" | head -10)"
  formatted="$(grep -c '\.rs$' <<<"$output" || true)"
  if [ "$formatted" -gt 0 ]; then summary+=("rustfmt changed $formatted file(s)"); fi

  run cargo clippy "${packages[@]}" --all-targets --message-format=short -- -D warnings ||
    fail CLIPPY_ERROR "$(diagnostics 'error|warning')"
  summary+=("clippy clean")
fi

# The UI tools print their own short diagnostics; the tail holds them.
ui_run() {
  local label="$1"
  shift
  run pnpm --dir ui "$@" || fail "$label" "$(grep -v '^[[:space:]]*$' <<<"$output" | tail -20)"
}

if [ "$mode" != test ] && [ "$ui" = 1 ]; then
  ui_run UI_INSTALL_ERROR install --frozen-lockfile --prefer-offline
  ui_run UI_FORMAT_ERROR format
  fixed="$(sed -n -E 's/.*Fixed ([0-9]+) file.*/\1/p' <<<"$output")"
  if [ -n "$fixed" ]; then summary+=("biome changed $fixed file(s)"); fi
  ui_run UI_LINT_ERROR lint
  ui_run UI_TYPE_ERROR typecheck
  summary+=("UI clean")
fi
if [ "$ui_skipped" = 1 ]; then summary+=("UI skipped, no pnpm"); fi

if [ "$mode" != lint ] && [ "$rust" = 1 ]; then
  if ! run cargo test "${packages[@]}" --message-format=short "${test_args[@]}"; then
    detail="$(diagnostics error)"
    if [ -z "$detail" ]; then
      failed="$(sed -n -E 's/^---- (.+) stdout ----$/\1/p' <<<"$output" | head -10 | paste -sd, -)"
      if [ -n "$failed" ]; then
        detail="$(
          echo "failed: $failed"
          grep -A4 -m2 'panicked at' <<<"$output" || true
          grep -m1 'to rerun pass' <<<"$output" || true
        )"
      fi
    fi
    fail TEST_ERROR "$detail"
  fi
  passed="$({ grep -oE 'test result: ok\. [0-9]+ passed' <<<"$output" || true; } | awk '{n += $4} END {print n + 0}')"
  summary=("$passed tests passed" ${summary[@]+"${summary[@]}"})
fi

if [ "$(cksum Cargo.lock 2>/dev/null || true)" != "$lock_before" ]; then
  summary+=("Cargo.lock updated, commit it")
fi

line="OK"
for part in ${summary[@]+"${summary[@]}"}; do
  if [ "$line" = OK ]; then line="OK - $part"; else line="$line, $part"; fi
done
echo "$line"
