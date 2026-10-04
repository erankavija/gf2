# Re-archive of epic 97bf0879 (83f9ce69)

Container `97bf0879-357e-4151-bdeb-781987cd0902`, archive directory
`dev/archive/97bf0879-gf2-core-sota-performance`. Execution commit on main:
`92756ce0a`. Verification base: main `0d16a2be5`.

| File | Content |
|---|---|
| `83f9ce69-preexec-plan.json` | Fields of the `jit archive container 97bf0879 --json` preview taken on main immediately before execution: per artifact the source, action, destination, content identity, evidence, reference changes and deletions |
| `83f9ce69-rearchive.py` | Verification script; its docstring states each check |
| `83f9ce69-rearchive.txt` | Output of the script at this commit |

## REQ-01: preview before execution

| Field | Value |
|---|---|
| `eligible` | true |
| `destination_root` | `dev/archive/97bf0879-gf2-core-sota-performance`, marker present |
| Blockers, plan level and per artifact | 0 |
| Artifacts | 90: 83 move, 7 retain |
| Moves with `already_archived` | 83 |
| Deletions | 1: `dev/bench_results/2026-04-29-7c954fb5-criterion.txt` |
| Reference changes listed | 91 |

The seven retains: three files outside the development root
(`outside-development-root`), and four entries for three files that tracker
references pin to a commit (`pinned-historical`).

## REQ-02: execution and marker

`jit archive container 97bf0879 --execute` on main, commit `92756ce0a`: one
publication adopted
(`bench_results/2026-05-04-0fd48627-gf2-m4ri-profile.md`), 15 reference changes
on the container issue, one source deleted (the file under REQ-01).
`.jit-container` holds `97bf0879-357e-4151-bdeb-781987cd0902` (script section
`marker`).

## REQ-03: bytes and tracker references

| Check (script section) | Result |
|---|---|
| `hashes`: sha256 and byte size of each planned destination equal the planned content identity | 83 of 83 |
| `hashes`: planned move source absent | 83 of 83 |
| `references`: tracker document at each planned (issue, index) names the planned archive path | 91 of 91, on 33 issues |

The hash check compares against the identity recorded before execution. It
does not compare against a pre-archive source file: no source of the 83 moves
exists.

## REQ-04: citations

Scan: `git grep -F` for each of the 83 move sources and the deleted source,
over tracked files outside `dev/archive/`, `.jit/` and receipt `inputs/` trees.

Repointed in commit `fb3132b8e` (comment and docstring path text only):

| File:line | Target |
|---|---|
| `benchmarks/analyze.py:21,22,27` | `dev/archive/97bf0879-gf2-core-sota-performance/bench_results/2026-04-26-{gf2,reference}.csv` |
| `benchmarks/analyze.py:294`, `benchmarks/reference/ntl_bench.cpp:37,65` | `dev/archive/026fc832-gf2-core-sota-stretch/bench_results/b13799ac/2026-05-04-b13799ac-gf2pow32-promotion.md` |
| `benchmarks/reference/fflas_sparse_bench.cpp:5`, `linbox_sparse_bench.cpp:4,264,479`, `sparse_smoke.cpp:4,660,828,1464` | `dev/archive/97bf0879-gf2-core-sota-performance/plans/sparse_benchmark_corpus.md` |
| `benchmarks/reference/ntl_bench.cpp:411` | `dev/archive/97bf0879-gf2-core-sota-performance/plans/gf2m_reference_lane_selection.md` |
| `dev/bench_results/run_41096af5_post_wire_in_bench.sh:187` | `dev/archive/97bf0879-gf2-core-sota-performance/bench_results/2026-04-26-reference.csv` |

`dev/scripts/check-receipt-input-snapshots.py` prints the same output before
and after these edits.

Citations of a move source that this unit leaves in place:

| File:line | Cited source | Reason |
|---|---|---|
| `perf-evidence-catalog.md:126,197` | `dev/bench_results/2026-04-29-strassen-matmul-crossover.md` | Manifest destination is the babcf05e archive; unit 3759f995 |
| `67048b47-linked-pairs.txt:260,501,629,631,632,911,914,994` | eight sources whose manifest destination is the babcf05e archive | Committed script output; unit 3759f995 |
| `migration/manifest.toml`, 8 rows | the same eight sources | `path` fields of manifest rows |
| Files under `dev/bench_results/` and `dev/plans/` | several | Receipts and commit-pinned plans keep their bytes |

No README, `AGENTS.md` or Rust source cites a move source or the deleted
source.

## REQ-05: rerun, links, manifest

| Check | Result |
|---|---|
| `jit archive container 97bf0879 --execute --json` on main ([archive rerun results](archive-rerun-results.md)) | 0 publications, 0 reference changes, 0 deleted sources, no event appended |
| Preview at main `0d16a2be5` (script section `rerun`) | eligible, 0 blockers, 83 moves all `already_archived`, 0 move sources present, 0 pending deletions, 7 retains |
| Archive directory, Markdown inline links and HTML `href`/`src` (script section `links`) | 60 local links, 0 unresolved |
| Six repointed files, `dev/archive/` path tokens (script section `links`) | 16, 0 unresolved |
| Manifest rows with `epic = "97bf0879"` (script section `manifest`) | 5 of 5 `complete` |
| `python3 dev/active/fa787f85-documentation-overhaul/migration/check.py` | exit 0 |

The preview lists 91 reference changes whether or not the tracker already
names the target; all 91 are in the set verified under REQ-03. The link scan
does not check anchors, URLs, fenced code or backticked paths in Markdown.

The execution deletes one source and adopts one publication; neither source
has a manifest row. The archive plan retains three files of this epic in
place, because tracker references pin them to a commit (`pinned-historical`).
Their rows record that state: disposition `retained-operational`, empty
destination, status `complete`.

| Row `path` | Pinned commit | Retaining references (issue, document index) |
|---|---|---|
| `dev/bench_results/2026-05-06-7a106fe4-gfp-parity-evidence.md` | `11be30fe6f32` | 7a106fe4 0, 97bf0879 7, cc5de315 1 |
| same file | `4cde8a8f93c9` | cc5de315 2 |
| `dev/plans/small_prime_kernel_strategy.md` | `3f62600971b0` | 5cacaec5 0, 662f7a15 0, 97bf0879 6, b9aed0d8 0 |
| `dev/plans/sota_target_matrix.md` | `47e452544b04` | 4c0d0202 0, 97bf0879 5, cbecfced 3 |

The other two rows of this epic are `deletion` rows of absent
`dev/active/` directories.

## Commands

```
python3 dev/active/fa787f85-documentation-overhaul/83f9ce69-rearchive.py
python3 contrib/gates/docs-mechanical.py
```
