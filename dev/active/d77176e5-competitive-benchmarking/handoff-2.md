# Handoff — Competitive benchmarking against Magma/Sage/AFF3CT (d77176e5) — session 2

**Date:** 2026-11-10
**Prior handoffs:** `dev/active/d77176e5-competitive-benchmarking/handoff.md`. Related: `dev/active/b4b4b9ee-tech-debt-2026-06-30/handoff-2.md`.

## Current state

- Main is `6c694722f`, plus the commit that adds this file. Nothing is pushed. All worker sessions are idle or signed off; nothing is running on the host.
- Night window: user timer `gf2-bench-window-20261111a` fires at 02:00 local on 2026-11-11 with `GF2_WINDOW_MAX_EST_MIN=0`. It detaches the four run worktrees (`agent-de311215`, `jit-be6d6f45-run`, `jit-befccab3-run`, `jit-c9cb0dd2-run`) at main and runs the decode table, route profiles, BCH encode, gf2x and GF(2^8) lines. The NR encode line is marked done and is skipped.
- Done today with all gates passed: `a57cb8d0`, `bc4eb125`, `1ffa72fc`, `39cbde20`, `a44566c0`, `01877f76`, `be6d6f45`, `c62356d2`, `7a5ee7c7`, `1015ae73`, `07fe99b8`, `be113cbf`.
- Rejected by owner decision: `c9022d14`, `de3ec390`, `10a729d3`.
- In progress, code merged, review findings open: `040bc2b8`, `c9cb0dd2`, `259acf37`, `07a1813a`; `069e3c33` (claim released, state ready) and `8a8a3bc9` (doc-review failed).
- In progress, code merged, review not run: `befccab3`, `de311215`.
- `4f21da96` is WIP on branch `jit-4f21da96` at `309ab9985`, unmerged, blocked (see open questions). Its claim is recorded inside that commit only.
- Disk: 292 GB free. Five worktrees remain besides the main checkout: the four run worktrees and `jit-4f21da96`. Every other worktree was removed after a per-worktree check; their branches remain, except the superseded `jit-befccab3-k8` and `jit-c62356d2-bounds`, which were deleted.
- `cargo-ci` gate: passes on a warm build; a cold run after merges exceeds its 900 s timeout.

## What just happened

- Window 4 was stopped after 35 minutes (owner decision) and replaced by short windows. Stated results recorded in the issues:
  - NR rate-matched encode against srsRAN AVX2: 3.19 to 4.82 (`be6d6f45`).
  - GF(2^8) against ISA-L and M4RIE: axpy 0.99 to 1.29, matrix product 1.22 to 26.1; generic `Gf256` at 0.85 to 1.25 of the byte API (`c9cb0dd2`, `be113cbf`).
  - Long carry-less product against gf2x 1.3.0: 1.09 to 1.62 in every cell after vectorized Karatsuba passes, a 16-word node, leaf-aligned splits and a broadcast-form 9-word kernel (`befccab3`). The 12:59Z table overlapped another session's test build and is provisional until the night rerun.
- Falsified and recorded: Toom-3 for the 2048-word gap (`befccab3`); the BCH lead of 1.16 and 1.38, which was against bchlib's runtime build (`040bc2b8`). Against the const-parameter build gf2 ran at 0.69 and 0.77; the multi-accumulator fold and a cached packed generator bring the indication to 1.31 and 1.67.
- `be113cbf`: one `unsafe` byte-view module in `gf2-core` by owner decision; `unsafe-kernel-isolation` names it.
- `07a1813a`: polar butterfly on words, about 1500 to 3100 times faster than the bit loop on the criterion bench (indication).
- `de311215` REQ-04 rests on route tests over all 12 DVB-T2 codes; no bundles were generated.
- Process changes by owner decision: a window defers jobs estimated above 5 minutes unless the cap is lifted; the quiet-host wait uses load 4.0; the code-review prompt treats findings that only a history rewrite could repair as advisory; `259acf37` REQ-06 and `7a5ee7c7` REQ-03 were reworded.
- About 570 local branches exist; pruning the merged ones is not analysed.
- A dashboard of today's ratios is published privately at `https://claude.ai/artifact/4ZoceRZh2aYohYxaumogP7`.

## What to do next

