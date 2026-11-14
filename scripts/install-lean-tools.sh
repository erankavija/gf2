#!/usr/bin/env bash
# Build the translator matching the Lean library and its upstream-pinned extractor.
# Requires git, Python 3.11+, rustup, and an initialized opam switch.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TOOLS_DIR="${LEAN_TOOLS_DIR:-$REPO_ROOT/target/lean-tools}"
mkdir -p "$TOOLS_DIR"
TOOLS_DIR="$(cd "$TOOLS_DIR" && pwd)"
AENEAS_REV="$(python3 - "$REPO_ROOT/proofs/lakefile.lean" <<'PY'
import re
import sys
from pathlib import Path

text = Path(sys.argv[1]).read_text()
match = re.search(r'require aeneas from git "[^"]+" @ "([0-9a-f]{40})"', text)
if match is None:
    sys.exit("Expected a full Aeneas git revision in proofs/lakefile.lean")
print(match[1])
PY
)"

checkout() {
  local url="$1" directory="$2" revision="$3"
  if [ ! -d "$directory/.git" ]; then
    git clone "$url" "$directory"
  fi
  git -C "$directory" fetch origin "$revision"
  git -C "$directory" checkout --detach "$revision"
  git -C "$directory" submodule update --init --recursive
}

checkout https://github.com/AeneasVerif/aeneas.git "$TOOLS_DIR/aeneas" "$AENEAS_REV"
CHARON_REV="$(tail -n 1 "$TOOLS_DIR/aeneas/charon-pin")"
checkout https://github.com/AeneasVerif/charon.git "$TOOLS_DIR/aeneas/charon" "$CHARON_REV"
CHARON_DIR="$TOOLS_DIR/aeneas/charon"

if [ "$(cat "$REPO_ROOT/proofs/lean-toolchain")" != \
     "$(cat "$TOOLS_DIR/aeneas/backends/lean/lean-toolchain")" ]; then
  echo "ERROR: proofs/lean-toolchain must match the selected Aeneas backend" >&2
  exit 1
fi
RUST_TOOLCHAIN="$(python3 - "$CHARON_DIR/charon/rust-toolchain" <<'PY'
import sys
import tomllib
from pathlib import Path
print(tomllib.loads(Path(sys.argv[1]).read_text())["toolchain"]["channel"])
PY
)"
rustup toolchain install "$RUST_TOOLCHAIN" --profile minimal \
  --component rustc-dev,llvm-tools,rust-src,miri
cargo +"$RUST_TOOLCHAIN" build --release --locked \
  --target-dir "$CHARON_DIR/charon/target" \
  --manifest-path "$CHARON_DIR/charon/Cargo.toml" --bin charon --bin charon-driver
make -C "$CHARON_DIR" charon-ml/src/CharonVersion.ml

# Use upstream package constraints, including the PPX versions needed by charon-ml.
opam install --yes --deps-only "$TOOLS_DIR/aeneas/src/aeneas.opam" \
  "$CHARON_DIR/charon.opam" "$CHARON_DIR/name_matcher_parser.opam"
opam exec -- dune build --root "$TOOLS_DIR/aeneas/src" --profile release main.exe

mkdir -p "$TOOLS_DIR/bin"
install -m 755 "$CHARON_DIR/charon/target/release/charon" \
   "$CHARON_DIR/charon/target/release/charon-driver" "$TOOLS_DIR/bin/"
install -m 755 "$TOOLS_DIR/aeneas/src/_build/default/main.exe" "$TOOLS_DIR/bin/aeneas"
echo "Installed Aeneas $AENEAS_REV / Charon $CHARON_REV in $TOOLS_DIR/bin"
