#!/usr/bin/env bash
# Queue launcher of the decoder calibration campaign (jit:0a77ecfc).
# Usage: run-campaign.sh, with no arguments
# Resumes the issue's one existing stage. Exits 75 while the campaign is
# incomplete, which keeps its queue line open.
set -euo pipefail
issue=0a77ecfc
if [[ $# -ne 0 ]]; then
  echo 'usage: run-campaign.sh' >&2
  exit 2
fi
# The benchmark window has no login shell.
export PATH="${HOME:?}/.cargo/bin:/usr/local/bin:/usr/bin${PATH:+:$PATH}"
repo=$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)
cd "$repo"
repository_files=$(git ls-files -- ':(glob)**/repository_files.py' ':(exclude,glob)**/inputs/**')
launcher=$(python3 -B "$repository_files" file tuning-extent-campaign.sh)
stages() {
  local stage
  for stage in /tmp/gf2-"$issue"-*; do
    [[ -d $stage ]] && printf '%s\n' "$stage"
  done
}
mapfile -t found < <(stages)
case ${#found[@]} in
  0) "./$launcher" "$issue" ;;
  1) "./$launcher" "${found[0]##*/}" ;;
  *)
    echo "several stages of $issue exist: ${found[*]}" >&2
    exit 2
    ;;
esac
mapfile -t found < <(stages)
if [[ ${#found[@]} -ne 1 ]]; then
  echo "the launcher left ${#found[@]} stages of $issue" >&2
  exit 2
fi
# publish-campaign exits 3 while the campaign is incomplete.
status=0
"${found[0]}/bin/driver" publish-campaign "${found[0]}" || status=$?
if [[ $status -eq 3 ]]; then
  echo "campaign ${found[0]##*/} is incomplete; its queue line stays open" >&2
  exit 75
fi
exit "$status"
