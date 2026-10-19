#!/usr/bin/env bash
# Runs the profile summarizer as committed at a baseline revision and the
# working-tree one on every committed series that carries an
# attribution-chains.json, each in its own scratch repository, and compares
# every produced file with cmp.
# Usage: reproduce-summaries.sh <baseline-ref>
set -euo pipefail

BASE="$1"
ROOT="$(git rev-parse --show-toplevel)"
cd "${ROOT}"
SURVEY="$(dirname "$(python3 -B "$(git ls-files -- ':(glob)**/repository_files.py' ':(exclude,glob)**/inputs/**')" \
    file 3be770d5/survey/summarize-profile.py)")"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "${SCRATCH}"' EXIT

# The summarizer takes its repository root from its own location, reads the
# build identity and structural costs below it, and records the series path
# relative to it; each scratch repository gives both runs the same layout.
stage() {
    local name="$1" source="$2"
    git init -q "${SCRATCH}/${name}"
    git ls-files | cut -d/ -f1 | sort -u | while IFS= read -r entry; do
        ln -s "${ROOT}/${entry}" "${SCRATCH}/${name}/${entry}"
    done
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
    failed=0
    for name in old new; do
        rm -rf "${SCRATCH}/${name}/series"
        cp -r "${series}" "${SCRATCH}/${name}/series"
        if ! python3 -B "${SCRATCH}/${name}/survey/summarize-profile.py" "${SCRATCH}/${name}/series" \
            --json "${SCRATCH}/${name}/series/profile-summary.json" \
            --markdown "${SCRATCH}/${name}/series/profile.md" > /dev/null 2> "${SCRATCH}/${name}.err"; then
            cat "${SCRATCH}/${name}.err" >&2
            failed=1
        fi
    done
    verdict=identical
    [ "${failed}" = 0 ] || verdict=failed
    while IFS= read -r file; do
        cmp -s "${SCRATCH}/old/series/${file}" "${SCRATCH}/new/series/${file}" || verdict=different
    done < <(cd "${SCRATCH}/old/series" && find . -type f | sort)
    [ "$(cd "${SCRATCH}/old/series" && find . -type f | sort)" = \
      "$(cd "${SCRATCH}/new/series" && find . -type f | sort)" ] || verdict=different
    [ "${verdict}" = identical ] || status=1
    printf '%s\t%s\n' "${series}" "${verdict}"
done < <(git ls-files '*/attribution-chains.json' | sort)
exit "${status}"
