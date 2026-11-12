#!/usr/bin/env bash
set -euo pipefail

# Times gf2's GF(2^8) region and matrix-product routes, the byte-region API
# and the generic API over one-byte Gf256 elements, against ISA-L and M4RIE on
# identical inputs, and prints one row per cell with the host name.
#
# Usage: GF256_BASELINE_PREFIX=<install prefix> experiments/gf256-baselines/run.sh
#
# The prefix holds include/ and lib/ of ISA-L (libisal) and M4RIE (libm4rie,
# libm4ri). GF256_BASELINE_CORE (default 8) is the core both arms run pinned
# to. The script builds both arms, then measures: inside a benchmark window
# (GF2_BENCH_WINDOW=1) it re-runs itself under ccx1-bench-flock.sh,
# which pins to cores 6-11, and labels the table a window measurement; outside
# one it measures unlocked and labels the table an indication. Beside the host
# line it prints the SHA-256 of both measured executables and of the ISA-L
# and M4RIE libraries the reference arm loads. It exits 1 when
# the arms' outputs differ on a cell.

: "${GF256_BASELINE_PREFIX:?name the ISA-L and M4RIE install prefix}"
export GF256_BASELINE_PREFIX
core=${GF256_BASELINE_CORE:-8}
here=$(cd "$(dirname "$0")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel)
flock="$root/$(python3 "$root/$(git -C "$root" ls-files -- ':(glob)**/repository_files.py')" file ccx1-bench-flock.sh)"
out="$root/target/gf256-baselines"
mode=indication
[[ "${GF2_BENCH_WINDOW:-}" == 1 ]] && mode=window

if [[ "${1:-}" != --measure ]]; then
  mkdir -p "$out"
  (cd "$root" && ./scripts/cargo-budget.sh cargo build --locked --release -p gf256-baselines --quiet)
  cc -O2 -Wall -I"$GF256_BASELINE_PREFIX/include" -o "$out/arm" "$here/ext/arm.c" \
    -L"$GF256_BASELINE_PREFIX/lib" -lisal -lm4rie -lm4ri -Wl,-rpath,"$GF256_BASELINE_PREFIX/lib"
  if [[ $mode == window ]]; then
    cd "$root"
    CARGO_CI_NO_LOCK=1 exec "$flock" bash "$here/run.sh" --measure
  fi
fi

echo "host $(hostname), $(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2- | sed 's/^ //'), core $core, $mode"
lib="$GF256_BASELINE_PREFIX/lib"
"$root/target/release/gf256-baselines" --digests "$out/arm" "$lib/libisal.so" "$lib/libm4rie.so" "$lib/libm4ri.so"

taskset -c "$core" "$root/target/release/gf256-baselines" >"$out/gf2.tsv"
taskset -c "$core" "$out/arm" >"$out/ext.tsv"

python3 -B - "$out/gf2.tsv" "$out/ext.tsv" <<'PY'
import re, sys

rows = {}
for path in sys.argv[1:]:
    for line in open(path):
        op, size, arm, metric, digest = line.rstrip("\n").split("\t")
        rows.setdefault((op, int(size)), []).append((arm, metric, digest))

def rate(metric):
    # GB/s for regions, Gmul-add/s for products: larger is faster.
    return float(re.findall(r"([\d.]+) G", metric)[-1])

failed = False
print(f"{'cell':<14} {'byte API':<28} {'generic Gf256':<28} {'gen/byte':<9} "
      f"{'fastest reference':<58} byte/ref")
for (op, size), arms in sorted(rows.items()):
    digests = {digest for _, _, digest in arms}
    if len(digests) != 1:
        failed = True
    byte = next(a for a in arms if "Gf2mField" in a[0])
    generic = next(a for a in arms if "Gf256" in a[0] and "Gf2mField" not in a[0])
    ref = max((a for a in arms if not a[0].startswith("gf2")), key=lambda a: rate(a[1]))
    status = "" if len(digests) == 1 else "  OUTPUTS DIFFER"
    print(f"{op + ' ' + str(size):<14} {byte[1]:<28} {generic[1]:<28} "
          f"{rate(generic[1]) / rate(byte[1]):<9.2f} {ref[0] + ': ' + ref[1]:<58} "
          f"{rate(byte[1]) / rate(ref[1]):.2f}{status}")
sys.exit(1 if failed else 0)
PY
