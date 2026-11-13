#!/usr/bin/env bash
# The indication runs of @/issue/d5cd53e0 on DVB-T2 normal rate 1/2
# (@/citation/Etsi2015): gf2's flooding and layered decoders through
# dvb_t2_fer_diag, and AFF3CT 4.7.0 (@/citation/Cassagne2019) flooding NMS.
#
# Usage: dvb-t2-r12-diagnosis.sh STEP...   (no STEP lists the steps)
#
# Environment: GF2_AFF3CT_ROOT names the staged AFF3CT tree whose
# build/bin/aff3ct-4.7.0 the AFF3CT steps run; DIAGNOSIS_CPUS the CPU list
# every run is pinned to (default 8-11), whose size is the worker count.
#
# Each step appends to target/aff3ct-ldpc-gaps/dvb-t2-r12-diagnosis/STEP.log:
# a header with UTC time, host, CPU list, 1-minute load average, Git HEAD and
# the SHA-256 of every executable it runs, then each command line before its
# output, and the closing load average. Every seed, frame count and decoder
# of the recorded tables is a literal below.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo=$(git -C "$here" rev-parse --show-toplevel)
cd "$repo"
cpus="${DIAGNOSIS_CPUS:-8-11}"
workers=$(taskset -c "$cpus" nproc)
out="$repo/target/aff3ct-ldpc-gaps/dvb-t2-r12-diagnosis"
target=$(cargo metadata --format-version 1 --no-deps --offline |
    python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')
diag="$target/release/dvb_t2_fer_diag"
export_alist="$target/release/export_alist"

steps=(req01-gf2 req01-aff3ct control-gf2 control-aff3ct explore-aff3ct
    flooding-failures layered-scan layered-factorial layered-range)
if [[ $# -eq 0 ]]; then
    printf '%s\n' "${steps[@]}"
    exit 0
fi

aff3ct() {
    : "${GF2_AFF3CT_ROOT:?GF2_AFF3CT_ROOT must name the staged AFF3CT tree}"
    echo "$GF2_AFF3CT_ROOT/build/bin/aff3ct-4.7.0"
}

load() { cut -d' ' -f1 /proc/loadavg; }

header() {
    echo "STEP $1 utc=$(date -u +%Y-%m-%dT%H:%M:%SZ) host=$(hostname) cpus=$cpus" \
        "workers=$workers load1=$(load) head=$(git rev-parse HEAD)"
    shift
    for exe in "$@"; do echo "EXECUTABLE $(sha256sum "$exe")"; done
}

run() {
    echo "COMMAND taskset -c $cpus $*"
    taskset -c "$cpus" "$@"
}

gf2() {
    local ebn0=$1 frames=$2 seed=$3
    shift 3
    run "$diag" --ebn0 "$ebn0" --frames "$frames" --seed "$seed" --workers "$workers" "$@"
}

# AFF3CT flooding NMS: ENC is LDPC_DVBS2 (H from AFF3CT's own tables, random
# codewords) or AZCW (H from the exported AList, the all-zero codeword).
aff() {
    local enc=$1 ebn0=$2 frames=$3 seed=$4
    shift 4
    local code=(--enc-type LDPC_DVBS2)
    [[ $enc == AZCW ]] && code=(--enc-type AZCW --dec-h-path "$out/dvb_t2_r12.alist")
    local command=("$(aff3ct)" -C LDPC "${code[@]}" -N 64800 -K 32400 -E EBN0
        -m "$ebn0" -M "$(python3 -c "print($ebn0 + 0.001)")" -s 1 -n "$frames"
        -e 100000000 -t "$workers" --sim-seed "$seed" --ter-freq 0 "$@")
    echo "COMMAND taskset -c $cpus ${command[*]}"
    # The result row is the last row of the progress display.
    taskset -c "$cpus" "${command[@]}" 2>&1 | tr '\r' '\n' |
        grep -E '^\(EE\)|\|\|' | grep -v '^#' | tail -n 1
}

step() {
    case $1 in
    req01-gf2)
        header "$1" "$diag"
        gf2 1.2 5000 12 --decoder flooding:0.75:50
        gf2 1.5 20000 15 --decoder flooding:0.75:50
        gf2 2.0 20000 20 --decoder flooding:0.75:50
        ;;
    req01-aff3ct)
        header "$1" "$(aff3ct)"
        local nms=(-D BP_FLOODING --dec-implem NMS --dec-norm 0.75 -i 50)
        aff LDPC_DVBS2 1.2 5000 12 "${nms[@]}"
        aff LDPC_DVBS2 1.5 20000 15 "${nms[@]}"
        aff LDPC_DVBS2 2.0 20000 20 "${nms[@]}"
        ;;
    control-gf2)
        header "$1" "$diag"
        gf2 1.0 5000 10 --decoder flooding:0.875:50
        gf2 1.2 5000 12 --decoder flooding:0.875:50
        gf2 1.5 5000 15 --decoder flooding:0.875:50
        ;;
    control-aff3ct)
        header "$1" "$(aff3ct)"
        local nms=(-D BP_FLOODING --dec-implem NMS --dec-norm 0.875 -i 50)
        aff LDPC_DVBS2 1.0 5000 10 "${nms[@]}"
        aff LDPC_DVBS2 1.2 5000 12 "${nms[@]}"
        aff LDPC_DVBS2 1.5 5000 15 "${nms[@]}"
        ;;
    explore-aff3ct)
        header "$1" "$(aff3ct)" "$export_alist"
        run "$export_alist" --code dvb-t2-r12 --output "$out/dvb_t2_r12.alist"
        echo "INPUT $(sha256sum "$out/dvb_t2_r12.alist")"
        aff AZCW 1.5 400 1 -D BP_FLOODING --dec-implem NMS --dec-norm 0.75 -i 50
        aff LDPC_DVBS2 1.5 400 1 -D BP_FLOODING --dec-implem NMS --dec-norm 0.75 -i 50
        aff AZCW 1.5 400 1 -D BP_FLOODING --dec-implem NMS --dec-norm 0.75 -i 200
        aff AZCW 1.5 400 1 -D BP_FLOODING --dec-implem NMS --dec-norm 0.875 -i 50
        aff AZCW 1.5 400 1 -D BP_FLOODING --dec-implem SPA -i 50
        aff AZCW 1.5 400 1 -D BP_FLOODING --dec-implem MS -i 50
        aff AZCW 1.5 400 1 -D BP_HORIZONTAL_LAYERED --dec-implem NMS --dec-norm 0.75 -i 50
        aff AZCW 1.5 400 1 -D BP_HORIZONTAL_LAYERED --dec-implem NMS --dec-norm 0.75 -i 25
        ;;
    flooding-failures)
        header "$1" "$diag"
        gf2 2.0 1280 20 --decoder layered:25 --failures flooding:0.75:50
        ;;
    layered-scan)
        header "$1" "$diag"
        gf2 1.3 512000 2013 --decoder layered:25 --failures layered:25
        gf2 1.5 512000 2015 --decoder layered:25 --failures layered:25
        gf2 1.7 512000 2017 --decoder layered:25 --failures layered:25
        gf2 2.0 512000 2020 --decoder layered:25 --failures layered:25
        ;;
    layered-factorial)
        header "$1" "$diag"
        local decoders=(--decoder layered:25 --decoder layered:25:7:2:31:40
            --decoder layered:25:7:4:62:40 --decoder layered:25:7:2:15:40
            --decoder layered:25:7:4:31:20)
        gf2 1.5 256000 3015 "${decoders[@]}"
        gf2 2.0 256000 3020 "${decoders[@]}"
        ;;
    layered-range)
        # In natural-log units at scale s and message limit M: quantum 1/s,
        # message bound M/s, variable-to-check bound (127 - M)/s, belief bound
        # 127/s.
        header "$1" "$diag"
        local decoders=(--decoder layered:25 --decoder layered:25:7:3:31:40
            --decoder layered:25:7:2:31:40 --decoder layered:25:7:2:31:20
            --decoder layered:25:7:2:31:83 --decoder layered:25:7:4:31:30
            --decoder layered:25:7:4:31:50 --decoder layered:25:7:1:31:20)
        gf2 1.3 256000 4013 "${decoders[@]}"
        gf2 1.5 256000 4015 "${decoders[@]}"
        ;;
    *)
        echo "unknown step $1" >&2
        exit 2
        ;;
    esac
    echo "END load1=$(load) utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}

CARGO_NET_OFFLINE=true cargo build --locked --release -p aff3ct-ldpc-gaps \
    --bin dvb_t2_fer_diag --bin export_alist >&2
mkdir -p "$out"
for name in "$@"; do
    step "$name" 2>&1 | tee -a "$out/$name.log"
done
