#!/usr/bin/env bash
# Re-derive excerpts/toolchain-versions.txt for the pinned-pair (baseline) leg.
#
# Two toolchains are in play and both are recorded:
#   * the workspace toolchain that builds gf2-core, pinned to the MSRV by
#     `RUSTUP_TOOLCHAIN=1.95.0` at the head of every Charon .cmd file;
#   * the nightly Charon drives itself for the rustc that produces MIR, which
#     is a property of the Charon build and not selectable per run.
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../../.." && pwd)"
{
  echo "# Toolchain versions observed in the worktree of JIT issue 34d85cb9."
  echo "# Reproduce with: dev/active/34d85cb9/extraction/versions.sh"
  echo "# Captured: $(date -Is)"
  echo
  echo "## Workspace toolchain — pinned to the MSRV for every run"
  echo "\$ grep -n rust-version $ROOT/crates/gf2-core/Cargo.toml"
  grep -n "rust-version" "$ROOT/crates/gf2-core/Cargo.toml"
  echo
  echo "\$ RUSTUP_TOOLCHAIN=1.95.0 cargo --version"
  RUSTUP_TOOLCHAIN=1.95.0 cargo --version
  echo "\$ RUSTUP_TOOLCHAIN=1.95.0 rustc --version"
  RUSTUP_TOOLCHAIN=1.95.0 rustc --version
  echo
  echo "# For reference only, NOT what the runs used: this host's default"
  echo "# toolchain, which is newer than the MSRV."
  echo "\$ rustc --version"
  rustc --version
  echo
  echo "## Charon's own driver toolchain (not selectable per run)"
  echo "\$ charon version";            charon version
  echo "\$ charon toolchain-version";  charon toolchain-version
  echo "\$ charon toolchain-path";     charon toolchain-path
  echo
  echo "## Aeneas"
  echo "\$ aeneas -version";           aeneas -version
} >"$HERE/excerpts/toolchain-versions.txt"
echo "wrote $HERE/excerpts/toolchain-versions.txt"
