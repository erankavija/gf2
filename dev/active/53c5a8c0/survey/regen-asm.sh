#!/usr/bin/env bash
# Regenerate the release-assembly artefacts of the measured gf2 paths
# (jit:53c5a8c0).
#
# Usage (from the worktree root):
#   dev/active/53c5a8c0/survey/regen-asm.sh
#
# The artefacts are dumped from the arm executables the receipts pinned, in the
# two targeting variants `build-arms.sh` produces, because the measured gf2
# paths are generic: a monomorphisation no library item instantiates is emitted
# in the consumer, so `cargo asm` over `gf2-core` cannot see it. The study
# changes no kernel, so the artefacts live beside the survey rather than beside
# a kernel source.
#
# Symbol sets:
#
# * long-product.asm.txt - the polynomial family's gf2 side, from the
#   host-targeted executable that family measured: the long product at the two
#   widths a wide kernel covers (four and nine words) and at the three widths
#   above them, plus the two kernels themselves.
# * crossover.asm.txt - the crossover family's gf2 side, from the portable
#   executable that family measured: the raw-batch kernel and the single
#   carry-less multiply the two raw-batch arms select, the reduced element-wise
#   batch kernel, the field reduction the dot product ends in, and the
#   dispatched dot-product consumer itself.
#
# Nothing here times anything, so it takes no host mutex.
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"
survey=dev/active/53c5a8c0/survey
out="$survey/asm"
native=target/53c5a8c0-arms-native/release
conservative=target/53c5a8c0-arms-conservative/release

for binary in "$native/poly-arm" "$conservative/crossover-arm"; do
  [[ -x "$binary" ]] || { echo "$binary is absent; run $survey/build-arms.sh" >&2; exit 2; }
done

mkdir -p "$out"

python3 "$survey/dump-asm.py" --binary "$native/poly-arm" \
  --out "$out/long-product.asm.txt" \
  --long-product-width 4 \
  --long-product-width 9 \
  --long-product-width 16 \
  --long-product-width 64 \
  --long-product-width 256 \
  --symbol 'gf2_kernels_simd::gf2m_wide::clmul_wide4_ymm_safe' \
  --symbol 'gf2_kernels_simd::gf2m_wide::clmul_wide9_ymm_safe'

python3 "$survey/dump-asm.py" --binary "$conservative/crossover-arm" \
  --out "$out/crossover.asm.txt" \
  --symbol 'gf2_kernels_simd::x86::clmul::clmul_batch' \
  --symbol 'gf2_kernels_simd::x86::clmul::clmul_u64' \
  --symbol 'gf2_kernels_simd::x86::gf2m_batch::gf2m_batch_mul_ymm_unroll4' \
  --symbol 'gf2_kernels_simd::x86::clmul::clmul_barrett_reduce' \
  --symbol 'gf2_core::gf2m::batch::batch_mul_raw' \
  --symbol 'gf2_core::field::vec::FieldVec<gf2_core::gf2m::field::Gf2mElement_>::simd_dot_product'

python3 "$survey/annotate-asm.py" "$out/long-product.asm.txt" "$out/crossover.asm.txt"
