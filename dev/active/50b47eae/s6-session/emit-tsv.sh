#!/usr/bin/env bash
# emit-tsv.sh <ledger.tsv> <out.tsv> : the 7-column committed member ledger (v4, E-only)
set -euo pipefail
awk -F'\t' 'BEGIN{OFS="\t"} NR==1{print "member_j","E","rustflags","bytes","sha256","text_vaddr","text_page_offset"; next} {print $1,$2,$3,$4,$5,$6,$7}' "$1" > "$2"
