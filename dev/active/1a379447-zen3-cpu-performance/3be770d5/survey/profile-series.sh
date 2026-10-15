# shellcheck shell=bash
# Session loop of a repeated profile series, sourced by its launchers, which
# define `series_cases` (prints the expanded cases) and `series_identity
# <expanded-cases>` (refuses a differing executable, prints the identity).
# `repetitions.log` is the append-only execution log; a re-run resumes after
# the last completed session, re-runs an unfinished one whole, and refuses any
# session whose identity differs from the first session's.

series_now() { date -u +%Y-%m-%dT%H:%M:%SZ; }
series_load() { cut -d' ' -f1-3 /proc/loadavg; }
series_digest() { sha256sum "$1" | cut -d' ' -f1; }
# Usage (from the worktree root): series_located <name-or-path-suffix>
series_located() {
    python3 -B "$(git ls-files -- ':(glob)**/repository_files.py' ':(exclude,glob)**/inputs/**')" \
        file "$1"
}
# Usage: series_quality <build-record>
series_quality() {
    python3 -c 'import json, pathlib, sys
(directory,) = {str(pathlib.Path(path).parent) for path in json.load(open(sys.argv[1]))["prepared_quality"]}
print(directory)' "$1"
}
# Usage: series_recorded <json-record> <key>...
series_recorded() {
    python3 -c 'import functools, json, sys
print(functools.reduce(lambda node, key: node[key], sys.argv[2:], json.load(open(sys.argv[1]))))' "$@"
}

# Usage (from the worktree root):
#   profile_series <output-dir> <bin-dir> <session-script> <repetitions> <host-title>
profile_series() {
    local out="$1" bin="$2" session="$3" repetitions="$4" title="$5"
    local wrapper log identity first current index rep starts statuses
    mkdir -p "${out}"
    out="$(cd "${out}" && pwd)"
    log="${out}/repetitions.log"
    touch "${log}"
    echo "profile execution log: ${log}" >&2
    wrapper="$(series_located ccx1-bench-flock.sh)"

    # Case names resolve to arguments, environments and prepared quality
    # outside the mutex.
    series_cases >"${out}/cases.expanded.tsv"
    identity="$(series_identity "${out}/cases.expanded.tsv")" || return 2
    first="$(sed -n '/^rep-[0-9]* start [^ ]* .* load=/{s/^rep-[0-9]* start [^ ]* \(.*\) load=.*/\1/p;q;}' "${log}")"
    if [[ -n "${first}" && "${first}" != "${identity}" ]]; then
        echo "the series identity changed since the first session" >&2
        echo "first:   ${first}" >&2
        echo "current: ${identity}" >&2
        return 2
    fi

    if [[ ! -e "${out}/host.txt" ]]; then
    {
        echo "# ${title}"
        echo "# generated_utc: $(series_now)"
        echo "# gf2 revision (informational): $(git rev-parse HEAD)"
        echo "# identity: ${identity}"
        echo
        echo "## uname"; uname -a
        echo; echo "## perf"; perf --version
        echo; echo "## perf_event_paranoid"; cat /proc/sys/kernel/perf_event_paranoid
        echo; echo "## lscpu"; lscpu
        echo; echo "## smt"; cat /sys/devices/system/cpu/smt/control 2>/dev/null || echo "no smt control"
        echo; echo "## thread siblings"
        for cpu in /sys/devices/system/cpu/cpu[0-9]*; do
            printf '%s %s l3=%s\n' "$(basename "${cpu}")" "$(cat "${cpu}/topology/thread_siblings_list")" \
                "$(cat "${cpu}/cache/index3/shared_cpu_list" 2>/dev/null || echo none)"
        done
        echo; echo "## governors"
        for policy in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do
            printf '%s %s\n' "${policy}" "$(cat "${policy}")"
        done 2>/dev/null || echo "no cpufreq sysfs entries"
        echo; echo "## load average at start"; uptime
    } >"${out}/host.txt"
    else
        { echo; echo "## resumed $(series_now)"; uptime; } >>"${out}/host.txt"
    fi

    for index in $(seq 1 "${repetitions}"); do
        rep="rep-$(printf '%02d' "${index}")"
        if grep -q "^${rep} done " "${log}"; then
            continue
        fi
        starts="$(awk -v rep="${rep}" '$1 == rep && $2 == "discarded" {list = ""}
            $1 == rep && $2 == "start" {list = list (list == "" ? "" : " ") $3}
            END {print list}' "${log}")"
        if [[ -n "${starts}" ]]; then
            echo "${rep} discarded $(series_now) unfinished start(s) ${starts}; partial output removed, the session re-runs whole" >>"${log}"
        fi
        rm -rf "${out:?}/${rep}"
        current="$(series_identity "${out}/cases.expanded.tsv")" || return 2
        if [[ "${current}" != "${identity}" ]]; then
            echo "the series identity changed during the series; ${rep} does not start" >&2
            return 2
        fi
        echo "${rep} start $(series_now) ${identity} load=[$(series_load)]" >>"${log}"
        GF2_BENCH=1 CARGO_CI_NO_LOCK=1 "./${wrapper}" --full-host \
            bash "${session}" "${out}/${rep}" "${bin}" "${out}/cases.expanded.tsv" \
            "$([[ "${index}" == 1 ]] && echo yes || echo no)"
        # A session whose every case failed carries no figure, and the next
        # session would repeat the failure.
        statuses="$(cat "${out}/${rep}"/stat/*.status "${out}/${rep}"/record/*.status 2>/dev/null || true)"
        if ! grep -q '^0$' <<<"${statuses}"; then
            echo "${rep} discarded $(series_now) every profiled case failed" >>"${log}"
            echo "every profiled case in ${rep} failed; see ${out}/${rep}/*/*.err" >&2
            return 1
        fi
        echo "${rep} done $(series_now) load=[$(series_load)]" >>"${log}"
    done
    grep -q '^series done ' "${log}" || echo "series done $(series_now) sessions=${repetitions}" >>"${log}"
    { echo; echo "## load average at end"; uptime; echo "# finished_utc: $(series_now)"; } >>"${out}/host.txt"
}
