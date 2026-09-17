#!/usr/bin/env bash
set -euo pipefail

# Rebuilds the planning-time feasibility artefacts for JIT issue 85fc5ff4
# REQ-05: the toolchain identity the prototype is compiled with, and the
# emitted assembly of each hot loop. Untimed; run from this directory.
#
# Re-running on an unchanged tree reproduces every artefact byte for byte.

cd "$(dirname "$0")"

TOOLCHAIN=1.95

# Plan constant: the `#[no_mangle]` hot loops under study.
SYMBOLS=(
  shift_left_funnel_avx2
  shift_right_funnel_avx2
  shift_left_funnel_bmi2_pair
  shift_right_funnel_bmi2_pair
  shift_left_funnel_bmi2_dp
  shift_right_funnel_bmi2_dp
)

# Intel syntax only; no -C target-cpu and no -C target-feature, so the build
# baseline stays x86-64 and every wide instruction below comes from a
# `#[target_feature]` scope rather than from a flag.
RUSTC_ARGS=(--emit asm -C llvm-args=--x86-asm-syntax=intel)

{
  echo "# Toolchain and build identity for the residual-shift feasibility prototype."
  echo "# Regenerate with ./capture-asm.sh"
  echo
  echo "## rustc +${TOOLCHAIN} --version --verbose"
  rustc "+${TOOLCHAIN}" --version --verbose
  echo
  echo "## cargo +${TOOLCHAIN} --version --verbose"
  cargo "+${TOOLCHAIN}" --version --verbose
  echo
  echo "## cargo +${TOOLCHAIN} rustc --release --lib -- ${RUSTC_ARGS[*]}"
  echo
  echo "## rustc +${TOOLCHAIN} --print cfg (target_feature lines: the build baseline)"
  rustc "+${TOOLCHAIN}" --print cfg | grep '^target_feature' | sort
} > toolchain.txt

# rustc's own verdict on the double-precision shift intrinsic, whichever way
# it falls. The probe is expected to fail to compile; a success is recorded
# just as plainly.
{
  echo "# Does rustc ${TOOLCHAIN} expose _shld_u64 / _shrd_u64?"
  echo "# Regenerate with ./capture-asm.sh; source: probe/shld_probe.rs"
  echo
  echo "## rustc +${TOOLCHAIN} --edition 2021 --crate-type lib --emit metadata probe/shld_probe.rs"
  if rustc "+${TOOLCHAIN}" --edition 2021 --crate-type lib --emit metadata \
      -o /dev/null probe/shld_probe.rs 2>&1; then
    echo "exit status: 0"
  else
    echo "exit status: $?"
  fi
} > intrinsic-probe.txt

cargo "+${TOOLCHAIN}" clean --release -p residual_shift_feasibility
cargo "+${TOOLCHAIN}" rustc --release --lib -- "${RUSTC_ARGS[@]}"

SOURCE=$(ls target/release/deps/residual_shift_feasibility-*.s)
mkdir -p asm
for sym in "${SYMBOLS[@]}"; do
  awk -v s="$sym" '$0 == s ":" {on=1} on {print} on && $0 ~ ("^\t\\.size\t" s) {exit}' \
    "$SOURCE" > "asm/${sym}.s"
  # A symbol that vanished from the emitted assembly is a capture failure, not
  # an empty artefact to be committed.
  [ -s "asm/${sym}.s" ] || { echo "capture-asm: no body emitted for $sym" >&2; exit 1; }
done
