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
| 613574db | 613574db | 1a379447 | single epic | `E/613574db` | moved | |
| 9f0e385e | 9f0e385e, 9fb40c83 | 1a379447 | single epic | `E/9f0e385e` | moved | |
| a1ad6d4e | a1ad6d4e | 1a379447 | single epic | `E/a1ad6d4e` | moved | |
| a203a23c | a203a23c, 7cdc28e9 | 1a379447 | single epic | `E/a203a23c` | left | `timing/measure.py:99` names the entry's own path and `:314` reads it at recorded revisions. |
| bc091474 | bc091474 | 1a379447 | single epic | `E/bc091474` | left | Digest-pinned script consumer `dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/make-logical-producing-inputs.py:42,83`; digest-pinned `run-candidate.sh:8,100` resolves the root by depth and names the entry path. |
| bdc507a3 | bdc507a3 | 1a379447 | single epic | `E/bdc507a3` | left | Digest-pinned `dev/active/f547c394/amendment-v4.md:27` links `../bdc507a3/verdict-preservation.json`; the link resolves only while both entries are siblings. Manifest consumer `compare-acceptance-verdicts.sh:112,134`. |
| c04dd4ac-zen3-shifts-and-permutations | c04dd4ac, 85fc5ff4, 8ed3ac58, 9fb40c83, a0812b83 | 1a379447 | single epic | `E/c04dd4ac-zen3-shifts-and-permutations` | left | Test consumer `dev/active/eda07788/survey/gf2-side/tests/profile_addendum.rs:17-21,251`; script consumers `dev/active/00dd43c3/survey/run-campaign.sh:48` and `make-producing-inputs.py:56`; digest-pinned own scripts name the entry path. |
| c73ffa25 | c73ffa25 | 1a379447 | single epic | `E/c73ffa25` | moved | |
| f8dd4dde | f8dd4dde, 00dd43c3 | 1a379447 | single epic | `E/f8dd4dde` | left | `msrv-validation/Cargo.toml:23` holds a depth-dependent `path` dependency on `crates/gf2-core`; `validate-msrv.sh:11,13` names the entry path. |
| fcb04d66 | fcb04d66 | 1a379447 | single epic | `E/fcb04d66` | moved | |

The seven moved entries hold no script; REQ-06 applies to no file.

Branches `worktree-agent-ad2a6a58`, `worktree-agent-f63a2464` and
`worktree-agent-4c1e441f` list commits on 613574db, a203a23c and
c04dd4ac-zen3-shifts-and-permutations under `git log main..<branch>`;
`git cherry main <branch>` marks each of those commits patch-equivalent to
main, so they hold no path.

## Left in place, reported for decoupling

Paths without a `dev/` prefix are relative to the entry. "Runtime root" is the
one configured repository root of `no-dev-path-coupling`.

