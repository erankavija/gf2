#!/usr/bin/env bash
# Re-derive excerpts/host-context.txt.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
{
  echo "# Host and revision context for the 34d85cb9 spike runs."
  echo "# Reproduce with: dev/active/34d85cb9/extraction/host.sh"
  echo
  echo "$ git rev-parse HEAD";            git rev-parse HEAD
  echo; echo "$ uname -srm";              uname -srm
  echo; echo "$ grep -m1 'model name' /proc/cpuinfo"; grep -m1 'model name' /proc/cpuinfo
  echo; echo "$ nproc";                   nproc
  echo; echo "$ free -h";                 free -h
} >"$HERE/excerpts/host-context.txt"