1. After the night window: read `.agents/bench-window/window.log`, record the decode, profile, BCH, gf2x and GF(2^8) tables in their issues with host and commit, and update the dashboard's BCH and gf2x values.
2. `040bc2b8` review findings: F2 `FOLD_RUN_BLOCKS = 64` needs a structural derivation and an override; F3 the scalar-pinning advice in `docs/how-to/select-acceleration.md` is stale. F1 (queue measures old code) is answered by the night timer's checkout.
3. `8a8a3bc9` doc-review: the link-simulation tutorial shows results from before the cell-mapping fix; the `with_gpu` demapper sentence is wrong.
4. `c9cb0dd2` review findings, F1 first: `gf256_gemm` panics on valid zero-row or zero-column shapes across block boundaries. Then block-size derivation, the vacuous axpy checksum in the benchmark, stale rustdoc, the worktree-depth queue path. Lead: the byte API's 1 MiB axpy cell runs 25% below the generic route on the same kernel.
5. `259acf37` review findings: x86 macro without a target guard in the contract suite; FER tests over the slow-tier budget; the FER comparator passes without bracketing a crossing.
6. `069e3c33` review findings: a cache test expects twelve files; round-trip coverage misses most code families.
7. `07a1813a`: rerun `cargo-ci`, code-review and doc-review on main; the plain-loop change merged after the last `cargo-ci` verdict.
8. Run code-review for `befccab3` and `de311215`; both have recorded results and merged code.
9. `ccaed736` and `a33fda32`: plans approved (shared rank-derived parity-check helper; `WorkspaceEncoder` trait with rustdoc workflow example, one family selection, and a default `encode_into` that is not the slow path). No code exists.
10. Epic `d77176e5` keeps `12a3c312` and `f768c925` ready and untouched.

## Traps — do not repeat these

- `grep -c 'window end'` on the window log counts every earlier window. Wait on the last `window start` or `window end` line instead.
- A `cd` into a queue worktree inside a command moves the session's working directory there. Use `git -C` and absolute paths.
- The disk filled to 100% from per-worktree `target/debug` directories; two `cargo-ci` gate runs failed in the doc step with "No space left on device". Check `df` before blaming code.
- A pause request does not stop a build a worker already started. Ask workers to confirm idle before starting a window.
- `evaluate-many` prints verdicts only when the whole run ends; read `jit gate status <id> <gate>` for progress.
- A merge that touches a tracker file a gate run has modified conflicts on that file. Resolve it field by field: gate state from main, description and title from the branch.
- `! command` under `set -e` asserts nothing in a shell test.
- A comparison library's default build is not its fastest configuration. bchlib needed `CONFIG_BCH_CONST_PARAMS`; read the reference's build options before recording a ratio.
- A criterion that asks for a table can be satisfied by a test: read the criterion before generating inputs (`de311215` REQ-04).
- Rust's orphan rule blocks implementing `std::ops` traits in `gf2-core` for a type defined in a kernel crate; an element type with operators lives where its operators are implemented.
- Appending a bullet to a description that ends with the criteria puts it inside `## Success Criteria`. Add a `## Notes` heading first.
- Count a commit subject before committing; `aa98b11f2` is 72 characters.

## Open questions needing invoker input

- `4f21da96`: the recorded input lists of finished campaigns `a83583e0` and `dbd8787d` name the removed wrapper, and the campaign validator's self-test, one `tuning-campaign-support` test and `check-references.py` (34 faults) require the file to exist. Options: exempt finished-campaign lists and dated session records in those checks, or edit the lists. Recommendation: exempt. The branch also removes the lock turnstile from the bench mutex script; confirm or revert at review.
- `07a1813a` REQ-01 names `kernels::ops::xor_inplace` or an equivalent helper; the merged code uses a plain loop because both measured slower. Recommendation: amend REQ-01 to a loop measured no slower than the kernel.
- Remaining BCH lever: the descending layout reverses every message block, about 10% of short-frame instructions. A reflected-domain fold removes it in about half a day. Not started; decide after the window's BCH table.
- Review fixes were queued behind one worker by owner decision; that worker's session has wound down. Staffing for items 2 to 9 is open.

## Reference artefacts

- Issues with today's recorded results: `befccab3`, `c9cb0dd2`, `be113cbf`, `be6d6f45`, `040bc2b8`, `de311215`, `07a1813a`.
- Unmerged branches: `jit-4f21da96` (`309ab9985`, WIP), `jit-befccab3-toom3` (`7356edcff`, reference only).
- Window runner and queue: `dev/scripts/bench-window/run-window.sh`, `dev/scripts/bench-window/queue.tsv`; state under `.agents/bench-window/`.
- Review prompt: `contrib/gates/code-review-prompt.md`.
- Harnesses: `experiments/encode-compare/compare.sh`, `experiments/clmul-compare/compare.sh`, `experiments/gf256-baselines/run.sh`, `experiments/aff3ct-ldpc-gaps/quick-compare.sh`.
