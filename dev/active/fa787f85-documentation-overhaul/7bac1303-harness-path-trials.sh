#!/usr/bin/env bash
# usage: 7bac1303-harness-path-trials.sh [outfile]
# Resolves the root and harness manifests on Rust 1.95 under three dependency
# forms, in a scratch copy of HEAD below the git-ignored target/ directory. The
# working tree is read only; the scratch copy is removed on exit. The default
# outfile is 7bac1303-harness-path-trials.txt beside this script.
set -u
here=$(cd "$(dirname "$0")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel)
out=$(realpath "${1:-$here/7bac1303-harness-path-trials.txt}")
scratch="$root/target/7bac1303-harness-path-trials"
repo="$scratch/repo"
active=$(realpath --relative-to="$root" "$here/..")
trap 'rm -rf "$scratch"' EXIT

fresh() { rm -rf "$repo"; mkdir -p "$repo"; git -C "$root" archive HEAD | tar -x -C "$repo"; }

# Harness manifests: standalone manifests of the active-development tree that
# carry a parent-relative path dependency.
harness_manifests() {
  (cd "$here" && git grep -l --full-name -E 'path *= *"\.\./' -- \
    ':(glob)../**/Cargo.toml' ':(exclude,glob)../**/inputs/**') | sort
}

locked_manifests() {
  git -C "$root" ls-files -- ':(glob)**/Cargo.lock' ':(exclude,glob)**/inputs/**' |
    sed 's/Cargo\.lock$/Cargo.toml/' | sort
}

# resolve <label> <cargo args...>: runs in the scratch copy, prints the exit
# status, each unused-patch warning and the first error line.
resolve() {
  local label=$1; shift
  (cd "$repo" && ./scripts/cargo-budget.sh cargo +1.95.0 metadata --format-version 1 --offline "$@" \
    >"$scratch/stdout" 2>"$scratch/err")
  local rc=$?
  local unused
  unused=$(grep -o 'patch `[A-Za-z0-9_-]*' "$scratch/err" | sed 's/patch `//' | sort | tr '\n' ' ')
  echo "$label rc=$rc unused=[${unused% }] $(grep -m1 '^error' "$scratch/err")"
}

# One `name = { path = "<root-relative>" }` line per distinct package that a
# harness manifest reaches through a parent-relative path.
patch_entries() {
  local base=$1
  harness_manifests | while read -r m; do
    grep -o -E 'path *= *"\.\./[^"]*"' "$root/$m" | sed -E 's/path *= *"//; s/"$//' |
    while read -r p; do
      d=$(realpath --relative-to="$root/$base" "$root/$(dirname "$m")/$p")
      n=$(sed -n -E 's/^name *= *"([^"]*)".*/\1/p' "$root/$(dirname "$m")/$p/Cargo.toml" | head -1)
      echo "$n = { path = \"$d\" }"
    done
  done | sort -u
}

{
echo "# toolchain"
cargo +1.95.0 --version
rustc +1.95.0 --version
echo "# source: $(git -C "$root" rev-parse HEAD)"

echo
echo "# harness manifests"
harness_manifests

echo
echo "# control: --locked resolution of every tracked lockfile, repository unchanged"
fresh
locked_manifests | while read -r m; do resolve "$m" --locked --manifest-path "$m"; done > "$scratch/control"
cat "$scratch/control"
echo "control: $(grep -c ' rc=0 ' "$scratch/control") of $(wc -l < "$scratch/control") resolve"

echo
echo "# form 1: [patch.crates-io] table appended to .cargo/config.toml"
{ echo; echo "[patch.crates-io]"; patch_entries .; } > "$scratch/patch.toml"
cat "$scratch/patch.toml"
cat "$scratch/patch.toml" >> "$repo/.cargo/config.toml"
locked_manifests | while read -r m; do resolve "$m" --locked --manifest-path "$m"; done > "$scratch/form1"
cat "$scratch/form1"
echo "form 1: $(grep -c ' rc=0 ' "$scratch/form1") of $(wc -l < "$scratch/form1") resolve"
echo "form 1 lockfile diff of the root manifest after resolution without --locked:"
resolve Cargo.toml --manifest-path Cargo.toml
(cd "$scratch" && git -C "$root" show HEAD:Cargo.lock | diff - repo/Cargo.lock)

echo
echo "# form 2: the same table in .cargo/harness-paths.toml, passed with --config"
fresh
cp "$scratch/patch.toml" "$repo/.cargo/harness-paths.toml"
harness_manifests | while read -r m; do
  sed -i -E 's/path *= *"\.\.\/[^"]*"/version = "*"/' "$repo/$m"
done
echo "harness manifests declare each such dependency as version = \"*\":"
harness_manifests | while read -r m; do
  resolve "$m without-flag" --manifest-path "$m"
  resolve "$m with-flag" --config .cargo/harness-paths.toml --manifest-path "$m"
done > "$scratch/form2"
cat "$scratch/form2"
echo "form 2 without flag: $(grep ' without-flag ' "$scratch/form2" | grep -c ' rc=0 ') of $(grep -c ' without-flag ' "$scratch/form2") resolve"
echo "form 2 with flag: $(grep ' with-flag ' "$scratch/form2" | grep -c ' rc=0 ') of $(grep -c ' with-flag ' "$scratch/form2") resolve"
echo "form 2 root manifest, no flag:"
resolve Cargo.toml --locked --manifest-path Cargo.toml
echo "form 2 tracked scripts of the active tree that invoke cargo build, run or test,"
echo "with the number of tracked JSON files that hold the script's SHA-256:"
(cd "$here" && git grep -l --full-name -E 'cargo[^#]*\<(build|run|test)\>' -- \
  ':(glob)../**/*.sh' ':(glob)../**/*.py' ':(exclude,glob)../**/inputs/**') | sort > "$scratch/scripts"
while read -r s; do sha256sum "$root/$s" | cut -d' ' -f1; done < "$scratch/scripts" > "$scratch/digests"
git -C "$root" grep -o -F -f "$scratch/digests" -- ':(glob)**/*.json' | sort -u > "$scratch/pins"
paste "$scratch/scripts" "$scratch/digests" | while read -r s d; do
  echo "$s sha256=$d pinned_in=$(grep -c ":$d\$" "$scratch/pins")"
done > "$scratch/pinned"
cat "$scratch/pinned"
echo "form 2: $(grep -c -v ' pinned_in=0$' "$scratch/pinned") of $(wc -l < "$scratch/pinned") scripts are digest-pinned"

echo
echo "# form 3: harness workspace at the fixed location $active"
fresh
h=$(harness_manifests | head -1)
hd=$(dirname "$h")
echo "member under trial: $h"
echo "[workspace] tables of the harness manifests:"
harness_manifests | (cd "$root" && xargs git grep -n '^\[workspace\]' --)
{
  echo "[workspace]"
  echo "members = [\"$(realpath --relative-to="$root/$active" "$root/$hd")\"]"
  echo 'resolver = "2"'
  echo
  echo "[workspace.dependencies]"
  patch_entries "$active"
} > "$repo/$active/Cargo.toml"
echo "workspace manifest:"
cat "$repo/$active/Cargo.toml"
echo "member manifest keeps its [workspace] table, path dependencies as workspace = true:"
sed -i -E 's/path *= *"\.\.\/[^"]*"/workspace = true/' "$repo/$h"
resolve "$h" --manifest-path "$h"
grep -A8 '^error' "$scratch/err"
echo "member manifest without its [workspace] table; the lockfile it resolves into:"
sed -i -E '/^\[workspace\]$/d' "$repo/$h"
resolve "$h" --manifest-path "$h"
(cd "$repo" && ls "$active/Cargo.lock")
echo "the same member copied one directory level deeper:"
mkdir -p "$repo/$hd.deeper"
cp -r "$repo/$hd" "$repo/$hd.deeper/"
resolve "$hd.deeper/$(basename "$hd")/Cargo.toml" --manifest-path "$hd.deeper/$(basename "$hd")/Cargo.toml"
grep -A3 '^error' "$scratch/err"
} 2>&1 | sed -e "s#$repo/##g" -e "s#$repo#.#g" -e "s#$root/##g" -e "s#$HOME#~#g" > "$out"
