# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 31

**Date:** 2026-10-07T18:05Z
**Session number:** 31
**Prior handoffs:** `handoff.md`, then `handoff-2.md` through `handoff-26.md` in this directory.

## Current state

- Epic: `1a379447` — state: backlog (claimed by `agent:jit-execution-lead`).
- Wave in progress: 4 of 7; story `ed3d490e` runs its own layers inside it.
- Children summary: 21 of 24 direct children done or rejected; open `bfc2ceab` (in progress), `7dbe1393` (ready), `1362381c` (backlog). `63bad95d` is in progress below `1362381c`.
- Story `ed3d490e`: 13 issues carrying the story label are done, 3 ready, 54 backlog. Ready: `13728e1a` (AVX2 lane kernels), `4767d936` (rate-matched 5G NR batches in lanes), `4e68f9b2` (DVB-T2 concatenated batches in lanes).
- Active claims: `bfc2ceab` and `63bad95d`, each `agent:worker`; no worker is running.
- Open escalations: none. Two questions for the invoker are listed below; neither blocks tonight's window.
- Benchmark window: timer `gf2-bench-window-20261008` armed for 2026-10-08 02:00 Europe/Helsinki. Queue: `bfc2ceab` `sparse-product-check-cost` pilot (10 min), `63bad95d` `63bad95d-profile-dispatch-overhead` pilot (10 min), `63bad95d` `63bad95d-native-ladder` pilot (12 min).
- Worktrees of this epic: `agent-bfc2ceab` (pinned at `108356c4c`, merged into main), `agent-63bad95d-fam` (pinned at `347b5e2b6`, unmerged, carries the baked cutover), `agent-63bad95d-run` (pinned at `d808af2f3`, unmerged publication; holds the campaign stage backup under `target/campaign-stage-backup/`).
- No worker, build or gate of this epic is running. Main's last cargo-ci on a merged tree of this epic passes (`5504c92a1`); `jit validate` reports no error.
- Progress file: `progress.json` in this directory reflects the above.

## What just happened

- Window of 2026-10-07 collected from each campaign's own log: `4337c02e` confirmation complete and rejected by acceptance (P-03); `bfc2ceab` confirmation accepted (two sparse-row cells regressed, four not-worse); `63bad95d` producer campaign complete-valid, 4644 results.
- `07ae376c` DONE first round. `e6fe64c9` DONE after the invoker amended REQ-01 and one rework. `5cb74f81` DONE (`LdpcBatchDecoder`; doc-review rework by the lead). `1d4b05d5` DONE (`PooledBatchDecoder`; doc-review rework by the lead). `556eb4fa`, `95ca83aa`, `941ac609` DONE first round.
- `95ca83aa`: `compute_systematic_cols` took columns `0..k` on a condition that does not imply independent parity columns; `encoding::SystematicReduction` is the one definition for the encoder, `systematic_cols` and `generator_matrix`.
- `4337c02e` REJECTED as superseded (invoker ruling): code-review round 2 ruled REQ-01 unmet and the family admits no further confirmatory cell. Its evidence, the freezer repair (resolution floored at the whole-pilot half-width) and `63bad95d/selector-outcomes.json` with its check stay on main. `7dbe1393` owns the lane question under the epic.
- `bfc2ceab`: invoker ruled checked entries everywhere and a per-product check for the sparse-row interfaces; production change merged (`cc9bfe66b`); code-review F1 (zero-width product) does not reproduce and is bound by tests (`bffbc2f2a`); F2 (REQ-04) waits on the new family, whose pilot is queued.
- `63bad95d`: campaign published and re-validated (`published`, 13993 entries); interpretation, tables, baked cutover and two pilot families on the work branch; the owner keeps the conservative `bit_matrix.matvec_simd_min_words`.
- Filed: `556eb4fa`, `95ca83aa`, `7dbe1393` (in epic); `817fc47d` (private selector constants, REQ-11 exception of `63bad95d`), `a57cb8d0` (GEMM zero inner dimension), `eb5cae22` (NR mother encoding duplicate elimination), `069e3c33` (encoding directory cache reloads under columns `0..k`) outside the epic.
- Invoker rulings: `e6fe64c9` REQ-01 amended; `bfc2ceab` hoist, new family over two windows, `scripts/cargo-ci.sh` pathspec exclusion approved; `4337c02e` rejected; existing subagents are not reused; no questions while working.
- Eight merged worktrees reclaimed.

