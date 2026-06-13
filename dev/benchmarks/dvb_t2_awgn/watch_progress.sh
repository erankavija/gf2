#!/usr/bin/env bash
# Snapshot of the DVB-T2 AWGN production campaign progress.
# Usage: bash dev/benchmarks/dvb_t2_awgn/watch_progress.sh
#   add `watch -n 10 bash .../watch_progress.sh` for a live view.
set -uo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "### driver tail"
tail -n 3 "$DIR/campaign_driver.log" 2>/dev/null
echo "### extend driver tail"
tail -n 3 "$DIR/extend_driver.log" 2>/dev/null
echo

for sub in curve_1_2_16qam curve_2_3_16qam curve_3_4_16qam \
           curve_1_2_64qam curve_2_3_64qam curve_3_4_64qam \
           curve_2_3_16qam_ext curve_3_4_16qam_ext \
           curve_1_2_64qam_ext curve_2_3_64qam_ext curve_3_4_64qam_ext; do
  cp="$DIR/$sub/checkpoints"
  [ -d "$cp" ] || continue
  echo "### $sub"
  for f in $(ls "$cp"/snr_*.json 2>/dev/null | sort); do
    python3 - "$f" <<'PY'
import json,sys
d=json.load(open(sys.argv[1]))
fr=d['frames_completed']; er=d['errors_accumulated']
fer=er/max(fr,1)
print(f"  {d['esn0_db']:5.2f} dB  frames={fr:>8}  errors={er:>6}  fer={fer:.3e}")
PY
  done
  echo
done
