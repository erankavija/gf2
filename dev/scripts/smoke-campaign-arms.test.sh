#!/usr/bin/env bash
# Tests dev/scripts/smoke-campaign-arms.sh without building or dispatching.
#
# Usage (from the repository root): dev/scripts/smoke-campaign-arms.test.sh
#
# The script runs in a scratch repository whose cargo wrapper, runner and
# family smoke binary are stubs that record what they were handed, over the
# committed pilot and confirmation addenda of a real family and its real plan
# projection. The cases assert what the script decides itself: the label of the
# projected plan and which executable writes the record.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FAMILY=dev/active/ad2a6a58
WORK="$(mktemp -d)"
trap 'rm -rf "${WORK}"' EXIT

mkdir -p "${WORK}/dev/scripts" "${WORK}/scripts" "${WORK}/${FAMILY}/survey" \
    "${WORK}/target/release" "${WORK}/target/fixture-arm/release"
git -C "${WORK}" init -q
cp "${ROOT}/dev/scripts/smoke-campaign-arms.sh" "${ROOT}/dev/scripts/campaign_plan.py" \
    "${WORK}/dev/scripts/"
cp "${ROOT}/${FAMILY}/survey/make-plan.py" "${WORK}/${FAMILY}/survey/"
cp "${ROOT}/${FAMILY}"/addendum-v4-axpy-{pilot,confirmation}.json "${WORK}/${FAMILY}/"

# The cargo wrapper builds nothing here.
printf '#!/usr/bin/env bash\n' >"${WORK}/scripts/cargo-budget.sh"
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
cat >"${WORK}/target/fixture-arm/release/family-smoke" <<'SH'
#!/usr/bin/env bash
python3 -c 'import json,sys; print("family", json.load(open(sys.argv[1]))["label"])' "$1"
SH
chmod +x "${WORK}/scripts/cargo-budget.sh" "${WORK}/target/release/benchmark-ab-runner" \
    "${WORK}/target/fixture-arm/release/family-smoke"

# Runs the script over one stage's addendum; further arguments are passed on.
smoke() {
    local stage=$1
    shift
    (cd "${WORK}" && dev/scripts/smoke-campaign-arms.sh \
        --issue fixture \
        --addendum "${FAMILY}/addendum-v4-axpy-${stage}.json" \
        --arm-manifest unused/Cargo.toml \
        --arm-bin fixture-arm \
        --plan-tool "${FAMILY}/survey/make-plan.py" \
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

# A caller naming its own smoke binary gets that binary's record, labelled the
# same way.
smoke confirmation --smoke-bin family-smoke
expect_record confirmation 'family confirmation'

# An addendum whose cells name no smoked stage is refused before any dispatch.
python3 - "${WORK}/${FAMILY}/addendum-v4-axpy-pilot.json" <<'PY'
import json, sys
addendum = json.load(open(sys.argv[1]))
for cell in addendum["cells"]:
    cell["role"] = "holdout"
json.dump(addendum, open(sys.argv[1], "w"))
PY
rm "${WORK}/record-pilot.txt"
if smoke pilot --pilot-pairs 12; then
    echo 'the script smoked an addendum with no exploratory or confirmatory cell' >&2
    exit 1
fi
[[ ! -e "${WORK}/record-pilot.txt" ]] || {
    echo 'a refused smoke wrote a record' >&2
    exit 1
}

echo 'smoke-campaign-arms: every case passed'
