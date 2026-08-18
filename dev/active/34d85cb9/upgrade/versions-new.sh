#!/usr/bin/env bash
# Re-derive excerpts/toolchain-versions-new.txt for the upgrade leg: the
# newest-upstream Charon/Aeneas built under /data/aeneas-upgrade-34d85cb9, the
# OCaml toolchain that built Aeneas, and proof that the host installation was
# left on the pinned pair.
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
NEW=/data/aeneas-upgrade-34d85cb9
{
  echo "# Newest-upstream toolchain built for the 34d85cb9 upgrade leg."
  echo "# Built from fresh clones under $NEW; NOT installed into ~/.cargo/bin."
  echo "# Reproduce with: dev/active/34d85cb9/upgrade/versions-new.sh"
  echo "# Captured: $(date -Is)"
  echo
  echo "## Newest-upstream pair"
  echo "\$ $NEW/aeneas/charon/bin/charon version"
  "$NEW/aeneas/charon/bin/charon" version
  echo "\$ .../charon toolchain-version"
  "$NEW/aeneas/charon/bin/charon" toolchain-version
  echo "\$ git -C $NEW/aeneas/charon rev-parse HEAD"
  git -C "$NEW/aeneas/charon" rev-parse HEAD
  echo "\$ $NEW/aeneas/src/_build/default/main.exe -version"
  "$NEW/aeneas/src/_build/default/main.exe" -version | head -1
  echo "\$ git -C $NEW/aeneas rev-parse HEAD"
  git -C "$NEW/aeneas" rev-parse HEAD
  echo "\$ tail -1 $NEW/aeneas/charon-pin"
  tail -1 "$NEW/aeneas/charon-pin"
  echo
  echo "## Workspace toolchain used by the upgrade leg's runs (same MSRV pin)"
  echo "\$ RUSTUP_TOOLCHAIN=1.95.0 cargo --version"
  RUSTUP_TOOLCHAIN=1.95.0 cargo --version
  echo "\$ RUSTUP_TOOLCHAIN=1.95.0 rustc --version"
  RUSTUP_TOOLCHAIN=1.95.0 rustc --version
  echo
  echo "## OCaml toolchain that built Aeneas"
  eval "$(opam env)" 2>/dev/null || true
  echo "\$ dune --version";   dune --version
  echo "\$ ocaml -version";   ocaml -version
  echo "\$ opam switch show"; opam switch show
  echo
  echo "## Host installation — left on the pinned pair"
  echo "\$ charon version";   charon version
  echo "\$ aeneas -version";  aeneas -version | head -1
  echo "\$ sha256sum ~/.cargo/bin/{charon,charon-driver,aeneas} and their .bak-2026-08-19 copies"
  sha256sum "$HOME/.cargo/bin/charon" "$HOME/.cargo/bin/charon.bak-2026-08-19" \
            "$HOME/.cargo/bin/charon-driver" "$HOME/.cargo/bin/charon-driver.bak-2026-08-19" \
            "$HOME/.cargo/bin/aeneas" "$HOME/.cargo/bin/aeneas.bak-2026-08-19"
} >"$HERE/excerpts/toolchain-versions-new.txt"
echo "wrote $HERE/excerpts/toolchain-versions-new.txt"
