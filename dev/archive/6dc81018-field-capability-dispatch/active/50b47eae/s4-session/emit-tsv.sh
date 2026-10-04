#!/usr/bin/env bash
# emit-tsv.sh <ledger.tsv> <out.tsv> : the 8-column committed member ledger
set -euo pipefail
awk -F'\t' 'BEGIN{OFS="\t"} NR==1{print "member_j","E","G","rustflags","bytes","sha256","text_vaddr","text_page_offset"; next} {print $1,$2,$3,$4,$5,$6,$7,$8}' "$1" > "$2"
