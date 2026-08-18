#!/usr/bin/env bash
# Re-derive excerpts/toolchain-versions.txt.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
{
  echo "# Toolchain versions observed in the worktree of JIT issue 34d85cb9."
  echo "# Reproduce with: dev/active/34d85cb9/extraction/versions.sh"
  echo
  echo "$ charon version";            charon version
  echo; echo "$ charon toolchain-version"; charon toolchain-version
  echo; echo "$ charon toolchain-path";    charon toolchain-path
  echo; echo "$ aeneas -version";          aeneas -version
  echo; echo "$ rustc --version";          rustc --version
  echo; echo "$ cargo --version";          cargo --version
} >"$HERE/excerpts/toolchain-versions.txt"