| Entry | Consumer | Kind | Decoupled form under `no-dev-path-coupling` |
|---|---|---|---|
| 00dd43c3 | `dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/make-publication-tables.py:326,341,356,357` | script | Locates the four 00dd43c3 artifacts through the runtime root or by content identity. |
| 00dd43c3 | `survey/freeze-confirmation.py:11`, `survey/freeze-pilot.py:10`, `survey/make-plan.py:7-8`, `survey/make-producing-inputs.py:14`, `survey/run-campaign.sh:22` | digest-pinned script, own-path literal | Derives the entry directory from the script's own location. |
| 00dd43c3 | `survey/test_pilot_freeze.py:11,13,42` | test, depth-dependent path | Resolves the repository root at runtime and the entry directory from the test's own location. |
| 19513245 | `survey/consumer/Cargo.toml:25-27`, `survey/field-laws/Cargo.toml:18,20` | digest-pinned config, depth-dependent path | Declares its `path` dependencies in a depth-independent form. |
| 19513245 | `survey/make-producing-inputs.py:23`, `survey/make-source-evidence.py:27` | digest-pinned script, own-path literal | Derives the entry directory from the script's own location. |
| 19513245 | `survey/run-profile.sh:30` | script, depth-dependent path | Resolves the repository root at runtime. |
| a203a23c | `timing/measure.py:99`, read at `:314` | script, own-path literal read at recorded revisions | Identifies the restore tool by content identity at each revision. |
| bc091474 | `dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/make-logical-producing-inputs.py:42,83` | digest-pinned script | Locates `run-candidate.sh` and `portfolio.md` through the runtime root or by content identity. |
| bc091474 | `run-candidate.sh:8,100` | digest-pinned script, depth-dependent path and own-path literal | Resolves the repository root at runtime and the entry directory from the script's own location. |
| bc091474 | `make-candidate-tables.py:10-11` | script, depth-dependent path | Resolves the repository root at runtime and the entry directory from the script's own location. |
| bdc507a3 | `dev/active/f547c394/amendment-v4.md:27` | digest-pinned relative link | None in the file: the link `../bdc507a3/verdict-preservation.json` resolves while bdc507a3 and f547c394 share a parent directory, so the two entries share a directory dependency and move in one change. |
| c04dd4ac-zen3-shifts-and-permutations | `dev/active/eda07788/survey/gf2-side/tests/profile_addendum.rs:17,19,21,251` | test | Locates the addendum, producing inputs, cases and plan tool through the runtime root. |
| c04dd4ac-zen3-shifts-and-permutations | `dev/active/00dd43c3/survey/make-producing-inputs.py:56`, `dev/active/00dd43c3/survey/run-campaign.sh:48` | digest-pinned script | Locates `find-shift-executable.py` through the runtime root or by content identity. |
| c04dd4ac-zen3-shifts-and-permutations | `survey/build-dvb-harness.sh:6,43`, `survey/freeze-shift-addendum.py:9`, `survey/inspect-shift-consumers.py:10,14`, `survey/make-dvb-addendum.py:18,21`, `survey/make-producing-inputs.py:7-8,61`, `survey/make-shift-plan.py:12,16`, `survey/make-shift-producing-inputs.py:9`, `survey/make-source-evidence.py:8,10`, `survey/run-dvb-campaign.sh:12,18-22`, `survey/run-profile.sh:12`, `survey/run-shift-profile.sh:24` | digest-pinned script, depth-dependent path or own-path literal | Resolves the repository root at runtime and the entry directory from the script's own location. |
| c04dd4ac-zen3-shifts-and-permutations | `survey/make-profile-provenance.py:151`, `survey/make-publication-tables.py:17`, `survey/make-shift-source-evidence.py:15`, `survey/make-voided-attempt.py:14`, `survey/smoke-dvb-arms.sh:10` | script, depth-dependent path | Resolves the repository root at runtime. |
| f8dd4dde | `msrv-validation/Cargo.toml:23` | config, depth-dependent path | Declares its `gf2-core` dependency in a depth-independent form. |

Own-path literals in scripts that are neither digest-pinned nor read at a
recorded revision take path text edits at the move and are not listed.

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

The first command prints no row of the seven moved entries. Run at the parent of
move commits `eb481bd02` and `4ff883a17`, the second prints, for each moved entry, only Markdown
citations and `progress.json` strings. It matches root-relative path text;
a consumer that reaches an entry by a relative path is outside it.

## Citations left at the flat path

| File | Reason |
|---|---|
| `E/progress.json:1423` | Machine state of the 1a379447 execution lead. |

Receipt snapshot `inputs/` trees and `dev/archive/` hold no citation of the seven
moved entries.

## Link scan

`python3 contrib/gates/docs-mechanical.py` prints
`docs-mechanical: PASS: 0 finding(s), 1 baselined; 384 Markdown files, 736 Rust sources, 133 item references`.
Its `markdown` footprint `dev/active/**/*.md` contains every Markdown file the
4f161183 commits touch or move. It checks inline and reference links and their
anchors; it does not check code-span path citations under `dev/active/`. The
baselined row is `E/9f0e385e/coverage-addressability.md:19`.

## Tracker references

`target/4f161183-relink.sh` (uncommitted) repoints the 18 document references
of issues 428f2f6b, 50f0bd42, 613574db, 9f0e385e, 9fb40c83, a1ad6d4e, c73ffa25
and fcb04d66.
