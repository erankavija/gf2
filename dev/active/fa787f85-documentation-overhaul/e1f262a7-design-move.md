# e1f262a7 rare-event design move

Move commit: `deda5bf29`. Baseline record: [11964cb8-baseline.md](11964cb8-baseline.md).

## Move

| Item | Value |
| --- | --- |
| Source | `dev/active/3f664839/design.md` |
| Destination | `dev/active/b8206228-permanent-statistics/3f664839/design.md` |
| Directory check | `jit doc dir b8206228 dev/active` prints `dev/active/b8206228-permanent-statistics` |
| Bytes | `git diff -M --stat 4dfbab9ce HEAD -- dev/active` reports the design as a pure rename |
| Manifest row | `dev/active/3f664839/design.md`, `status = "complete"`; `check.py` reports no finding for it |
| Rust files changed | none |
| Entry scripts | none; the entry holds only `design.md` |
| Own-path literal in the entry | `design.md:517` cites the entry's own path as prose; the file is a preregistration whose bytes `plan.md:69-71` (`relocation-protocol`) fixes, and its manifest row is `digest_pinned = true`; the line is kept |

## Consumers

| Check | Command | Establishes |
| --- | --- | --- |
| No digest or blob pin | `git grep -nF "$(sha256sum < FILE \| cut -d' ' -f1)"`, same with `git hash-object FILE`, over the whole tree including `crates/` and `.jit` | no tracked file pins the bytes of the design, of `handoff-22.md` or of the two `f547c394` sweep files |
| Code citations | `git grep -n '3f664839' -- crates dev/scripts scripts dev/tools` | one hit, the test fixture below |
| Validator | `crates/gf2-sim/src/permanent_rare_event/artifact.rs:98` documents `path` as informational; `validate_design` (`artifact.rs:4219`) checks revision, blob and SHA-256 only | validation never compares `path` with a filesystem path |
| Design identity | `validate_design` (`artifact.rs:4219`) checks a design's revision, blob and SHA-256 | an artifact embeds the blob of this file, so its bytes stay |
| Path as data | `artifact.rs:4504` compares the whole `DesignIdentityV1` value for equality; the value is serialized into the configuration digest | the field feeds identity equality and a digest as an opaque string |

| Literal | Ground | Evidence |
| --- | --- | --- |
| `crates/gf2-sim/tests/permanent_rare_event_artifacts.rs:55` `"dev/active/3f664839/design.md"` | `path` field value in a test fixture; no code opens it | `rare_event_design_identity_is_path_independent` (`:1262`) round-trips the identity with an unrelated path; the suite passes after the move |

## Citations

`git grep -nF 'dev/active/3f664839'` and `git grep -nE '(\.\./)+(active/)?3f664839/'` outside `.jit/`:

| Hit | Action | Ground |
| --- | --- | --- |
| `b8206228-permanent-statistics/handoff-22.md:20,79` | repointed | unpinned handoff |
| `fa787f85-documentation-overhaul/dev-path-code-consumers.md:99` | heading repointed | unpinned survey |
| `3f664839/design.md:517` | kept | preregistration whose bytes `plan.md:69-71` (`relocation-protocol`) fixes; manifest row `digest_pinned = true` |
| `f547c394/research-r3-sweeps.json` (`:3001,4968,4973`), `research-r3-sweep-resolutions.md` (`:588,963,964`) | kept | sweep output measured at reviewed commit `259c1b9a` (`f547c394/findings.md:65`), cited as record in `decoupling-findings.md:352` |
| `dev/bench_results/*/inputs/producing/**` | kept | receipt `inputs/` snapshots |
| `migration/manifest.toml` | kept | manifest schema fields |
| `fa787f85-documentation-overhaul/breakdown.json:3589,3634,3674` | kept | file left by assignment |
| `fa787f85-documentation-overhaul/plan-review-r2.md:124` | kept | quotes `artifact.rs` at the reviewed commit; the sentence describes that code |

## Verification

| Check | Result |
| --- | --- |
| Artifact-validation repeat | appended to `11964cb8-baseline.md`: 26 of 26 tests pass at `deda5bf29`, identical to `d13d03f83` |
| Link scan | `python3 contrib/gates/docs-mechanical.py` reports 0 findings |
| Manifest | `python3 dev/active/fa787f85-documentation-overhaul/migration/check.py` reports no finding for this unit's row and exits 1 on rows of other units; the first line is `dev/active/3be770d5/survey/harness/src/bin/ldpc-plan-check.rs: pending source is missing` |
| Unmerged branch work | `git log --oneline main..BRANCH -- dev/active/3f664839 dev/active/b8206228-permanent-statistics` is empty for every branch of `git worktree list` |

## Tracker

Reference to the old path: issue `3f664839`, type `design`, label `Preregistered rare-event design`, no `commit` pin. It is relinked by `target/e1f262a7-relink.sh` (uncommitted). The script also rescans `handoff-22.md` (issue `b8206228`) and `dev-path-code-consumers.md` (issue `fa787f85`).
