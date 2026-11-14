# Handoff: Technical debt assessment remediation (2026-06-30) (b4b4b9ee), session 4

**Date:** 2026-11-13
**Prior handoffs:** retired; superseded history is in Git history.

## Current state

- Lead `gf2-73` under steward `gf2-f3`; the epic and every open claim are assigned to `agent:gf2-73`. Peer leads on the shared `main`: `gf2-af` (d77176e5, BCH), `gf2-a6` (45ecf975).
- Landed tonight, all gates passed, closed: `f21b25ac` (merge `ace3b37b9`), `0afbd34a` (merge `b697da248`).
- Open branches, each `worktree-agent-<id>` in `.agents/worktrees/agent-<id>`, worktree clean unless stated:

| Issue | State | Next |
|---|---|---|
| `1b7228b9` | code-review and doc-review passed after rework 1 (`96c326fc6`); stacked on `f21b25ac`, main merged at `f37ae4fa0` | merged-tree `./scripts/cargo-ci.sh` run and a nextest run of the standalone `gf2-kernels-hip` tests under its new config, then land |
| `51f299d2` | gates never completed (stopped mid cargo-ci) | cargo-ci, code-review, doc-review; land; unblocks `2e6e3557` |
| `5a3abee1` | cargo-ci failed only on a main reference fault since fixed | cargo-ci, code-review |
| `22bb2dcd` | rework 1 done (`dc18b2e64`, Codex) | cargo-ci, code-review |
| `697b9178` | rework 1 done (`2310996e5`, Claude) | cargo-ci, code-review; does not close on its unverified Kani harnesses (`903dc74c`) |
| `d73ac633` | rework 1 done (`589cb7aac`, Claude) | cargo-ci, code-review |
| `9c0568ee` | rename committed (`7b2166aae`) | cargo-ci, code-review |
| `e29fd776` | rework 1 done (session 3); round-1 failures recorded | cargo-ci, code-review, doc-review |
| `9ff05f12` | done by worker (session 3) | cargo-ci, code-review, doc-review |
| `32a8bc0e` | rework 1 done; steward ruling below | indication, record gf2-af approval, gates |
| `903dc74c` | Kani 0.68.0 (rustc 1.100 nightly) runs; first gate run verified 23 of 25 including the three named harnesses; the two shift harnesses failed on inline-asm cpuid and are fixed in `a6f3d0b48` (portable funnel under `cfg(kani)`); run recorded in the issue (`814bbca23`) | full `cargo-kani` gate run, gf2-core nextest and clippy, code-review |

- Ready and unclaimed: `2e6e3557` (after `51f299d2`), `c34e8203` (commit-msg hook), `5c1c87b7` (existence-only test audit), `41517289` (owner files the upstream report), and the ready set `jit query available` lists.
- Nothing pushed. No test, build or gate of this lead runs.

## What happened

