#!/bin/bash
# This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
# the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.
#
# Everything Polter is checked with before "done", each step on its own exit code (never through
# a pipe), with a summary at the end:
#
#   tools/check.sh          tests, clippy, rustfmt, golden frames, a new game both ways
#   tools/check.sh --quick  without the golden frames and the new game (the slowest steps)
#
# Exit 0 when every step passed, 1 otherwise. Output of each step goes to a log in a temporary
# folder; a failing step shows its last lines.
set -u
export PATH="$HOME/.cargo/bin:$PATH"
cd "$(dirname "$0")/.."
logs="$(mktemp -d)"
failed=0
summary=""

step() {
  local name="$1"
  shift
  local log="$logs/$(echo "$name" | tr ' ' '_').log"
  local start=$SECONDS
  "$@" > "$log" 2>&1
  local code=$?
  local took=$((SECONDS - start))
  if [ $code -eq 0 ]; then
    summary+="  ✓ $name (${took} s)"$'\n'
  else
    summary+="  ✗ $name: exit $code (${took} s), log $log"$'\n'
    failed=1
    echo "✗ $name failed, last lines:"
    tail -15 "$log"
  fi
}

# A game made by `polter new` passes clippy as it is made (with the tools) and as it ships
# (without them), so the template can't rot. Its list of games is a temporary one.
new_game_both_ways() {
  local dir="$logs/new_game"
  POLTER_PROJECTS="$logs/projects.txt" target/release/polter new "$dir/Fresh" || return 1
  local target="$PWD/target/new_game"
  CARGO_TARGET_DIR="$target" cargo clippy --release --manifest-path "$dir/Fresh/Cargo.toml" -- -D warnings || return 1
  CARGO_TARGET_DIR="$target" cargo clippy --release --no-default-features --manifest-path "$dir/Fresh/Cargo.toml" -- -D warnings
}

step "cargo test" cargo test --workspace --release
# a warning fails the step (-D warnings), so its exit code says it all
step "clippy" cargo clippy --workspace --all-targets --release -- -D warnings
step "rustfmt" cargo fmt --all --check
if [ "${1:-}" != "--quick" ]; then
  # the golden frames run the carts' programs: build them first, or an old program is checked
  step "build carts" cargo build --release --workspace
  step "golden frames" python3 tests/golden/make.py check
  step "new game both ways" new_game_both_ways
fi

# how many tests passed, from the test log
passed=$(grep -hoE 'test result: ok\. [0-9]+ passed' "$logs/cargo_test.log" 2>/dev/null | awk '{s += $4} END {print s + 0}')
echo "Polter check:"
printf "%s" "$summary"
echo "  $passed tests passed"
echo "  logs: $logs"
exit $failed
