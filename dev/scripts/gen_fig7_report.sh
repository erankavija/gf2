#!/usr/bin/env bash
# Generate Fig 7 comparison report: GLDPC vs LDPC NMS vs paper reference
set -euo pipefail

OUT="dev/simulation_results/fig7_comparison_report.txt"
REF="dev/reference_data/fig_gldpc_sogrand.csv"
GLDPC="dev/simulation_results/fig7_gldpc.csv"
LDPC="dev/simulation_results/fig7_ldpc_nms.csv"

echo "Phase 3 — Figure 7: (1024, 646) QC-GLDPC vs 5G NR LDPC" > "$OUT"
echo "==========================================================" >> "$OUT"
echo "" >> "$OUT"
echo "Generated: $(date -Iseconds)" >> "$OUT"
echo "" >> "$OUT"

echo "GLDPC vs Paper Reference (eBCH_GLDPC):" >> "$OUT"
echo "----------------------------------------" >> "$OUT"
python3 dev/reference_data/scripts/compare_results.py \
    --ref "$REF" --sim "$GLDPC" --decoder GLDPC >> "$OUT" 2>&1
echo "" >> "$OUT"

echo "GLDPC vs Paper LDPC (NMS baseline):" >> "$OUT"
echo "------------------------------------" >> "$OUT"
python3 dev/reference_data/scripts/compare_results.py \
    --ref "$REF" --sim "$LDPC" --decoder LDPC >> "$OUT" 2>&1
echo "" >> "$OUT"

echo "Raw GLDPC results:" >> "$OUT"
echo "-------------------" >> "$OUT"
cat "$GLDPC" >> "$OUT"
echo "" >> "$OUT"

echo "Raw LDPC NMS results:" >> "$OUT"
echo "----------------------" >> "$OUT"
cat "$LDPC" >> "$OUT"

echo "Report written to $OUT"
