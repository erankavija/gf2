# Re-archive of epic 026fc832 (f29a9225)

`A` = `dev/archive/026fc832-gf2-core-sota-stretch`. Container archival ran on main in
`b977e603b`, after babcf05e and f9717e7e (`aac49881e`) and 97bf0879 (`92756ce0a`).

Evidence script: `python3 dev/active/fa787f85-documentation-overhaul/f29a9225-verify.py`,
output in [f29a9225-verify.txt](f29a9225-verify.txt). It finds `A` by its marker and reads the
sources this unit removed from history: every path a `jit:f29a9225` commit deletes or renames
away. [f29a9225-identities.tsv](f29a9225-identities.tsv) is the `--extract` projection of the
`jit archive container 026fc832 --json` preview on this tree: destination, sha256 and byte size
per planned move.

## Criteria

| Criterion | Evidence | Establishes | Does not establish |
|---|---|---|---|
| REQ-01 | Preview before execution: `eligible = true`, no blocker, `destination_root = A`, 105 moves (97 already archived, 8 from `dev/bench_results/`), 0 copies, 14 retains | the plan is eligible into the marker-backed directory | — |
| REQ-02 | Execution result: 9 publications, 23 reference changes, 10 deleted sources, 1 `deletion-failed`; verify line 1: one archive directory whose `.jit-container` names `026fc832-a480-4d07-8d25-47b8bfcb69a3` | the archive ran through the mechanism and the marker names the full identifier | — |
| REQ-03 | verify line 3: 15 removed sources (8 bench reports, 7 Route B crate files); 0 mirrors under `A` differ from the git blob of the last source version, 0 mirrors lack a tracker reference, 0 tracker references name a removed source | the archive holds the last bytes of each source this unit removed, and `.jit/issues/` references them at their `A` path | identity of the 14 retained artifacts, which stay in place |
| REQ-04 | Citation table below | each in-scope citation of a moved path names the `A` path | citations in bench reports and archived artifacts, which keep their bytes |
| REQ-05 | Preview on this tree: 109 moves, 109 already archived, 0 `pending_deletions`, 0 blockers; verify line 2: the 109 planned destinations carry the planned sha256 and size; verify lines 4-5: 0 unresolved local links in 82 archive files and in the 3 Markdown files this unit edited; rerun rows of 026fc832 in [archive-rerun-results.md](archive-rerun-results.md); manifest table below | the plan has nothing to move, copy or delete; inline Markdown links resolve | the preview does not exercise `--execute`; the scan covers inline links, not code-span paths |

## Route B crate

Execution deleted `dev/research/blas_sgemm_gf251/src/blas_ffi.rs` and `tests/bit_exact.rs`
(bytes equal to the `A` copies) and skipped `src/lib.rs`, whose bytes differed from the `A` copy
by the comment hunks of `839e33a02` and `c1203721e`.

| File | Bytes under `A/research/blas_sgemm_gf251/` |
|---|---|
| `Cargo.toml`, `build.rs`, `.gitignore`, `src/bin/bench_blas_gf251.rs` | moved by `git mv` in `839ccb113`; blobs equal the `dev/research` versions |
| `src/lib.rs` | blob `0dbba77ad7cc6b2cfbc7f5aeaab399b07cf85e73`, sha256 `d025117b811851a61fd23367275f174243b7b2cbac5a9046a92cb70bf57f3cb8`, 23374 bytes: equal to `dev/research/blas_sgemm_gf251/src/lib.rs` at `b977e603b`, the last version under `dev/research` |

Verify line 3 checks each of the seven crate files against the blob of its last `dev/research`
version. The copy of `src/lib.rs` that the first archive event published (sha256 `7420b638...`,
blob `a98a585cec77`) is in git history at `839ccb113`. No other archive holds a copy of the file.

`dev/research/blas_sgemm_gf251/` is absent. The archived `Cargo.toml` keeps its path dependency
`../../../crates/gf2-core`, which does not resolve from `A`; the archived crate does not build
in place.

## Citations

| File:line | Target |
|---|---|
| `dev/active/7d7c647c/design.md:351`, `:435` | `A/bench_results/2026-05-27-68db401b-fp-medium-ple.md:30-31` |
| `dev/active/fa787f85-documentation-overhaul/investigation.md:161` | `A/bench_results/2026-05-27-8df0c501-blocked-invert.md:4` |

Scope scanned with `git grep -F` per moved path and basename over `README.md`, `AGENTS.md`,
`crates/`, `docs/` and `dev/active/`, excluding the manifest and `67048b47-linked-pairs.txt`
(script output at its recorded commit). No README, `AGENTS.md` or Rust source cites a moved path.

## Manifest rows

| Row | Change | Reason |
|---|---|---|
| 7 rows `dev/bench_results/2026-05-2{6,7}-*.md` (6613abf4, 869ce43b, 68db401b, 6a7d4c8e, 8df0c501, 9138d86c, d36cc414) | `status = complete` | source absent, destination present |
| `dev/bench_results/2026-05-06-662f7a15-f-vs-c-verification.md` | `destination` = its `A` path, `epic = 026fc832`, `status = complete` | 662f7a15 resolves under 026fc832 (`jit graph tree`: parent cc5de315, parent 026fc832); the mechanism placed the file under `A` |
| `benchmarks/README.md` | `epic = 026fc832`, `status = complete` | owner 5102d87a: parent b0434149, parent 026fc832; the plan retains the file in place |
| `dev/plans/flint_promotion_evidence.md`, `dev/plans/ntl_promotion_evidence.md` | `epic = 026fc832` | owners 73ab8eef and cbecfced: parent 026fc832. The plan retains both in place as `pinned-historical`; their `destination` and `status` are unchanged |

`python3 dev/active/fa787f85-documentation-overhaul/migration/check.py` reports no finding for
these 11 rows.
