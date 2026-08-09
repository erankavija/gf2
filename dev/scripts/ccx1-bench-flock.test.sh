#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

mkdir "$TMP/bin"
printf '#!/usr/bin/env bash\nprintf "%%s\\n" "$@"\n' >"$TMP/bin/flock"
chmod +x "$TMP/bin/flock"
touch "$TMP/lock"

default=$(
  PATH="$TMP/bin:$PATH" GF2_CCX1_LOCK="$TMP/lock" \
    "$ROOT/dev/scripts/ccx1-bench-flock.sh" probe arg
)
full_host=$(
  PATH="$TMP/bin:$PATH" GF2_CCX1_LOCK="$TMP/lock" \
    "$ROOT/dev/scripts/ccx1-bench-flock.sh" --full-host probe arg
)

expected_default=$(printf '%s\n' -x "$TMP/lock" taskset -c 6-11 nice -n -5 probe arg)
expected_full_host=$(printf '%s\n' -x "$TMP/lock" nice -n -5 probe arg)

test "$default" = "$expected_default"
test "$full_host" = "$expected_full_host"
