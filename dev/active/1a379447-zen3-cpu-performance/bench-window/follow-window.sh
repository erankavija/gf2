#!/usr/bin/env bash
# Live view of the benchmark window for epic 1a379447 (jit:1a379447).
#
# Reads the same three things run-window.sh writes and nothing else: the
# append-only window.log, the per-job <key>.out, and the <key>.done markers.
# It derives each job's state from those rather than from a process check, so
# the view is the same whether the window is running, finished or interrupted.
#
# A job's exit code is not evidence that it measured anything: a launcher that
# logs each case's status and continues exits 0 over a wholly failed series.
# The last panel therefore tails the running job's own output, which is where
# that shows up, and a finished job is shown with the rc the log recorded.
#
# The layout adapts to the pane, so it stays readable in a narrow split.
#
# Usage: dev/active/1a379447-zen3-cpu-performance/bench-window/follow-window.sh [interval-seconds]
set -uo pipefail

repo=/home/vkaskivuo/Projects/gf2
here="$repo/dev/active/1a379447-zen3-cpu-performance/bench-window"
state="${GF2_WINDOW_STATE:-$repo/.agents/bench-window}"
log="$state/window.log"
queue="$here/queue.tsv"
unit="${GF2_WINDOW_UNIT:-gf2-bench-window-20260912b}"
interval="${1:-10}"
ndone_prev=0
nfail_prev=0

export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
export DBUS_SESSION_BUS_ADDRESS="${DBUS_SESSION_BUS_ADDRESS:-unix:path=$XDG_RUNTIME_DIR/bus}"

key_of() { printf '%s\t%s\t%s' "$1" "$2" "$3" | sha256sum | cut -c1-16; }

while :; do
    cols=$(tput cols 2>/dev/null || echo 80)
    rows=$(tput lines 2>/dev/null || echo 24)
    # Rows the fixed chrome costs: title, unit, load, blank, header, blank,
    # log heading, blank, footer.
    spare=$(( rows - 9 - $(grep -cvE '^\s*(#|$)' "$queue") ))
    (( spare < 2 )) && spare=2
    logn=$(( spare > 8 ? 4 : 2 ))
    outn=$(( spare - logn ))
    (( outn < 1 )) && outn=1

    njobs=$(grep -cvE '^\s*(#|$)' "$queue")
    # A short pane cannot hold a row per job, so collapse the table to one
    # progress line and spend the space on the running job's own output.
    compact=0
    (( rows < njobs + 12 )) && compact=1

    clear
    if (( compact )); then
        printf '1a379447 %s %s done %d/%d%s la %s\n' \
            "$(systemctl --user is-active "$unit" 2>/dev/null || echo '?')" \
            "$(date -u +%H:%MZ)" "$ndone_prev" "$njobs" \
            "$( (( nfail_prev )) && printf ' FAIL:%d' "$nfail_prev" )" \
            "$(cut -d' ' -f1 /proc/loadavg)"
    else
        printf 'gf2 benchmark window — 1a379447\n'
        printf '%s  %s  load%s  free %s\n' \
            "$(systemctl --user is-active "$unit" 2>/dev/null || echo '?')" \
            "$(date -u +%H:%M:%SZ)" \
            "$(cut -d' ' -f1-3 /proc/loadavg | sed 's/^/ /')" \
            "$(df -h "$repo" | awk 'NR==2{print $4}')"
    fi

    from=$(grep -n 'window start' "$log" 2>/dev/null | tail -1 | cut -d: -f1)
    [[ -n "$from" ]] && tailed=$(tail -n +"$from" "$log") || tailed=""

    running_key=""; ndone=0; nfail=0; runline=""
    namew=$(( cols - 26 )); (( namew < 12 )) && namew=12
    while IFS=$'\t' read -r issue wt mins cmd; do
        [[ -z "${issue// /}" || "$issue" == \#* ]] && continue
        key=$(key_of "$issue" "$wt" "$cmd")
        short="${cmd##*/}"; short="${short%% >*}"
        if [[ -e "$state/$key.done" ]]; then
            rc=$(grep "job exit .*key=$key " <<< "$tailed" | tail -1 | sed 's/.*rc=//')
            st="done:${rc:-?}"; ndone=$(( ndone + 1 ))
            [[ "${rc:-0}" != 0 ]] && nfail=$(( nfail + 1 ))
        elif grep -q "job start .*key=$key " <<< "$tailed"; then
            started=$(grep "job start .*key=$key " <<< "$tailed" | tail -1 | cut -d' ' -f1)
            begun=$(date -u -d "$started" +%s 2>/dev/null || echo 0)
            (( begun > 0 )) && el=$(( ($(date -u +%s) - begun) / 60 )) || el=0
            st="RUN ${el}/${mins}m"
            running_key="$key"; runline="$issue ${el}/${mins}m ${short:0:$namew}"
        else
            st="pending"
        fi
        (( compact )) || printf '%-8s %-10s %s\n' "$issue" "$st" "${short:0:$namew}"
    done < "$queue"

    ndone_prev=$ndone; nfail_prev=$nfail
    if (( compact )); then
        # Budget: the running-job line, then the job's own output filling the
        # rest, then one footer line. Nothing else fits and nothing else is
        # worth the row.
        printf '%s\n' "${runline:-(no job running)}" | cut -c1-"$cols"
        outn=$(( rows - 3 )); (( outn < 1 )) && outn=1
        if [[ -n "$running_key" && -s "$state/$running_key.out" ]]; then
            tail -"$outn" "$state/$running_key.out" | cut -c1-"$cols"
        else
            tail -"$outn" "$log" 2>/dev/null | cut -c1-"$cols"
        fi
        printf 'Ctrl-C stops watching, not the window\n'
    else
        printf -- '-- log --\n'
        tail -"$logn" "$log" 2>/dev/null | cut -c1-"$cols"
        if [[ -n "$running_key" && -s "$state/$running_key.out" ]]; then
            printf -- '-- job --\n'
            tail -"$outn" "$state/$running_key.out" | cut -c1-"$cols"
        fi
        printf '\nCtrl-C stops watching, not the window\n'
    fi
    sleep "$interval"
done
