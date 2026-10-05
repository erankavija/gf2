# dev/plans removal (ec655592)

Commits: `65acf0aae` (moves, citations), `c2ad54d52` (citations in four
files), `e34089ad0` (manifest rows, archived stub).

## Moved files

`git ls-files dev/plans` prints nothing. Every move is a `git mv`.

| Source under `dev/plans/` | Owner (tracker state) | Destination |
|---|---|---|
| `16283d6f-fieldmatrix-gpu/gpu_fieldmatrix_sketch.md` | 16283d6f (ready) | `dev/active/16283d6f-fieldmatrix-gpu/plans/` |
| `3eeb57f6/m4rie_promotion_evidence.md` | 3eeb57f6 (ready) | `dev/active/3eeb57f6/plans/` |
| `ae03bcd0-general-bch/planning-brief.md` | ae03bcd0 (backlog) | `dev/active/ae03bcd0-general-bch/plans/` |
| `field_poly_module_overview.md` | none | `dev/archive/legacy/dev/plans/` |
| `flint_promotion_evidence.md`, `ntl_promotion_evidence.md` | 73ab8eef, cbecfced (done; subtree of 026fc832, archived) | `dev/archive/026fc832-gf2-core-sota-stretch/plans/` |
| `small_prime_kernel_strategy.md`, `sota_target_matrix.md` | 97bf0879 (archived); 5cacaec5, 662f7a15, b9aed0d8, 4c0d0202, cbecfced (done; subtree of 026fc832) | `dev/archive/97bf0879-gf2-core-sota-performance/plans/` and a byte-identical copy in `dev/archive/026fc832-gf2-core-sota-stretch/plans/` |

Ownership of the terminal rows is the `owners[].inside_subtree` field of
`jit archive container <epic> --json`: the 97bf0879 plan lists the last two
files only, the 026fc832 plan lists all four. The first three directories are
the output of `jit doc dir <owner> dev/active` plus the manifest's `plans/`.

`dev/active/3eeb57f6/plans/m4rie_promotion_evidence.md` and
`dev/archive/026fc832-gf2-core-sota-stretch/plans/m4rie_promotion_evidence.md`
have equal SHA-256: the first belongs to the open owner 3eeb57f6, the second
to cbecfced and 507b0036.

The moved files keep their bytes. `.jit/config.toml` and
`packages/jit-default/manifest.toml` list `dev/plans` among the managed and
issue-scoped paths; this unit leaves both files unchanged.

## Citations (REQ-03)

Scan: `git grep -nE 'dev/plans|\.\./plans/|plans/[a-z0-9_]+\.md'`, then
`git grep -nF` of the SHA-256 and blob id of each citing file outside `.jit/`
(no occurrence for any edited file).

| File:line | Change |
|---|---|
| `benchmarks/README.md:168`, `benchmarks/run.sh:308`, `benchmarks/smoke.sh:114`, `benchmarks/reference/{charpoly_minpoly_smoke.cpp:8,10; fflas_bench.cpp:33; m4ri_bench.c:23,35,315,548; m4rie_bench.c:29,47; ntl_bench.cpp:19; ntl_flint_smoke.cpp:4}` | path text to `dev/archive/026fc832-gf2-core-sota-stretch/plans/{sota_reference_acceptance_protocol,ntl_promotion_evidence,m4rie_promotion_evidence}.md` |
| `proofs/Gf2Algebra/Proofs/{Bipedal3Correctness.lean:5, Packed5Correctness.lean:5, Packed7Correctness.lean:4, RyserBounded.lean:7,108,109}`, `proofs/WORKAROUNDS.md:136`, `scripts/fix-aeneas-gf2algebra.py:4,166,224`, `scripts/verify-lean.sh:141,143,145` | path text to the sketches under `dev/archive/ae82bd73-gf2-algebra-permanent/plans/` |
| `scripts/asm-artefact-present.sh:4,104` | path text to `dev/archive/babcf05e-gf2-core-ppc-spiral/plans/gf2_core_ppc_spiral.md` |
| `dev/research/perm_uniformity_gpu/README.md:64`, `dev/research/permanent_gpu_crossover/README.md:13,15` | path text to the writeups under `dev/archive/ae82bd73-gf2-algebra-permanent/plans/` |
| the archived packed-field research stub, `src/lib.rs:10,1067` | path text to the design records under `dev/archive/ae82bd73-gf2-algebra-permanent/plans/` |
| `dev/research/simd_batching_bench/src/{lib.rs,generic.rs}` module docs, `dev/research/permanent_gpu_crossover/src/main.rs` (`N_VALUES` doc) | citation removed |
| `dev/research/{f5,f7}_packing/src/main.rs:130` | the closing printed sentence, which names the encoding decision, is removed; the other printed lines are unchanged |
| `dev/active/4e732b56/findings.md:34`, `dev/active/ae03bcd0-general-bch/plan.md:12,436` | Markdown link targets |
| `dev/active/ae03bcd0-general-bch/investigation.md:5,182,187,205,463,485`, `dev/active/6dc81018-field-capability-dispatch/investigation.md:514,515,531,534`, `dev/active/b8206228-permanent-statistics/0de41c82/investigation.md:404` | path text of the moved files |

