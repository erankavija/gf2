#!/usr/bin/env bash
# Capture the upstream branch/tag topology that justifies calling the upgrade
# pair "newest upstream", into excerpts/upstream-topology.txt.
#
# Everything here is read-only network access (`git ls-remote`, raw file fetch);
# no local repository is touched.
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
OUT="$HERE/excerpts/upstream-topology.txt"
mkdir -p "$HERE/excerpts"

AENEAS=https://github.com/AeneasVerif/aeneas
CHARON=https://github.com/AeneasVerif/charon

{
  echo "# Upstream topology receipt for the 34d85cb9 upgrade leg."
  echo "# Captured: $(date -Is)"
  echo "# Reproduce with: dev/active/34d85cb9/upgrade/topology.sh"
  echo
  echo "## \$ git ls-remote $AENEAS HEAD refs/heads/main"
  git ls-remote "$AENEAS" HEAD refs/heads/main
  echo
  echo "## \$ git ls-remote --tags $AENEAS | tail -5   (newest nightly tags)"
  git ls-remote --tags "$AENEAS" | tail -5
  echo
  echo "## \$ git ls-remote $CHARON HEAD refs/heads/main"
  git ls-remote "$CHARON" HEAD refs/heads/main
  echo
  echo "## charon-pin declared by aeneas main HEAD (c10cc997)"
  curl -sS "https://raw.githubusercontent.com/AeneasVerif/aeneas/c10cc997a2cc885f5b2c9ef3929cc05632cb106c/charon-pin" | tail -1
  echo
  echo "## charon-pin declared by the newest aeneas tag (daa85d7e)"
  curl -sS "https://raw.githubusercontent.com/AeneasVerif/aeneas/daa85d7e89400fa978be83fedbc7e475a83f0889/charon-pin" | tail -1
  echo
  echo "## aeneas main HEAD relative to the newest tag (GitHub compare API)"
  curl -sS "https://api.github.com/repos/AeneasVerif/aeneas/compare/daa85d7e89400fa978be83fedbc7e475a83f0889...c10cc997a2cc885f5b2c9ef3929cc05632cb106c" \
    | python3 -c "import json,sys; d=json.load(sys.stdin); print(f\"status={d.get('status')} ahead_by={d.get('ahead_by')} behind_by={d.get('behind_by')}\")"
  echo
  echo "## rust toolchain required by charon 340b1af4"
  curl -sS "https://raw.githubusercontent.com/AeneasVerif/charon/340b1af4df92608d0911fc2ba26eef3fd3a30ab4/charon/rust-toolchain" | head -3
} >"$OUT"

echo "wrote $OUT"
