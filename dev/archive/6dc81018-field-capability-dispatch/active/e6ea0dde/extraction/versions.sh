#!/usr/bin/env bash
# Re-derive excerpts/toolchain-versions.txt for JIT issue e6ea0dde.
#
# Three toolchains are in play and all three are recorded:
#   * the workspace toolchain that builds gf2-core, pinned to the MSRV by
#     `RUSTUP_TOOLCHAIN=1.95.0` at the head of every Charon .cmd file;
#   * the nightly Charon drives itself for the rustc that produces MIR, which
#     is a property of the Charon build and not selectable per run;
#   * the Lean toolchain the elaboration harness runs under.
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../../.." && pwd)"
{
  echo "# Toolchain versions observed in the worktree of JIT issue e6ea0dde."
  echo "# Reproduce with: dev/active/e6ea0dde/extraction/versions.sh"
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
  echo "## Pinned Charon/Aeneas pair — every X* and Q1-Q5 run"
  echo "\$ command -v charon";             command -v charon
  echo "\$ charon version";                charon version
  echo "\$ charon toolchain-version";      charon toolchain-version
  echo "\$ charon toolchain-path";         charon toolchain-path
  echo "\$ command -v aeneas";             command -v aeneas
  echo "\$ aeneas -version";               aeneas -version
  echo
  echo "## Newest-upstream pair — the Q6 diagnostic probe only"
  NEW_CHARON=/data/aeneas-upgrade-34d85cb9/aeneas/charon/bin/charon
  NEW_AENEAS=/data/aeneas-upgrade-34d85cb9/aeneas/src/_build/default/main.exe
  echo "\$ $NEW_CHARON version";  "$NEW_CHARON" version
  echo "\$ $NEW_AENEAS -version"; "$NEW_AENEAS" -version
  echo
  echo "## Lean toolchain of the elaboration harness"
  echo "\$ cat $ROOT/proofs/lean-toolchain"; cat "$ROOT/proofs/lean-toolchain"
  echo "\$ (cd proofs && lean --version)";  (cd "$ROOT/proofs" && lean --version)
  echo "\$ (cd proofs && lake --version)";  (cd "$ROOT/proofs" && lake --version)
} >"$HERE/excerpts/toolchain-versions.txt"
echo "wrote $HERE/excerpts/toolchain-versions.txt"
