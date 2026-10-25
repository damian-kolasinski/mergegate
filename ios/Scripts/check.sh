#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
mode="${1:-all}"
if [ $# -gt 0 ]; then shift; fi
python3 ../appearance/generate.py --check
mkdir -p .build
# mise.toml pins the same Tuist for local generation and CI.
tuist generate --no-open >.build/generate.log 2>&1 || { cat .build/generate.log; exit 1; }
action=test
if [ "$mode" = lint ]; then action=build; fi
args=()
if [ -n "${MG_IOS_TEST_HUB_URL:-}" ]; then args+=("MG_IOS_TEST_HUB_URL=$MG_IOS_TEST_HUB_URL"); fi
if [ -n "${MG_IOS_TEST_TRIAGE:-}" ]; then args+=("MG_IOS_TEST_TRIAGE=$MG_IOS_TEST_TRIAGE"); fi
if [ -n "${MG_IOS_TEST_REVIEW:-}" ]; then args+=("MG_IOS_TEST_REVIEW=$MG_IOS_TEST_REVIEW"); fi
if [ -n "${MG_IOS_TEST_SOURCE:-}" ]; then args+=("MG_IOS_TEST_SOURCE=$MG_IOS_TEST_SOURCE"); fi
if [ -n "${MG_IOS_TEST_HIGHLIGHT:-}" ]; then args+=("MG_IOS_TEST_HIGHLIGHT=$MG_IOS_TEST_HIGHLIGHT"); fi
if [ -n "${MG_IOS_TEST_VIEWED:-}" ]; then args+=("MG_IOS_TEST_VIEWED=$MG_IOS_TEST_VIEWED"); fi
if [ -n "${MG_IOS_TEST_DRAFTS:-}" ]; then args+=("MG_IOS_TEST_DRAFTS=$MG_IOS_TEST_DRAFTS"); fi
if ! xcodebuild "$action" -workspace Mergegate.xcworkspace -scheme Mergegate \
  -destination "${MG_IOS_DESTINATION:-platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2}" \
  -derivedDataPath .build/DerivedData CODE_SIGNING_ALLOWED=NO \
  ${args[@]+"${args[@]}"} "$@" >.build/check.log 2>&1; then
  tail -80 .build/check.log
  exit 1
fi
grep -E 'Test (Suite|Case).*passed|Executed |passed after|BUILD SUCCEEDED|TEST SUCCEEDED' .build/check.log | tail -12