- `f21b25ac` REQ-05: 3000 serial idle-host runs of `test_gpu_awgn_matches_cpu_within_1_ulp` passed, no amdgpu fault or oversubscription in the journal. REQ-04 was amended by the owner (via the steward); filing the drafted report is `41517289`.
- `1b7228b9` was restacked on `f21b25ac`; the two `gpu-serial` overrides were merged into the one `profile.default` override the lock filter repeats. Four concurrent `group(gpu-serial)` runs passed with no fault. Rework 1 added `gpu_bch_syndrome_byte_identity` to both filters and gave `gf2-kernels-hip` its own `.config/nextest.toml` running the shared wrapper; the filter check covers both configs.
- Reworks dispatched and reviewed: `d73ac633` (trtrm reads only U's upper triangle; every sparse and dense loader bounds dimensions; seed-stream mismatch test), `22bb2dcd` (fresh seed for `rng_seed: None`, resume keeps `avg_queries_per_bit`, structural chunk default with override), `697b9178` (BLER fixture rejects a constant-zero predictor).
- `0afbd34a`: follower status from anchored `window.log` lifecycle records; two doc-review rounds.
- Filed: `9c0568ee`, `41517289`, `c34e8203`, `5c1c87b7`.
- At load 88 the three concurrent cargo-ci chains were stopped before completion; the `ps` capture went to the steward.

## Rulings that bind the next lead

- Owner decisions reach the lead through the steward, and a relayed answer is the owner's answer. Ask the owner directly only when a session refuses the relayed action, then report the outcome to the steward.
- `f21b25ac` REQ-04 (owner, 2026-11-13): a drafted upstream report with a reproducer free of gf2 code meets the reporting branch.
- `2e6e3557` option A (owner, 2026-11-13): workflow clippy, build, test and coverage take `$(./scripts/cargo-features.sh)`; doc and doctest in the workflow and `scripts/cargo-ci.sh` read one variable; the workflow states its non-ROCm coverage; Kani, Lean and profile steps stay. `tuning-profile` survives `5f33fb28` (gf2-a6), so workflow clippy adds `--features tuning-profile`. Starts after `51f299d2` lands. Record the owner approval in the issue.
- `32a8bc0e` (steward): one change under this lead. gf2-af approved the `bch/encode.rs` and `experiments/osd-ebch` parts; record that in the issue with the empty-workspace answer (`encode_batch_parallel_into` returns `BufferLengthMismatch` on an empty slice; `encode_batch_parallel` takes `NonZeroUsize`). Before merge: an indication with `crates/gf2-coding/benches/bch_encode_w1.rs`, rows B2 and T2S, batches 256 and 4096, W1 and W6 cells, `--features parallel`, `RAYON_NUM_THREADS=6`, `taskset -c 12-17`, main against branch, labelled an indication; a regression outside run-to-run spread goes to the steward. It lands before gf2-af's `encode.rs` chain (`6379e04e`, `7fbd1042`, `9a1b24fc`), which rebases onto it; never between a gf2-af gate and its merge.
- `9c0568ee` (owner): rename to a primitivity name, no alias.
- `c34e8203` (owner): versioned commit-msg hook; one source for the type list. Until it lands, check every subject (≤ 71 characters, closed type list) before a merge.
- `5c1c87b7` (owner via steward): mid-tier worker.
- Gating (owner, "max throughput"): no lead test lock; cargo-ci of independent branches may run concurrently under `BUSY_HOST_OVERRIDE=1`, recorded; reviews start as soon as cargo-ci passes, several at once; a test killed even under the busy profile goes to the steward with a `ps` capture. Main still takes a hold from gf2-af and gf2-a6 before a merge or a main-checkout gate. Nothing lands against a failing review.
- File boundary: gf2-af owns `experiments/`, `dev/scripts/bench-window/queue.tsv`, `docs/reference/performance-evidence.md` and BCH encode paths; gf2-a6 owns `scripts/check-references.py`, `scripts/check-tracked-budget.py`, `crates/gf2-kernels-hip/build.rs`, `experiments/permanent-statistics/`. Overlaps go to the steward.

## Next

1. Run cargo-ci for `51f299d2`, `1b7228b9` (merged-tree `./scripts/cargo-ci.sh`, plus `cargo nextest run --manifest-path crates/gf2-kernels-hip/Cargo.toml --profile ci`), `5a3abee1` and `9c0568ee`, at most three at once under `BUSY_HOST_OVERRIDE=1`; then their reviews; land `1b7228b9` and `51f299d2` first under one hold.
2. Gate the reworked `22bb2dcd`, `697b9178`, `d73ac633`, and `e29fd776`, `9ff05f12`; land each on a pass. Read `903dc74c`'s return and gate it.
3. `32a8bc0e` indication and record, then gates and landing; dispatch `2e6e3557` once `51f299d2` is on main, then `c34e8203`, `5c1c87b7` and the ready set.

## Traps

- The cargo-ci gate times out at 900 s. One run took 788 s under load; eight concurrent runs drove the load to 88. Run at most three at once.
- `jit gate evaluate <id> cargo-ci` on an issue without that gate exits 2 with "not required"; read `gates_required` before queuing. `1b7228b9` has only code-review and doc-review.
- `.agents/skills/jit-execution-lead/scripts/land.sh` commits every pending tracker file on main under the subject `chore(jit): sync tracker`, which breaks the convention and sweeps other leads' files. Merge by hand: `jit worktree exec -- git merge --no-ff -m "chore(jit:<id>): merge ..." <branch>`.
- A merge into main refuses while `.jit/events.jsonl` has other sessions' uncommitted lines. Save the appended lines (`git diff -U0`), `git checkout -- .jit/events.jsonl`, merge, then append them back.
- `dispatch-worker-worktree.sh` refuses when main has any uncommitted tracked change, other leads' tracker files included; a copy without the check worked.
- Rebasing a worker branch over main conflicts on its issue JSON (assignee and `gates_status`). A field-wise three-way merge of the JSON resolves it; a failed resolution script once committed conflict markers, so validate the JSON after every resolution.
- zsh parses `$VAR:c...` as a history modifier: write `${B}:crates/...`.
- The permission classifier refused an acceptance-criterion edit relayed from another session; the steward made it in their session.
- A test that only pins a prose string is a literal inventory under `@/inv/shared-test-contracts`; the Codex `9c0568ee` worker added one and the lead removed it.
- Gate runs live in each worktree's ignored `.jit/gate-runs/`: after a merge, copy `.agents/worktrees/agent-<id>/.jit/gate-runs/<id>-*` into the main checkout before `jit issue update --state done`, or jit fails with "Failed to load gate runs".
- Run cargo-ci before code-review; a review of code that then fails CI is wasted.
- Do not stage under `.agents/upstream/`: it is not ignored; stage temporary material under ignored `.agents/scratch/`.
- Do not mistake every cold-build test failure for a code defect. `protocol_contracts::acceptance_rejects_unrecorded_restarts_double_completions_and_abandoned_samples` failed twice just after merges, then passed warm; retain the failure output and confirm on a warm run before reworking it. If a test is killed even under the busy profile, capture `ps` and send it to the steward as ruled above.
- Do not change main while a main-checkout cargo-ci gate is running; it judges the current tree. Wait for the gate or run on an immutable branch.
- Do not let disk fill: worktrees grew 4–11 GB and main's target reached 36 GB; ENOSPC corrupted rustdoc output. Keep the disk watchdog running and reclaim merged worktrees promptly.
- Do not seed or recreate the cache pool from existing worktrees. A stale `gf2-kernels-simd` artifact caused "cannot find m4rm"; delete each worktree's `target/` before reclaim so stale artifacts are not harvested.
- Do not delete committed artifacts without a pin search. `dfb8820b` profiles were pinned by path and digest in checksum manifests and zen3 inputs; search `git grep -n <filename> -- dev` and update pins before removal.
- Verify an issue's behavior against current code before dispatch. Reports included fast tests, unreachable branches, and paths removed by a rewrite; reproduce the premise and reject it with evidence when it no longer holds.
- Reuse canonical test mechanisms instead of adding private duplicates: `0657c9fa` failed review for a private primitivity helper beside `Gf2mField::verify_primitive`; call the shared verifier.
- Test named public behavior directly. `9600eb56` failed review because no test called `decode_soft`; exercise the named method itself.
- Do not hand-copy API inventories into docs. `33057520` failed doc review for listing hash inputs; cite the API that owns the inventory.
- Preserve hot-kernel code shape unless a measured indication supports changing it. `b2520e0f` replaced an unrolled fold with a loop, but timing could not be measured in a working session.
- Keep follow-ups below the number of issues closed and fold small findings into the next issue touching that file; this is the invoker's rule.
- Do not add or remove gates without the invoker's approval. An unasked `doc-review` addition to `f0228940` had to be removed.
- Stop finished workers after their branch commit; a live background command caused the worker to resend its report on every wake.
- Do not use `pgrep -f` in a compound command; the harness blocks it. Use `ps -eo comm,etimes,args`.
- Keep merge subjects at or below 71 characters; one merge was redone. Check the subject length before landing.
- In zsh, do not pass optional `--gate` through `${6:+--gate $6}`; it becomes one word and `jit issue create` rejects it. Build the argument list conditionally.
- Do not edit a benchmark-window run worktree while its queue line is active. Editing `quick-compare.sh` made a job exit 2 and the next job profile dirty code; use a separate detached run worktree.
- Keep builds out of benchmark windows. A 40 s clippy run overlapped NR w24-b16 cells and disturbed them; schedule builds outside the measurement interval.
- Do not transcribe standard tables from memory or trust `pdftotext -layout`; it garbles columns. Confirm each value against the relevant PDF page.
- After merging one branch that regenerates a shared generated file, rebase the next branch on main before merging. The old listings are gone, but the shared-file conflict pattern remains.
- Do not assume `git merge` runs the pre-commit hook. A merge bypassed it and put main 43 bytes over the tracked-tree budget; run `python3 scripts/check-tracked-budget.py` after each merge.
- Workers do not run `jit issue claim`; the lead claims on main. A worker claim left tracker edits in main and blocked lead merges.
- Do not kill unrelated processes by PID; the permission classifier refused it. Stop only tasks this session started and pause workers by message.

## Open questions for the invoker

None.

## References

- Epic `b4b4b9ee`; charter `dev/research/2caf738d-charter.md`; progress `dev/active/b4b4b9ee-tech-debt-2026-06-30/progress.json`.
