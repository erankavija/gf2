# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 30

**Date:** 2026-10-06T21:05Z
**Session number:** 30
**Prior handoffs:** `handoff.md`, then `handoff-2.md` through `handoff-25.md` in this directory. Session 29 was interrupted and wrote no handoff; this one covers both.

## Current state

- Epic: `1a379447` — state: backlog (claimed by `agent:jit-execution-lead`).
- Wave in progress: 4 of 7; story `ed3d490e` runs its own layers inside it.
- Children summary: 17 of 22 direct children done; in progress `4337c02e`, `bfc2ceab`, `e6fe64c9`; ready `07ae376c`; backlog `1362381c`. `63bad95d` is in progress below `1362381c`.
- Story `ed3d490e`: 7 of 66 tasks done (`c79d60f9`, `6818f1b6`, `8840b0ff`, `53b74fc2`, `ee12b09e`, `2438c16f`, `441fbd5c`). Ready: `5cb74f81` (LDPC lane batch decoder), `1d4b05d5` (pooled batch decoder).
- Active claims: `4337c02e`, `bfc2ceab`, `e6fe64c9`, `63bad95d`, each `agent:worker`, no worker running.
- Open escalations: `e6fe64c9` REQ-01 wording; `bfc2ceab` per-interface choice (after its confirmation).
- Benchmark window: timer `gf2-bench-window-20261007` armed for 2026-10-07 02:00 Europe/Helsinki, confirmed by the invoker. Queue: `4337c02e` confirmation (15 min), `bfc2ceab` confirmation (8 min), `63bad95d` producer calibration campaign (160 min).
- Worktrees of this epic: `agent-4337c02e` and `agent-bfc2ceab` (pinned, unmerged branches), `agent-63bad95d-run` (pinned at `65f178adc`, branch `worktree-agent-63bad95d-run`). Thirteen merged worktrees reclaimed.
- No worker, build or gate of this epic is running. Main's last cargo-ci on a merged tree of this epic passes (`1fde31e97`); `jit validate` exits 0.
- Progress file: `progress.json` in this directory reflects the above.

## What just happened

- Session 29 (rebuilt from git, the tracker and its transcript): `c79d60f9` and `6605cac2` closed; eight workers delivered; the two 2026-10-06 pilots ran.
- Both pilots verified from their execution logs: `4337c02e` 17 cells, `bfc2ceab` 9 cells, each accepted with a terminal `complete`. Confirmations frozen on the worker branches (`4337c02e` six cells at resolution 0.034; `bfc2ceab` six cells, resolution 0.02, margin 1.05).
- `63bad95d` validator stage merged (`f94f90c8b`): the validator takes each campaign's field table from its own inventory; CI step `tuning-campaign-records`; producer campaign queued.
- `6818f1b6` closed after one doc-review rework. `dd359005` closed after the invoker amended REQ-02 (run record establishes Rust 1.95; CI runs its own toolchain).
- `8840b0ff` closed after one rework: decoded length comes from `SoftDecoder::soft_decoded_len` and `IterativeSoftDecoder::iterative_decoded_len`; `LdpcDecoder::decode_soft` returns `n` bits, its `decode_iterative` returns `k`.
- `53b74fc2` closed first time: `gf2_kernels_simd::ldpc_lanes` (`F32x8`, `LaneMask`, `LaneMinSumRule`, `LdpcLaneFns::portable()`, `contract::assert_lane_contract`).
- `ee12b09e` closed first time: `gf2_coding::ldpc::route::decide_batch_route`, `BatchRoute`, `LaneCapability`.
- `2438c16f` closed first time: `gf2_coding::traits::conformance::batch_soft_decoder_contract` with `ConvergenceMix`; integration test `batch_soft_decoder_contract`.
- `441fbd5c` closed first time: `gf2_sim::stages::batch_decode::BatchDecode<F>` and `BatchDecodeScratch`.
- `f6df53a9` closed after two reworks (input-derived test values and batch rustdoc; evidence citation in the contract page by the lead). Lead decision: a cap of zero returns the result of the frame's own channel LLRs with iteration count zero.
- `dcfe8386` closed; its one code-review failure was unlinked records, linked by the lead.
- `e6fe64c9`: invoker ruled removal; merged (`42015c6b3`), cargo-ci passes; code-review fails on a frozen transcript of epic fa787f85. Blocked.
- Filed `07ae376c` (default `decode_soft_with_result` reports `converged: true, iterations: 1` on the LDPC and GLDPC hard-decision path).
- Invoker rulings this session: standing rule "provenance or protocols shall not prevent work; focus on HPC"; no questions during the session; the zen3 `dev/active` entries move under this directory (executed by the fa787f85 lead, session gf2-14, in three merges from `97ce2d18d`).
- Decided by the lead under the standing rule and reported: session-record check accepted as re-validation of `a83583e0`; CI step `tuning-campaign-records` kept; the freezer reads pre-declared confirmation cells from the prose addendum the pilot pins.

