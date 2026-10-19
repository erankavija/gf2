#!/usr/bin/env bash
# Runs the profile summarizer as committed on main and the working-tree one on
# every committed series that carries an attribution-chains.json, each in its
# own scratch repository, and compares every produced file with cmp.
# Usage: reproduce-summaries.sh [baseline-ref]   (default: main)
set -euo pipefail

BASE="${1:-main}"
ROOT="$(git rev-parse --show-toplevel)"
SURVEY="dev/active/1a379447-zen3-cpu-performance/3be770d5/survey"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "${SCRATCH}"' EXIT
cd "${ROOT}"

# The summarizer takes its repository root from its own location, reads the
# build identity and structural costs below it, and records the series path
# relative to it; each scratch repository gives both runs the same layout.
stage() {
    local name="$1" source="$2"
    git init -q "${SCRATCH}/${name}"
    ln -s "${ROOT}/dev" "${SCRATCH}/${name}/dev"
    mkdir "${SCRATCH}/${name}/survey"
    for file in summarize-profile.py intervals.py; do
        if [ "${source}" = tree ]; then
            cp "${SURVEY}/${file}" "${SCRATCH}/${name}/survey/${file}"
        else
            git show "${source}:${SURVEY}/${file}" > "${SCRATCH}/${name}/survey/${file}"
        fi
    done
}
stage old "${BASE}"
stage new tree

status=0
while IFS= read -r chains; do
    series="$(dirname "${chains}")"
    for name in old new; do
        rm -rf "${SCRATCH}/${name}/series"
        cp -r "${series}" "${SCRATCH}/${name}/series"
        python3 -B "${SCRATCH}/${name}/survey/summarize-profile.py" "${SCRATCH}/${name}/series" \
            --json "${SCRATCH}/${name}/series/profile-summary.json" \
            --markdown "${SCRATCH}/${name}/series/profile.md" > /dev/null 2>&1
    done
    verdict=identical
    while IFS= read -r file; do
        cmp -s "${SCRATCH}/old/series/${file}" "${SCRATCH}/new/series/${file}" || verdict=different
    done < <(cd "${SCRATCH}/old/series" && find . -type f | sort)
    [ "$(cd "${SCRATCH}/old/series" && find . -type f | sort)" = \
      "$(cd "${SCRATCH}/new/series" && find . -type f | sort)" ] || verdict=different
    [ "${verdict}" = identical ] || status=1
    printf '%s\t%s\n' "${series}" "${verdict}"
done < <(git ls-files '*/attribution-chains.json' | sort)
exit "${status}"
