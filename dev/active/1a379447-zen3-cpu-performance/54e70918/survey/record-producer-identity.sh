#!/usr/bin/env bash
# Builds the offline tuning producer under Rust 1.95 from the working tree and
# from the producer source held under `inputs/before/`, and writes
# `producer-identity.json` beside this script (jit:54e70918): for each, the
# digest of the source, the digest of the release binary, and the behaviour
# token the tuning module declares. The working file is restored on exit.
#
# Usage: record-producer-identity.sh
set -euo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel)
cd "$root"
export RUSTUP_TOOLCHAIN=1.95.0

before=$(cd "$here" && git ls-files --cached --others --exclude-standard -- 'inputs/before/**/tuning_calibration.rs')
source=${before#inputs/before/}
features=parallel,simd,test-support,tuning-profile
token=$(git grep -h -o 'CORE_HARNESS_SCHEMA: &str = "[^"]*"' -- "${source%%/benches/*}/src" | cut -d'"' -f2)

kept=$(mktemp)
cp -- "$source" "$kept"
trap 'cp -- "$kept" "$source"; rm -f -- "$kept"' EXIT

observe() {
    local binary
    binary=$(nice -n 19 ./scripts/cargo-budget.sh cargo build --release -p gf2-core \
        --bench tuning_calibration --features "$features" --message-format json 2>/dev/null |
        python3 -c '
import json, sys
for line in sys.stdin:
    message = json.loads(line)
    if message.get("reason") == "compiler-artifact" and message.get("executable"):
        print(message["executable"])')
    printf '{"source_sha256": "%s", "binary_sha256": "%s"}' \
        "$(sha256sum "$source" | cut -d" " -f1)" "$(sha256sum "$binary" | cut -d" " -f1)"
}

tree=$(observe)
cp -- "$here/$before" "$source"
held=$(observe)

python3 - "$source" "$features" "$token" "$held" "$tree" > "$here/producer-identity.json" <<'PY'
import json, sys
source, features, token, held, tree = sys.argv[1:]
held, tree = json.loads(held), json.loads(tree)
print(json.dumps({
    "issue": "54e70918",
    "producer": source,
    "features": features,
    "behavior_token": token,
    "before": held,
    "working_tree": tree,
    "source_equal": held["source_sha256"] == tree["source_sha256"],
    "binary_equal": held["binary_sha256"] == tree["binary_sha256"],
}, indent=1))
PY
cat "$here/producer-identity.json"