## What to do next

- [ ] Collect the 2026-10-08 window from `.agents/bench-window/window.log`, then each campaign's own execution log (cell-start = cell-complete = the addendum's cell count, declared pairs, terminal `complete`, acceptance verdict).
- [ ] `bfc2ceab`: fresh worker in `agent-bfc2ceab`: commit the pilot receipt and ledger, `dev/bench_results/bfc2ceab/run-spmm-check.sh freeze`, smoke, queue the confirmation; after the confirmation, outcome rows in the decision table, merge, then cargo-ci and code-review (Tier 1.5: F1 bound by `crates/gf2-kernels-simd/tests/entry_conditions.rs`, F2 by the confirmation).
- [ ] `63bad95d`: fresh worker in `agent-63bad95d-fam`: commit both pilot receipts, `dev/bench_results/63bad95d/run-profile-families.sh freeze <family> --family-description ...`, smoke, queue two confirmations; after them merge `main` into the branch, regenerate the ledgers and closures, record the REQ-11 exception citing `@/issue/817fc47d`, then merge to main and run cargo-ci, code-review, research-review. The baked `bit_backend.simd_min_words` 8 to 4 merges only with its confirmation.
- [ ] Dispatch `13728e1a` (opus; kernel crate, asm artefacts), `4767d936` and `4e68f9b2` (opus). `941ac609`'s suite takes the AVX2 bundle as one more route once `LdpcBatchDecoder` can select it.
- [ ] Dispatch `7dbe1393` (opus): profile and assembly attribution first; its measurement is a new family over two windows.
- [ ] After `63bad95d` closes: `807ddab2` (comment trim), then `1362381c` (scorecard). Reconcile `surfaced_pitfalls` against the epic criteria before the epic gates.
- [ ] Reclaim `agent-63bad95d-run` and `agent-63bad95d-fam` when `63bad95d` closes, `agent-bfc2ceab` when `bfc2ceab` closes (`LEAD_CACHE_DIRS=none`).

## Traps — do not repeat these

- **Do NOT state a family's direction from its ratios without reading which arm is the baseline.** The lead briefed `63bad95d` that SIMD led at 9 and 63 words in the `4337c02e` family; the baseline there is the SIMD lane and the candidate the scalar lane, so a ratio above 1 favours scalar. Read `plan.json` arms first.
- **Do NOT let a freezer extension narrow the resolution below what acceptance verifies.** `4337c02e`'s freezer floored the resolution at the confirmation's own cells (0.034); P-03 verifies the whole pinned pilot (0.228) and rejected the receipt, spending the family's only confirmatory attempt. Keep one thermal class per family; the freezer on main now floors at the whole pilot.
- **Do NOT run `jit doc add` over a links table that holds a frozen addendum or derivation record with flat-layout links.** Both fail `jit doc check-links` with missing assets; the lead linked and unlinked them twice. Skip them.
- **Do NOT discard the output of the `63bad95d` ledger generators.** Run with stderr discarded they failed silently on main after two merges (`sim-default-empty` claim, undeclared `4337c02e` track); print the last line and the exit status.
- **Do NOT trust an sccache server after any gate run.** It was stale three times this session (`Failed to create temp dir /tmp/tmp.*`), failing every compile step of two cargo-ci gates. `sccache --stop-server`, then start one from the home directory with `TMPDIR=/tmp sccache --start-server` before a gate chain; workers use `CARGO_CI_NO_SCCACHE=1`.
- **Do NOT read a `fixture-leak` failure or three `driver_launcher` failures as a tree defect while other suites run.** Each occurred once and passed on the re-run of the same tree.
- **Do NOT expect the dispatch script to run while the peer lead's tracker files are dirty on main.** It refuses any uncommitted change; wait for the peer's commit (a loop on `git status --porcelain`), never stash or commit its files.
- **Do NOT write `$B:path` in zsh** (`git show $B:crates/...` parsed `:c` as a modifier). Write `"${B}:crates/..."`.
- **Do NOT `cd` into any subdirectory in the lead's shell**, a worktree or not: the session's working directory followed twice. Use absolute paths and `git -C`.
- **Do NOT message a worker that has reported** (invoker ruling): its cache is gone. Dispatch a fresh agent with a self-contained brief; the pinned worktree and branch carry the state.
- **A worker report can cross a ruling.** `bfc2ceab`'s worker reported "needs decision" twice after the ruling was in its inbox; check the branch for commits before acting on an idle notice.
- **The 4337c02e window job has a hand-made done marker** (`ae13fbd8b6c8b708.done`): it exited 1 on the acceptance verdict after a complete run and would otherwise re-run.
- **A code-review finding can be false.** `bfc2ceab` F1 claimed `checked_div(0)` fails the assertion; `is_none_or` accepts `None`. The response was a test that binds the behaviour, not an argument.
- All traps of [handoff-26](handoff-26.md) and earlier remain in force.

