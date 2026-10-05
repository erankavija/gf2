#!/usr/bin/env bash
# Tests verify-source-evidence.py, which lies beside this file, against a
# committed source-evidence ledger.
#
# Usage: verify-source-evidence.test.sh
#
# The positive case is a committed ledger whose cited sources changed after it
# was written. Negative cases perturb one row of it; the tree-tracking case
# rebuilds its rows from a committed revision.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT=$(git -C "${HERE}" rev-parse --show-toplevel)
CHECKER="${HERE}/verify-source-evidence.py"
PROJECT=gf2

# A committed revision at which sources the ledger cites differ from the
# revisions its rows record.
TREE=fcb972da393c878665b1653856d76ddf5b299731

live() {
    python3 -B "${HERE}/repository_files.py" file "$1"
}

LEDGER="${ROOT}/$(live dense-parity-source-evidence.json)"
RECORD="${ROOT}/$(live dense-parity-source-evidence-verification.json)"
WORK="$(mktemp -d)"
trap 'rm -rf "${WORK}"' EXIT

run() {
    local ledger=$1
    shift
    python3 -B "${CHECKER}" "${ledger}" --project "${PROJECT}" --tree "${TREE}" "$@"
}

expect_refusal() {
    local ledger=$1 frame=$2 pattern=$3 output
    if output=$(run "${ledger}" --frame "${frame}" 2>&1); then
        echo "the checker accepted ${ledger} in frame ${frame}: ${output}" >&2
        exit 1
    fi
    grep -q -- "${pattern}" <<<"${output}" || {
        echo "refusal of ${ledger} does not mention ${pattern}: ${output}" >&2
        exit 1
    }
}

expect_class() {
    local ledger=$1 frame=$2 class=$3 output
    output=$(run "${ledger}" --frame "${frame}")
    grep -qx -- "class: ${class}" <<<"${output}" || {
        echo "${ledger} is not classed ${class}: ${output}" >&2
        exit 1
    }
}

# A copy of the ledger with one field of its first row replaced.
perturb() {
    python3 - "${LEDGER}" "$1" "$2" "$3" <<'PY'
import json, sys
source, field, value, output = sys.argv[1:5]
document = json.load(open(source))
row = document["claims"][0]
row[field] = type(row[field])(value)
json.dump(document, open(output, "w"), indent=2)
PY
}

# The ledger's rows rewritten to describe revision $1: the line each cited
# text occupies there, the file digest there and the commit that last changed
# the file.
retell_at() {
    python3 - "${LEDGER}" "$1" "$2" "${ROOT}" <<'PY'
import hashlib, json, subprocess, sys
source, revision, output, root = sys.argv[1:5]
def git(*arguments):
    return subprocess.run(["git", "-C", root, *arguments], capture_output=True, check=True).stdout
document = json.load(open(source))
for row in document["claims"]:
    content = git("show", f"{revision}:{row['path']}")
    lines = content.decode().splitlines()
    row["line"] = lines.index(row["verbatim"]) + 1
    row["sha256"] = hashlib.sha256(content).hexdigest()
    row["commit"] = git("log", "-1", "--format=%H", revision, "--", row["path"]).decode().strip()
json.dump(document, open(output, "w"), indent=2)
PY
}

# The committed ledger is pinned to its recorded commits and does not describe
# the compared tree.
expect_class "${LEDGER}" recorded-commits recorded-commits
expect_refusal "${LEDGER}" tree "differs from the tree"

# A ledger that tracks the compared tree holds in both frames and is classed
# apart from the pinned one.
retell_at "${TREE}" "${WORK}/tracking.json"
expect_class "${WORK}/tracking.json" tree tree
expect_class "${WORK}/tracking.json" recorded-commits tree

perturb line 1 "${WORK}/line.json"
expect_refusal "${WORK}/line.json" recorded-commits "line 1 of .* at the recorded commit is not the cited text"

perturb sha256 0000 "${WORK}/digest.json"
expect_refusal "${WORK}/digest.json" recorded-commits "digest of .* at the recorded commit is not the recorded one"

perturb occurrences 3 "${WORK}/occurrences.json"
expect_refusal "${WORK}/occurrences.json" recorded-commits "3 occurrences recorded"

perturb commit 0000000000000000000000000000000000000000 "${WORK}/commit.json"
expect_refusal "${WORK}/commit.json" recorded-commits "does not resolve"

perturb path no/such/file.rs "${WORK}/path.json"
expect_refusal "${WORK}/path.json" recorded-commits "does not resolve"

# A row of another project is outside the repository's history and is counted,
# never verified.
perturb project elsewhere "${WORK}/project.json"
run "${WORK}/project.json" --frame recorded-commits |
    grep -qx "rows of other projects, not verified: 1" || {
    echo "a row of another project is not counted apart" >&2
    exit 1
}

# The committed verification record reproduces from the committed ledger.
run "${LEDGER}" --frame recorded-commits \
    --record "${WORK}/record.json" >/dev/null
cmp "${WORK}/record.json" "${RECORD}"

echo "verify-source-evidence: all cases pass"
