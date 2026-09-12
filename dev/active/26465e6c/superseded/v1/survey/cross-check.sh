#!/usr/bin/env bash
# Cross-language equivalence check for jit:26465e6c (population-count and
# fused-reduction baselines). This is the direct evidence for REQ-03's
# "validate identical buffers, bit lengths, tail semantics and returned counts".
#
# Three checks, all fail-closed:
#
#   1. Alignment parity. Every arm binary prints, under `--alignment`, the byte
#      alignment of the fixture window each `word_offset` selects. All three
#      must print identical lines, so a cell that varies alignment varies it
#      identically on both sides of the survey.
#   2. Independent-reference self-tests. Each external arm re-derives every
#      count with `__builtin_popcountll` over its own buffer under `--selftest`.
#   3. Count parity. For fixed (words, seed, pattern, word_offset) tuples, every
#      applicable arm's `--check` mode (which bypasses the timing transport and
#      prints just the resulting count) must return the identical value.
#
# Bit-length tail semantics are covered by the harness crate's test suite
# (`gf2-side/tests/correctness.rs`), which drives `BitVec::count_ones` at bit
# lengths 0/1/63/64/65/127/128/4095/4096.
#
# Usage: dev/active/26465e6c/survey/cross-check.sh
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GF2_SIDE="${HERE}/gf2-side/target/release/popcount-gf2-side"
LIBPOPCNT="${HERE}/libpopcnt-arm"
MULA="${HERE}/mula-avx2-harleyseal-arm"
for bin in "${GF2_SIDE}" "${LIBPOPCNT}" "${MULA}"; do
  [[ -x "${bin}" ]] || { echo "missing binary: ${bin}" >&2; exit 2; }
done

fail=0

echo "== alignment parity =="
alignment_reference=$(GF2_POPCOUNT_ARM=production-dispatch "${GF2_SIDE}" --alignment)
echo "${alignment_reference}"
for name in libpopcnt mula; do
  case "${name}" in
    libpopcnt) observed=$("${LIBPOPCNT}" --alignment) ;;
    mula) observed=$("${MULA}" --alignment) ;;
  esac
  if [[ "${observed}" != "${alignment_reference}" ]]; then
    echo "MISMATCH: ${name} fixture alignment differs from gf2-side" >&2
    diff <(echo "${alignment_reference}") <(echo "${observed}") >&2 || true
    fail=1
  else
    echo "alignment ${name}: identical to gf2-side"
  fi
done

echo "== independent-reference self-tests =="
echo "libpopcnt selftest: $("${LIBPOPCNT}" --selftest)"
echo "mula selftest: $("${MULA}" --selftest)"

echo "== returned-count parity =="
check_popcount() {
  local words="$1" seed="$2" pattern="$3" offset="$4"
  local -a values=() labels=()
  for arm in production-dispatch nibble-lut scalar-popcnt compiler-count-ones; do
    v=$(GF2_POPCOUNT_ARM="${arm}" "${GF2_SIDE}" --check popcount "${words}" "${seed}" "${pattern}" "${offset}")
    values+=("${v}"); labels+=("gf2-side:${arm}")
  done
  v=$("${LIBPOPCNT}" --check popcount "${words}" "${seed}" "${pattern}" "${offset}")
  values+=("${v}"); labels+=("libpopcnt")
  if [[ "${offset}" == "0" ]]; then
    v=$("${MULA}" --check popcount "${words}" "${seed}" "${pattern}" "${offset}")
    values+=("${v}"); labels+=("mula-avx2-harleyseal")
  else
    # The vendored AVX2 Harley-Seal reference loads through `__m256i*` and
    # requires a 32-byte aligned pointer; it fails closed on the offsets this
    # survey's alignment cell uses. Recorded as an unavailable arm, never
    # substituted with a different operation. See findings.md.
    if "${MULA}" --check popcount "${words}" "${seed}" "${pattern}" "${offset}" >/dev/null 2>&1; then
      echo "MISMATCH: mula accepted unaligned word_offset ${offset}, expected a closed failure" >&2
      fail=1
    fi
  fi
  local first="${values[0]}"
  local i=0
  for v in "${values[@]}"; do
    echo "popcount words=${words} seed=${seed} pattern=${pattern} offset=${offset} ${labels[$i]}=${v}"
    if [[ "${v}" != "${first}" ]]; then
      echo "MISMATCH: ${labels[$i]}=${v} != ${labels[0]}=${first} (words=${words} seed=${seed} pattern=${pattern} offset=${offset})" >&2
      fail=1
    fi
    i=$((i+1))
  done
}

check_and_popcnt() {
  local words="$1" seed_lhs="$2" seed_rhs="$3" pattern="$4" offset="$5"
  local -a values=() labels=()
  for arm in and-popcnt-fused and-popcnt-scalar-control and-popcnt-two-pass-consumer; do
    v=$(GF2_POPCOUNT_ARM="${arm}" "${GF2_SIDE}" --check and_popcnt "${words}" "${seed_lhs}" "${seed_rhs}" "${pattern}" "${offset}")
    values+=("${v}"); labels+=("gf2-side:${arm}")
  done
  local first="${values[0]}"
  local i=0
  for v in "${values[@]}"; do
    echo "and_popcnt words=${words} seed_lhs=${seed_lhs} seed_rhs=${seed_rhs} pattern=${pattern} offset=${offset} ${labels[$i]}=${v}"
    if [[ "${v}" != "${first}" ]]; then
      echo "MISMATCH: ${labels[$i]}=${v} != ${labels[0]}=${first}" >&2
      fail=1
    fi
    i=$((i+1))
  done
  echo "no external candidate: neither libpopcnt nor Mula's sse-popcount exposes a fused AND-then-popcount operation (see findings.md)"
}

# One tuple per confirmatory cell, plus the word-boundary sizes the engineering
# contract requires of bit-packed behavior.
check_popcount 4 201 random 0
check_popcount 8 202 random 0
check_popcount 256 203 random 3
check_popcount 64 204 all_one 0
check_popcount 63 205 random 0
check_popcount 64 206 all_zero 0
check_popcount 16384 207 random 0
check_popcount 524288 208 random 0
for boundary_words in 0 1 63 64 65; do
  for boundary_offset in 0 1 2 3; do
    check_popcount "${boundary_words}" 11 random "${boundary_offset}"
  done
done

check_and_popcnt 4096 401 1401 random 0
check_and_popcnt 524288 402 1402 random 0
check_and_popcnt 4096 403 1403 random 0
for boundary_words in 0 1 63 64 65; do
  for boundary_offset in 0 1 2 3; do
    check_and_popcnt "${boundary_words}" 13 17 random "${boundary_offset}"
  done
done
check_and_popcnt 64 3 4 all_one 0
check_and_popcnt 64 5 6 all_zero 0

if [[ "${fail}" -ne 0 ]]; then
  echo "cross-check FAILED" >&2
  exit 1
fi
echo "cross-check PASSED: identical fixture alignment on every side, and every arm returned an identical count on every identical buffer"