## Open questions needing invoker input

- Question: `63bad95d` REQ-09 "equivalent competitor targeting opportunities" — is the committed M4RI confirmation of `50f0bd42` sufficient comparator evidence, with the matvec external ladder excluded because the profile keeps the conservative lane there?
  - Context: a native-level M4RI cell would be a new family (dense harness with `-march=native` M4RI), two more windows.
  - Options: (A) the committed confirmation stands and the exclusion is recorded; (B) add the native M4RI family.
  - Recommendation: A; research-review judges it at closure either way.
- Question: `63bad95d` publishes `ple.scalar_base_max_cols` 1 where the earlier owner stated 24, and no family of this epic measures `FieldMatrix::ple`. Accept it as recorded and unconfirmed, or keep the earlier value until a family measures it?
  - Recommendation: keep the earlier value in the baked constants until measured; the worker recorded it as finding F-04 only.

## Reference artefacts

- Epic: `jit issue show 1a379447`; progress: `progress.json`; worker brief `worker-brief.md`; measurement contract `measurement-contract.md`.
- Reviews of this session: `reviews/07ae376c-r1.md`, `reviews/e6fe64c9-r2.md`, `reviews/5cb74f81-r1.md`, `reviews/1d4b05d5-r1.md`, `reviews/4337c02e-r2.md`, `reviews/556eb4fa-r1.md`, `reviews/95ca83aa-r1.md`, `reviews/941ac609-r1.md`.
- Lane-threshold evidence: `4337c02e/outcome.md`, `4337c02e/outcome-record.md`, `4337c02e/tables.md`.
- Entry checks: `bfc2ceab/decision-table.md`, `bfc2ceab/decisions.json`, `bfc2ceab/addendum-v4-spmm-check-pilot.json`, `dev/bench_results/bfc2ceab/tables.md`.
- Calibration (on branch `worktree-agent-63bad95d-fam`): `63bad95d/campaign-tables.md`, `63bad95d/campaign-findings.md`, `63bad95d/premeasurement-protocol-corrections.md`, `dev/bench_results/63bad95d/run-profile-families.sh`; publication on branch `worktree-agent-63bad95d-run` under `dev/benchmarks/tuning_profiles/`.
- Story `ed3d490e`: `dev/active/ed3d490e-zen3-ldpc-frame-simd/plan.md`, `breakdown.json`; decoders `crates/gf2-coding/src/ldpc/batch.rs`, `crates/gf2-coding/src/pooled.rs`; suites `crates/gf2-coding/tests/ldpc_batch_contract.rs`, `ldpc_message_round_trip.rs`, `pooled_batch_decoder.rs`.
- Window: `bench-window/run-window.sh`, `bench-window/queue.tsv`, state `.agents/bench-window/`.
