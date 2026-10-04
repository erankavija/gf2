#!/usr/bin/env bash
# Re-derive excerpts/toolchain-versions.txt for JIT issue 7d7c647c.
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../../.." && pwd)"
{
  echo "# Toolchain identities for JIT issue 7d7c647c, captured at run time."
  echo "# Reproduce with: dev/active/7d7c647c/probes/versions.sh"
  echo "# Captured: $(date -Is)"
  echo
  echo "RUSTUP_TOOLCHAIN=1.95.0 cargo --version: $(RUSTUP_TOOLCHAIN=1.95.0 cargo --version)"
  echo "RUSTUP_TOOLCHAIN=1.95.0 rustc --version: $(RUSTUP_TOOLCHAIN=1.95.0 rustc --version)"
  echo "charon version:                         $(charon version)"
  echo "charon toolchain-version:               $(charon toolchain-version)"
  echo "aeneas -version:                        $(aeneas -version)"
  echo "proofs/lean-toolchain:                  $(cat "$ROOT/proofs/lean-toolchain")"
  echo "crates/gf2-core rust-version:           $(grep '^rust-version' "$ROOT/crates/gf2-core/Cargo.toml")"
  echo "worktree HEAD:                          $(cd "$ROOT" && git rev-parse HEAD)"
} >"$HERE/excerpts/toolchain-versions.txt"
echo "wrote $HERE/excerpts/toolchain-versions.txt"
