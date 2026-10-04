# Zen 3 flat-entry regroup record

Issue 4f161183 covers the 14 flat `dev/active/` entries of `investigation.md`
Appendix A whose top epic is 1a379447, less the 20 entries the issue excludes
by name. `E` is `dev/active/1a379447-zen3-cpu-performance`, the directory
`jit doc dir 1a379447 dev/active` prints.

## Entries

Every manifest row of the 14 entries names epic 1a379447 alone, so the
multi-owner rule of plan contract `active-layout` selects no other directory,
and every row's `destination` is its `path` under `E`.

| Entry | Owners | Owning epic | Rule applied | Destination | Result | Reason |
|---|---|---|---|---|---|---|
| 00dd43c3 | 00dd43c3 | 1a379447 | single epic | `E/00dd43c3` | left | Script consumer `dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/make-publication-tables.py:326`; digest-pinned own scripts name the entry path (`survey/run-campaign.sh:22`). |
| 19513245 | 19513245 | 1a379447 | single epic | `E/19513245` | left | Digest-pinned `survey/consumer/Cargo.toml:25-27` and `survey/field-laws/Cargo.toml:18-20` hold depth-dependent `path` dependencies; digest-pinned `survey/make-producing-inputs.py:23` names the entry path. |
| 428f2f6b | 428f2f6b | 1a379447 | single epic | `E/428f2f6b` | moved | |
| 50f0bd42 | 50f0bd42 | 1a379447 | single epic | `E/50f0bd42` | moved | |
| 613574db | 613574db | 1a379447 | single epic | `E/613574db` | left | `git log main..<branch> -- dev/active/613574db` lists 4 commits on each of `worktree-agent-ad2a6a58`, `worktree-agent-f63a2464` and `worktree-agent-4c1e441f`. `git cherry main <branch>` marks none of them as absent from main. |
| 9f0e385e | 9f0e385e, 9fb40c83 | 1a379447 | single epic | `E/9f0e385e` | moved | |
| a1ad6d4e | a1ad6d4e | 1a379447 | single epic | `E/a1ad6d4e` | moved | |
| a203a23c | a203a23c, 7cdc28e9 | 1a379447 | single epic | `E/a203a23c` | left | The same three branches each list 4 commits on the path, none absent from main by `git cherry`. `timing/measure.py:99` reads the entry's own path at recorded revisions. |
| bc091474 | bc091474 | 1a379447 | single epic | `E/bc091474` | left | Digest-pinned script consumer `dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/make-logical-producing-inputs.py:42,83`; digest-pinned `run-candidate.sh:8,100` resolves the root by depth and names the entry path. |
| bdc507a3 | bdc507a3 | 1a379447 | single epic | `E/bdc507a3` | left | Digest-pinned `dev/active/f547c394/amendment-v4.md:27` links `../bdc507a3/verdict-preservation.json`; the link resolves only while both entries are siblings. Manifest consumer `compare-acceptance-verdicts.sh:112,134`. |
| c04dd4ac-zen3-shifts-and-permutations | c04dd4ac, 85fc5ff4, 8ed3ac58, 9fb40c83, a0812b83 | 1a379447 | single epic | `E/c04dd4ac-zen3-shifts-and-permutations` | left | Test consumer `dev/active/eda07788/survey/gf2-side/tests/profile_addendum.rs:17-21,251`; script consumers `dev/active/00dd43c3/survey/run-campaign.sh:48` and `make-producing-inputs.py:56`; digest-pinned own scripts name the entry path; the three branches above list 7, 11 and 33 commits on the path. |
| c73ffa25 | c73ffa25 | 1a379447 | single epic | `E/c73ffa25` | moved | |
| f8dd4dde | f8dd4dde, 00dd43c3 | 1a379447 | single epic | `E/f8dd4dde` | left | `msrv-validation/Cargo.toml:23` holds a depth-dependent `path` dependency on `crates/gf2-core`; `validate-msrv.sh:11,13` names the entry path. |
| fcb04d66 | fcb04d66 | 1a379447 | single epic | `E/fcb04d66` | moved | |

The six moved entries hold no script; REQ-06 applies to no file.

## Consumer check

```sh
python3 - <<'EOF'
import tomllib
m = tomllib.load(open("dev/active/fa787f85-documentation-overhaul/migration/manifest.toml", "rb"))
for r in m["artifacts"]:
    if r["epic"] == "1a379447" and r["consumers"]:
        print(r["path"], *r["consumers"])
EOF
git grep -nF "dev/active/<entry>" -- ':!dev/archive' ':!*/inputs/*' ':!.jit' \
  ':!dev/active/fa787f85-documentation-overhaul/migration/manifest.toml'
```

The first command prints no row of the six moved entries. Run at the parent of
move commit `eb481bd02`, the second prints, for each moved entry, only Markdown
citations and one `progress.json` string. It matches root-relative path text;
a consumer that reaches an entry by a relative path is outside it.

## Citations left at the flat path

| File | Reason |
|---|---|
| `E/progress.json:1423` | `git log main..<branch>` lists commits on the file for the three branches above. |
| `60652fe4-roadmap-map.md:14,18` | Branch `worktree-agent-103a792a` carries an unmerged commit on the file. |

Receipt snapshot `inputs/` trees and `dev/archive/` hold no citation of the six
moved entries.

## Link scan

`python3 contrib/gates/docs-mechanical.py` prints
`docs-mechanical: PASS: 0 finding(s), 1 baselined; 384 Markdown files, 736 Rust sources, 133 item references`.
Its `markdown` footprint `dev/active/**/*.md` contains every Markdown file the
4f161183 commits touch or move. It checks inline and reference links and their
anchors; it does not check code-span path citations under `dev/active/`. The
baselined row is `E/9f0e385e/coverage-addressability.md:19`.

## Tracker references

`target/4f161183-relink.sh` (uncommitted) repoints the 16 document references
of issues 428f2f6b, 50f0bd42, 9f0e385e, 9fb40c83, a1ad6d4e, c73ffa25 and
fcb04d66.