## What to do next

- [ ] Verify the 2026-10-07 window from `.agents/bench-window/window.log`, then each campaign's own execution log (cell-start = cell-complete = the addendum's cell count, every cell measured, terminal `complete`, acceptance verdict).
- [ ] `63bad95d` campaign: in `/tmp/<id>/execution.log`, 4644 cell-start, cell-complete and checkpoint-accepted records and a final `complete`; then `validate-tuning-extent-campaign.py --stage /tmp/<id>` prints `complete-valid` and `--publication <run worktree>` passes. `paused` or `budget-exhausted`: add a queue line `env PATH=/home/vkaskivuo/.cargo/bin:/usr/local/bin:/usr/bin dev/scripts/tuning-extent-campaign.sh gf2-63bad95d-<stamp>-<pid>` on the same worktree (identifier on the `GF2_CAMPAIGN_EXECUTION_LOG=` line of the job's `.out` file) and arm the next window. The stage lives in `/tmp`; a reboot loses it. Keep `agent-63bad95d-run` until `63bad95d` closes.
- [ ] After the confirmations are collected: merge main into `worktree-agent-4337c02e` and `worktree-agent-bfc2ceab`. Main moved their entries: put added files (`c7113c5a/survey/rederive-confirmation.py`, freezer edits, `2037941f-*` and `f547c394` edits) at the paths under this directory, regenerate the closure JSON files with their generators, regenerate `schema-versions.json` (`check-addendum-schema-versions.py --write`), run `rederive-confirmation.py` on main's records. Then outcome prose, gates, and for `4337c02e` the hand-over of the value to `63bad95d`.
- [ ] `bfc2ceab`: bring `bfc2ceab/decision-table.md` with the confirmed cost rows to the invoker; then stage 2.
- [ ] Dispatch `5cb74f81` (LDPC lane batch decoder, the main HPC item; opus) and `1d4b05d5` (pooled batch decoder). `5cb74f81` converts `MinSumRule` to `LaneMinSumRule`, calls `LdpcLaneFns`, decides with `decide_batch_route`, and passes `batch_soft_decoder_contract` with at least 17 distinct-codeword frames. Read gates from `gates_required` of each issue file.
- [ ] Dispatch `07ae376c` (sonnet).
- [ ] After each merge that adds a source file: the landing script pattern of this session (merge, family closure check and regeneration, `jit gate evaluate <id> cargo-ci`, commit tracker files by name), then `63bad95d/survey/make-source-evidence.py` and `make-route-inventory.py` twice.
- [ ] Notify the fa787f85 lead when `e6fe64c9` is ruled; it repoints the flat paths this directory's handoffs and reviews still cite.
- [ ] File or drop the reported defects in `progress.json` notes: tuning-campaign full-stage re-validation needs the producing worktree; the gated LDPC non-finite test fails on a host without AVX2 when `simd` is on; 20 early-return tests in probe and bench crates outside the workspace; `cargo doc -p gf2-coding` with warnings denied fails on four intra-doc links.

## Traps — do not repeat these

- **Do NOT take a gate list from a dispatch habit.** The lead told the `f6df53a9` worker its gates were cargo-ci and code-review; the issue carries doc-review too, which surfaced only when the issue stayed `gated`. `jq -r '.gates_required|join(",")' .jit/issues/<id>*.json` is populated; the handoff-23 trap about a null field describes `jit issue show --json`.
- **Do NOT gate an issue before linking the artifacts its commits add.** `dcfe8386` failed code-review solely on an unlinked run record under its directory (`@/invariant/active-document-layout`).
- **Do NOT set `BUSY_HOST_OVERRIDE=1` on a quiet host.** The invoker objected; the handoff-25 rule is for runs while workers build. A forced re-run to replace such a record is denied by the permission classifier as a CI bypass, so run without it the first time.
- **Do NOT stage `.jit` with `git add -A` or a broad glob while another lead works on main.** One glob staged two issue files of epic fa787f85; stage `.jit/events.jsonl` and this epic's issue files by name.
- **Do NOT run a dirty-tree guard that counts the peer's files.** The landing script stopped twice on the other lead's uncommitted tracker and progress files; exclude `.jit/` and the peer's directory from the guard.
- **Do NOT end a shell pipeline in `| tail` with `||` and expect the left side's status.** A closure check piped that way reported a stale closure only by its text; capture the status before filtering.
- **A cargo-ci failure with `sccache: Failed to create temp dir` is the stale-server trap of handoff-17**: `sccache --stop-server`, re-run.
- **Do NOT write a bare `echo ====`-style word in zsh** (handoff-11): it aborted one command chain this session.
- **Seeded `target/` trees fail with "cannot find `m4rm` in `gf2_kernels_simd`"**; five of eight workers hit it. `cargo clean -p gf2-kernels-simd -p gf2-core --profile ci-test` fixes it; put the line in every dispatch.
- **`gf2-sim` has no `simd`, `parallel` or `visualization` feature of its own.** A dispatch that named them was rejected by cargo; the worker used the crate's default features.
- **A removal criterion written as "every document and command that names it" reaches frozen transcripts of other epics** (`e6fe64c9`). Scope such a criterion to current commands and documents when filing.
- **Agree a merge and gate order with a second lead on main before the first gate.** Announcing each merge batch and each gate batch by message worked; a cargo-ci of the other lead failed twice on 15-second timeouts while this epic's cargo-ci ran.
- All traps of [handoff-25](handoff-25.md) and earlier remain in force.

## Open questions needing invoker input

- Question: `e6fe64c9` — how does REQ-01 close against `dev/active/fa787f85-documentation-overhaul/2596b143-doctest.txt:15`?
  - Context: the file is tool output at commit `cac8fa20e` and records the doctest command with `llr-f64`; the census record of done issue `2596b143` cites its lines by number.
  - Options: (A) amend REQ-01 to exempt records of runs at a named earlier commit; (B) epic fa787f85 regenerates the transcript and its citations.
  - Recommendation: A.
- Question: `bfc2ceab` — check or `_unchecked` per kernel interface.
  - Context: the pilot's two sparse-row cells are `regressed` (exploratory); the confirmation runs in the 2026-10-07 window.
  - Recommendation: decide from `bfc2ceab/decision-table.md` after the confirmation.
- Question: `8840b0ff` cargo-ci record was made under `BUSY_HOST_OVERRIDE=1` at `b7d604eed`; its later run at `52f9535c0` used no override. No action proposed.

## Reference artefacts

- Epic: `jit issue show 1a379447`; progress: `progress.json`; worker brief `worker-brief.md`; measurement contract `measurement-contract.md`.
- Reviews of this session: `reviews/6818f1b6-r1.md`, `reviews/dd359005-r1.md`, `reviews/8840b0ff-r2.md`, `reviews/53b74fc2-r1.md`, `reviews/ee12b09e-r1.md`, `reviews/dcfe8386-r1.md`, `reviews/2438c16f-r1.md`, `reviews/441fbd5c-r1.md`, `reviews/f6df53a9-r2.md`.
- Story `ed3d490e`: `dev/active/ed3d490e-zen3-ldpc-frame-simd/plan.md`, `breakdown.json`, `terminal-audit.md`; numerical contract `docs/reference/ldpc-decoder-numerical-contract.md`.
- Window: `bench-window/run-window.sh`, `bench-window/queue.tsv`, state `.agents/bench-window/`.
- Pilot records on the pinned branches: `4337c02e/pilot-resolution.md`, `bfc2ceab/decision-table.md`, `bfc2ceab/confirmation-derivation-entry-check.txt`.
- Calibration: `63bad95d/calibration-plan.md`, `63bad95d/premeasurement-protocol.md`, `63bad95d/campaign-declaration.json`.
