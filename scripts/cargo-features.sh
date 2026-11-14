#!/bin/sh
# Prints the Cargo feature flags of a workspace command on this host:
# `--all-features` where ROCm `hipcc` is on PATH or at /opt/rocm/bin/hipcc,
# otherwise `--features FEATURES` (default: the workspace set below), since the
# `hip` features build `gf2-kernels-hip`, whose build script runs hipcc.
#
# Usage, output expanded unquoted:
#   cargo build --workspace $(./scripts/cargo-features.sh [FEATURES])
if command -v hipcc >/dev/null 2>&1 || [ -x /opt/rocm/bin/hipcc ]; then
  echo "--all-features"
else
  echo "--features ${1:-simd,parallel,visualization}"
fi
