# Handoff: Technical debt assessment remediation (2026-06-30) (b4b4b9ee), session 4

**Date:** 2026-11-13
**Prior handoffs:** `dev/active/b4b4b9ee-tech-debt-2026-06-30/handoff.md`, `dev/active/b4b4b9ee-tech-debt-2026-06-30/handoff-2.md`, `dev/active/b4b4b9ee-tech-debt-2026-06-30/handoff-3.md`

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
| `903dc74c` | worker installed Kani 0.68.0 (rustc 1.100 nightly); gate result pending at stop | read the worker's return; run `cargo-kani`, code-review |

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
- Earlier traps still hold: copy gate runs to the main checkout before `--state done`; run cargo-ci before code-review; `.agents/upstream/` is not ignored.

## Open questions for the invoker

None.

## References

- Epic `b4b4b9ee`; charter `dev/research/2caf738d-charter.md`; progress `dev/active/b4b4b9ee-tech-debt-2026-06-30/progress.json`.
