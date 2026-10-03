#!/usr/bin/env bash
# Prints the root each bench-window script resolves from a second clone,
# relative to the invoking checkout's root.
set -euo pipefail

root=$(git rev-parse --show-toplevel)
clone="$root/target/18858de1-second-clone"
rm -rf "$clone"
git clone --quiet --local "$root" "$clone"
trap 'rm -rf "$clone"' EXIT

echo "clone: ${clone#"$root"/} at $(git -C "$clone" rev-parse HEAD)"
for script in $(git ls-files -- '*/bench-window/run-window.sh' '*/bench-window/follow-window.sh'); do
    echo "\$ <clone>/$script --print-root"
    resolved=$(cd / && "$clone/$script" --print-root)
    echo "${resolved#"$root"/}"
    [[ "$resolved" == "$clone" ]]
done
echo "window state created in clone: $([[ -e "$clone/.agents/bench-window" ]] && echo yes || echo no)"
