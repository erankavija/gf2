#!/usr/bin/env bash
# Tests smoke-campaign-arms.sh, which lies beside this file, without building
# or dispatching.
#
# Usage: smoke-campaign-arms.test.sh
#
# The script runs in a scratch repository whose cargo, plan projection
# and runner are stubs that record what they were handed, over synthetic addenda
# that carry cell roles and nothing else. The cases assert what the script
# decides itself: the label of the projected plan the runner writes the record
# for.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "${WORK}"' EXIT

mkdir -p "${WORK}/.cargo/bin" "${WORK}/target/release"
git -C "${WORK}" init -q
cp "${HERE}/smoke-campaign-arms.sh" "${WORK}/"

# Writes an addendum whose cells carry the given roles.
addendum() {
    local name=$1
    shift
    python3 - "${WORK}/${name}.json" "$@" <<'PY'
import json, sys
json.dump({"cells": [{"role": role} for role in sys.argv[2:]]}, open(sys.argv[1], "w"))
PY
}
addendum pilot exploratory exploratory
addendum confirmation confirmatory confirmatory
addendum holdout holdout

# The script puts ${HOME}/.cargo/bin first on PATH; with HOME at the scratch
# repository, this cargo is the one it finds, and it builds nothing.
printf '#!/usr/bin/env bash\n' >"${WORK}/.cargo/bin/cargo"
# The projection stub writes a plan holding the label it was handed.
cat >"${WORK}/make-plan.py" <<'PY'
import argparse, json
parser = argparse.ArgumentParser()
parser.add_argument("--label", required=True)
parser.add_argument("--output", required=True)
arguments, _ = parser.parse_known_args()
json.dump({"label": arguments.label}, open(arguments.output, "w"))
PY
# The runner stub accepts `check` and answers `smoke <plan> --record <path>`
# with the plan's label.
cat >"${WORK}/target/release/benchmark-ab-runner" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
case "$1" in
    check) ;;
    smoke)
        [[ "$3" == --record ]]
        python3 -c 'import json,sys; print("runner", json.load(open(sys.argv[1]))["label"])' \
            "$2" >"$4" ;;
    *) exit 2 ;;
esac
SH
chmod +x "${WORK}/.cargo/bin/cargo" "${WORK}/target/release/benchmark-ab-runner"

# Runs the script over one addendum; further arguments are passed on.
smoke() {
    local stage=$1
    shift
    (cd "${WORK}" && HOME="${WORK}" ./smoke-campaign-arms.sh \
        --issue fixture \
        --addendum "${stage}.json" \
        --arm-manifest unused/Cargo.toml \
        --arm-bin fixture-arm \
        --plan-tool make-plan.py \
        --producing unused/producing-inputs.json \
        --record "record-${stage}.txt" \
        --campaign-id "fixture-${stage}-smoke" \
        --seed 1 \
        --max-cells 3 \
        "$@" >/dev/null 2>&1)
}

expect_record() {
    local stage=$1 expected=$2 observed
    observed=$(cat "${WORK}/record-${stage}.txt")
    [[ "${observed}" == "${expected}" ]] || {
        echo "${stage}: record is '${observed}', expected '${expected}'" >&2
        exit 1
    }
}

smoke pilot --pilot-pairs 12
expect_record pilot 'runner pilot'
smoke confirmation
expect_record confirmation 'runner confirmation'

# An addendum whose cells name no smoked stage is refused before any dispatch.
if smoke holdout; then
    echo 'the script smoked an addendum with no exploratory or confirmatory cell' >&2
    exit 1
fi
[[ ! -e "${WORK}/record-holdout.txt" ]] || {
    echo 'a refused smoke wrote a record' >&2
    exit 1
}

echo 'smoke-campaign-arms: every case passed'
