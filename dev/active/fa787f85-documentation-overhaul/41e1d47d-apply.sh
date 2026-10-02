#!/usr/bin/env bash
# Applies the dispositions in 41e1d47d-disposition.md. Run from the repository
# root after owner approval of 41e1d47d-criteria-proposal.md.
set -euo pipefail

dir=dev/active/fa787f85-documentation-overhaul
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# Appends one criterion line to the end of an issue's `## Success Criteria`.
append_criterion() {
  local id=$1 line=$2
  jit issue show "$id" --json | python3 -c '
import json, sys
line = sys.argv[1]
text = json.load(sys.stdin)["description"]
head, sep, tail = text.partition("## Success Criteria\n")
if not sep:
    sys.exit("no Success Criteria section")
cut = tail.find("\n## ")
body, rest = (tail, "") if cut < 0 else (tail[:cut], tail[cut:])
sys.stdout.write(head + sep + body.rstrip("\n") + "\n" + line + "\n" + rest)
' "$line" > "$tmp/$id.md"
  jit issue update "$id" --description-file "$tmp/$id.md"
}

# (a) Criteria additions and the dev/tools sweep unit.
append_criterion fca578b1 '- [hard] REQ-05: Root `AGENTS.md` states `@/issue/<short-id>`, the form the mechanical documentation check resolves, as the way prose and Rustdoc reference a JIT issue, and every issue reference on the permanent surface uses that form.'
append_criterion ffc35b8c '- [hard] REQ-28: Every Rust file under `dev/tools/` meets REQ-21, REQ-22 and REQ-23 through comment, Rustdoc and whitespace edits only.'

sed -n '/^~~~markdown$/,/^~~~$/p' "$dir/41e1d47d-criteria-proposal.md" | sed '1d;$d' > "$tmp/unit.md"
unit=$(jit issue create --json \
  --title 'Tersify comments in the dev/tools crates' \
  --description-file "$tmp/unit.md" \
  --type task \
  --label epic:documentation-overhaul \
  --label component:docs \
  --label story:source-tersification-sweep \
  --label satisfies:ffc35b8c/REQ-28 \
  --gate cargo-ci --gate code-review --gate doc-review --gate docs-mechanical \
  | python3 -c 'import json, sys; d = json.load(sys.stdin); d = d.get("data", d); print(d["id"])')
jit dep add "$unit" 62f0d0e6
jit dep add 030496bd "$unit"

# (b) Filter label on item-level API documentation fixes.
for id in 2d65e37f 99c92597 5deee377 23f22f53 3d34f504 54278d0c aabc528a \
  ad978596 cdaf5da9 157c305c 835f34f0 c2663ce9; do
  jit issue update "$id" --label epic:documentation-overhaul
done

# (c) Rejections; each resolution names the overhaul item carrying the requirement.
reject() {
  local id=$1 note=$2
  jit issue update "$id" --append-description "## Resolution

$note"
  jit issue reject "$id" --reason obsolete
}

reject e2c649cd 'Subsumed by overhaul issue `13dfe1a5`, whose REQ-01 governs example file headers. `cargo doc` does not build example headers, so Rustdoc links there neither render nor get checked.'
reject 9b3452e9 'Subsumed by overhaul issues `a0a29512` (REQ-01, REQ-02) and `2596b143` (REQ-04): each Rustdoc example is removed under the example policy or retained compiling in the Rust CI doctest step.'
reject 0056e853 'Subsumed by overhaul issue `698fa793` (REQ-02, REQ-03), which owns the gf2-coding README. `README_NEW.md` is absent, and a learning-path page with example lists contradicts the entry-page contract.'
reject 315f4de5 'Subsumed by overhaul issue `cdba4e71` and brief D-45, which fix the research tutorial set. Elementary decoder-trace examples are outside the researcher audience.'
reject 35007c4c 'Subsumed by overhaul issue `13dfe1a5`, whose REQ-01 governs example file headers. Difficulty, prerequisite and reading-time headers contradict the comment contract.'
reject 5f3d0ff9 'Subsumed by overhaul issue `cdba4e71` and brief D-45, which fix the research tutorial set. An introductory soft- versus hard-decision example is outside the researcher audience.'
reject be331e20 'Subsumed by overhaul issue `13dfe1a5`, whose REQ-01 governs example comments. `hamming_basic.rs` exists, and elementary Hamming material is outside the researcher audience.'
reject 68189a0a 'Subsumed by overhaul issues `fdb998ec` (REQ-02: the capability map includes gf2-sim) and `fca578b1` (REQ-05: `@/issue/<short-id>` is the recorded issue-reference form and the permanent surface uses it).'
reject 807ddab2 'Subsumed by overhaul issue `ffc35b8c`: REQ-21 to REQ-23 and REQ-25 to REQ-27 cover every file under `crates/`, and REQ-28 covers `dev/tools/`.'
reject 049a89af 'Subsumed by overhaul issue `ec655592` (REQ-03, REQ-04): every `dev/plans/` citation in Rustdoc points to the plan'\''s archived path or is removed.'
