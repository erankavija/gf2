#!/usr/bin/env bash
# Build the 53c5a8c0 arm executables in their two targeting variants
# (jit:53c5a8c0).
#
# Usage: ./build-arms.sh
#
# The crossover family measures two gf2 entry points that select their kernel
# at run time, so its arms are the portable build a consumer gets by default.
# The polynomial family compares gf2 with an external library, so both of its
# sides are host-targeted and neither is measured under a handicap. Each
# variant builds into its own target directory inside this worktree, so the two
# executables have distinct digests in a receipt.
#
# The pinned gf2x build is the export the polynomial baselines survey staged:
# its source commit is checked here before anything links against it. The build
# is shared and is never rebuilt or removed by this script.
#
# Every compile takes the shared side of the canonical CCX1 mutex through
# `scripts/cargo-budget.sh`, so it can never overlap a timed run on this host.

set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"
export PATH="$HOME/.cargo/bin:$PATH" RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-1.95}" CARGO_CI_NO_SCCACHE=1

GF2X_COMMIT=27ba588f03bf6e1e74763903bab25e6e8bb6d0f0
# External comparator sources are shared and live in the primary checkout,
# which is the parent of the common git directory whether this is a worktree
# or the checkout itself.
primary=$(cd "$(dirname "$(git rev-parse --git-common-dir)")" && pwd)
GF2X_SOURCE=${GF2X_SOURCE:-$primary/.agents/ext/c7113c5a/gf2x}
GF2X_PREFIX_DIR=${GF2X_PREFIX_DIR:-$(dirname "$GF2X_SOURCE")/prefix-native}
MANIFEST="dev/active/53c5a8c0/survey/arms/Cargo.toml"

if [[ ! -d "$GF2X_SOURCE" ]]; then
  echo "gf2x source $GF2X_SOURCE is absent; stage it with dev/active/c7113c5a/survey/fetch-build.sh" >&2
  exit 2
fi
head=$(git -C "$GF2X_SOURCE" rev-parse HEAD)
if [[ "$head" != "$GF2X_COMMIT" ]]; then
  echo "gf2x source is at $head, not the pinned $GF2X_COMMIT" >&2
  exit 2
fi
if [[ ! -f "$GF2X_PREFIX_DIR/lib/libgf2x.so" ]]; then
  echo "no gf2x library under $GF2X_PREFIX_DIR" >&2
  exit 2
fi
# The license condition gf2x's own configure.ac tests, asserted rather than
# assumed: without it every size would use Karatsuba and the comparison would
# be against a different algorithm than the one the build evidence records.
if ! grep -q "released under the GPL" "$GF2X_SOURCE/toom-gpl.c"; then
  echo "gf2x toom-gpl.c is the placeholder; the measured build is not the GPL configuration" >&2
  exit 2
fi

build() {
  local name="$1" rustflags="$2"
  CARGO_TARGET_DIR="$repo/target/53c5a8c0-arms-$name" \
  RUSTFLAGS="$rustflags" \
  GF2X_PREFIX="$GF2X_PREFIX_DIR" \
  GF2X_CFLAGS="$(sed -n 's/^configure: *CFLAGS="\(.*\)"$/\1/p' \
      "$(dirname "$GF2X_SOURCE")/build-native/configure.stdout.log" | head -1)" \
    "$repo/scripts/cargo-budget.sh" cargo build --release --locked --manifest-path "$MANIFEST"
}

build conservative ""
build native "-C target-cpu=native"

cat <<SUMMARY
gf2x source   : $GF2X_SOURCE at $GF2X_COMMIT
gf2x library  : $GF2X_PREFIX_DIR/lib/libgf2x.so
conservative  : $repo/target/53c5a8c0-arms-conservative/release
native        : $repo/target/53c5a8c0-arms-native/release
toolchain     : $(rustc --version)
SUMMARY