No deleted comment sits between two nested `if`s.

Remaining citations of a `dev/plans/` path absent from the tree, with the ground for each:

| Location | Ground |
|---|---|
| `packages/jit-default/manifest.toml:353,397` | path-policy values, not plan citations |
| `dev/bench_results/` | receipts, snapshot `inputs/` trees and reports of unit b2bb1065 |
| `dev/archive/` other than the stub | historical documents inside archives; the moved plans cite each other by their `dev/plans/` paths and keep their bytes |
| `dev/active/16283d6f-fieldmatrix-gpu/plans/gpu_fieldmatrix_sketch.md:91`, `dev/active/3eeb57f6/plans/m4rie_promotion_evidence.md:15,31,328` | moved files keep their bytes |
| `dev/active/b8206228-permanent-statistics/0de41c82/investigation.md:289,521,522,807,862`, `dev/active/b8206228-permanent-statistics/progress.json:1030` | sentences that quote the stale citations of Rust sources at the investigated checkout |
| `dev/active/fa787f85-documentation-overhaul/` | manifest `path` keys, checker fixtures, and records, plans and handoffs of this epic that name the pre-move location |

## Manifest rows

| Row `path` | Fields changed |
|---|---|
| the three `dev/active` destinations, `field_poly_module_overview.md` | `status = complete` |
| `flint_promotion_evidence.md`, `ntl_promotion_evidence.md` | `destination` = the 026fc832 archive path (row `epic = 026fc832`), `status = complete` |
| `small_prime_kernel_strategy.md`, `sota_target_matrix.md` | `disposition = jit-container-archive`, `destination` = the 97bf0879 archive path |

## Tracker references

`target/ec655592-relink.sh` (uncommitted) repoints the unpinned references of
16283d6f, 3eeb57f6 and ae03bcd0 to the `dev/active` destinations and rescans
two linked documents whose Markdown links change. Each commit-pinned reference
keeps its `dev/plans/` path and its pin, which name the bytes the issue relied
on.

| Pinned path | Pin | Issues |
|---|---|---|
| `dev/plans/sota_target_matrix.md` | `47e4525` | 4c0d0202, 97bf0879, cbecfced |
| `dev/plans/small_prime_kernel_strategy.md` | `3f62600` | 5cacaec5, 662f7a15, 97bf0879, b9aed0d8 |
| `dev/plans/flint_promotion_evidence.md`, `dev/plans/ntl_promotion_evidence.md` | `5871eccf` | 73ab8eef, cbecfced |

## Verification

| Command | Result |
|---|---|
| `python3 contrib/gates/docs-mechanical.py` | PASS, 0 findings |
| `sed 's/jit:103a792a/jit:ec655592/' dev/active/fa787f85-documentation-overhaul/103a792a-link-scan.py \| python3 -` | 0 unresolved inline links over the Markdown files the `jit:ec655592` commits touch; anchors, fenced code and backticked paths are outside the scan |
| `python3 dev/active/fa787f85-documentation-overhaul/migration/check.py` | no finding for a `dev/plans/` row; exit 1 on `dev/active/*/survey/` rows of other units |
| `./scripts/cargo-budget.sh cargo fmt --all -- --check`; `rustfmt --check` on the edited sources outside the workspace | clean |

No build or test ran in this unit; `cargo-ci` runs on main.
