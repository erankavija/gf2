# Re-archive of epics babcf05e and f9717e7e

Issue 3759f995. Container archival of both epics is executed in commit
`aac49881e`. `BA` is `dev/archive/babcf05e-gf2-core-ppc-spiral`, `FA` is
`dev/archive/f9717e7e-gf2-sim`.

## Evidence files

| File | Content |
|---|---|
| `3759f995-preexec.json` | Per epic, the `jit archive container <epic> --json` preview taken immediately before execution, reduced to target, destination root, eligibility, blockers, action counts and per artifact source, action, destination, sha256 and deleted sources. |
| `3759f995-verify.py` | Produces the report; `--reduce` produces `3759f995-preexec.json` from the raw previews. |
| `3759f995-verify.txt` | Report of `python3 dev/active/fa787f85-documentation-overhaul/3759f995-verify.py`, at the commit that last changes it. The script takes the repository root from git, the archive roots from the live previews, the expected files and hashes from `3759f995-preexec.json`, the scanned files from the commits tagged `jit:3759f995`, and the manifest rows from the destinations of the pre-execution plans. |

## Criteria

| Criterion | Evidence in `3759f995-verify.txt` | The check establishes | The check does not establish |
|---|---|---|---|
| REQ-01 | `pre-execution preview` lines: both epics `eligible=True`, `blockers=0`, destination roots `BA` and `FA` | The preview before execution is eligible, without plan-level or artifact-level blocker, into the existing directory. | That the reduced file equals the raw preview; the raw JSON is not committed. |
| REQ-02 | `marker` lines | `BA/.jit-container` names `babcf05e-29e8-4c81-ba77-843ec6409cfa`; `FA/.jit-container` names `f9717e7e-3950-422c-a545-d940d119a440`. | – |
| REQ-03 | `byte verification`: 26/26 and 16/16 destinations, 7/7 and 1/1 deleted sources; `tracker references`: 57/57 and 25/25 | Each destination of the pre-execution plan has the sha256 the plan records; each source the plan deletes is absent; each document reference the plan relinks names its archive path in the live tracker. | Anything about `crates/gf2-core/src/sparse.asm.txt`, which the babcf05e plan retains in place without a destination. |
| REQ-04 | Table "Citations" below | – | – |
| REQ-05 | `archive-rerun-results.md`, rows babcf05e and f9717e7e: 0 publications, 0 reference changes, 0 deleted sources; `fresh preview` lines: 0 artifacts left to move, copy or delete; `link scan` lines: 0 unresolved; `manifest rows`: 11 rows `complete` | A second `--execute` of each epic changes nothing; a preview of each archived epic plans no publication and no deletion; no inline Markdown link with a local target is unresolved in a Markdown file of `BA` or `FA` or in a Markdown file the commits tagged `jit:3759f995` touch; each manifest row whose destination the plans place is complete. | Reference-style links, HTML links and links in non-Markdown files. |

## Files placed and deleted by the execution

| Epic | Published | Adopted (bytes present) | Sources deleted |
|---|---|---|---|
| babcf05e | `BA/bench_results/2026-04-27-asm-audit.md` | 1 | 7 |
| f9717e7e | `FA/benchmarks/gf2-sim/README.md` | 2 | 1 |

`dev/benchmarks/gf2-sim/comparison/README.md` is copied, not moved: issue
3be770d5 of open epic d77176e5 also owns it, and its source stays.

## Blocker resolved before execution

Issue babcf05e held two document references mapping to
`BA/active/babcf05e-handoff-5.md`, which the preview reports as two
`destination-conflict` blockers. The reference to the byte-identical copy
`dev/active/babcf05e-gf2-core-ppc-spiral/babcf05e-handoff-5.md` is removed and
that copy is deleted with `git rm`; the reference of type `session` labelled
"Session handoff 5" names the archive file.

babcf05e executes before 97bf0879 and 026fc832: both of those plans delete
`dev/bench_results/2026-04-27-asm-audit.md` as an embedded artifact, and the
babcf05e plan needs that source for its one publication.

## Citations

| File | Line | Target |
|---|---|---|
| `perf-evidence-catalog.md` | 124 | `BA/bench_results/2026-04-29-2598b981-fieldmatrix-gemm-fflas-sweep.md` |
| `perf-evidence-catalog.md` | 126, 197 | `BA/bench_results/2026-04-29-strassen-matmul-crossover.md` |
| `investigation.md` | 235 | `BA/active/babcf05e-handoff-5.md` |
| `036615b0-inventory-notes.md` | 5 | `BA/active/babcf05e-handoff-5.md`; the byte-identical-copy clause covers the first listed file only |
| `investigation.md` | 52 | The handoff leaves the list of files in `dev/active`. |

`git grep -F` for the nine source paths and their file names finds no citation
in a README, `AGENTS.md` or Rust source outside `dev/archive/` and receipt
`inputs/` trees.

Citations that keep their bytes:

| Location | Reason |
|---|---|
| `67048b47-linked-pairs.txt`, nine lines | Committed output of `67048b47-doc-links.py`. |
| `migration/manifest.toml`, `path` fields | The field names the source by schema. |
| `dev/bench_results/1d0da41f/*/inputs/producing/crates/gf2-core/examples/lto_opacity_audit.rs:22`, four files | Receipt snapshot copies. |

## Manifest

The report lists eleven complete rows: the nine this unit completes and the two `gruvbox.css` rows of the same archives.

Row `dev/benchmarks/gf2-sim/dvb-t2-regression-receipts.md` is corrected and is
no row of this unit:

| Field | Value | Evidence |
|---|---|---|
| `epic` | `d77176e5` | `jit graph tree d77176e5` resolves owner 0d9cb8e3 to parent 5d0a3fad and 5d0a3fad to parent d77176e5 (state backlog); the f9717e7e archive plan does not contain the file. |
| `disposition` | `retained-operational` | The checker requires a `dev/archive/` destination for `jit-container-archive`; the sibling row `dev/benchmarks/gf2-sim/comparison/README.md` of the same epic has this disposition. |
| `destination` | `dev/active/d77176e5-competitive-benchmarking/benchmarks/gf2-sim/dvb-t2-regression-receipts.md` | `jit doc dir d77176e5 dev/active` resolves `dev/active/d77176e5-competitive-benchmarking`. |
| `status` | unchanged | The file is at its `path`. |
