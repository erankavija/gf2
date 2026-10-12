# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 32

**Date:** 2026-10-08T16:45Z
**Session number:** 32
**Prior handoffs:** `handoff.md`, then `handoff-2.md` through `handoff-27.md` in this directory.

## Current state

- Epic: `1a379447` — state: backlog (claimed by `agent:jit-execution-lead`).
- Wave in progress: 4 of 7; story `ed3d490e` runs its own layers inside it.
- Children summary: 21 of 24 direct children done or rejected; open `bfc2ceab` (in progress), `7dbe1393` (in progress), `1362381c` (backlog). `63bad95d` is in progress below `1362381c`.
- Story `ed3d490e`: 24 issues carrying the story label are done, 2 in progress (`feb60d0d`, `aff59c3c`), 3 ready (`a96a4cd6`, `395fe4e2`, `e4e2ba52`), 41 backlog.
- Active claims, each `agent:worker`, no worker running: `bfc2ceab`, `63bad95d`, `7dbe1393`, `feb60d0d`, `aff59c3c`.
- Open escalations: one, the form of the `tuning-campaign-support` dependency (`aff59c3c`), below.
- Benchmark window: timer `gf2-bench-window-20261009` armed for 2026-10-09 02:00 Europe/Helsinki with seven jobs, in queue order: `63bad95d` native-ladder confirmation, `bfc2ceab` `pilot2`, `63bad95d` dispatch-overhead `pilot2`, `7dbe1393` dense profile session, `7dbe1393` ragged-tail pilot, `63bad95d` native gf2 against M4RI pilot, `63bad95d` portable gf2 against portable M4RI pilot.
- Pinned worktrees, all clean: `agent-bfc2ceab` (`73e37519d`), `agent-63bad95d-fam` (`345f7a6aa`), `agent-63bad95d-run` (`d808af2f3`), `agent-63bad95d-m4ri` (`ec5ab3404`), `agent-63bad95d-pm4ri` (`90dbb0482`), `agent-7dbe1393` (`e473f9344`). None is merged into main.
- No worker, build or gate of this epic is running. Main's last merged-tree `./scripts/cargo-ci.sh` passes on the tree of `184ba648e`; `jit validate` reports no error.
- Progress file: `progress.json` in this directory reflects the above.

## What just happened

- Window of 2026-10-08 collected: three pilots complete and accepted (`bfc2ceab` sparse-product check cost, `63bad95d` dispatch overhead and native ladder). The host was loaded at the window's start; one disturbed stretch of pairs set the resolution of the `bfc2ceab` pilot and of the dispatch-overhead pilot.
- `bfc2ceab`: pilot committed (`222b02396`); the freezer refused the confirmation (resolution at the worthwhile margin). Lead decision: frozen margins stay, second pilot at the protocol's pair cap, queued.
- `63bad95d`: both pilots committed; native-ladder confirmation frozen and queued; the dispatch-overhead confirmation of pilot 1 superseded (margin from a disturbed cell), second pilot queued.
- `7dbe1393` stage 1 DONE on its branch: assembly attribution, a masked-tail parity kernel behind `BitMatrix::matvec_parity_kernel` (production dispatch untouched), family `7dbe1393-matvec-ragged-tail` frozen and smoked; profile session and pilot queued.
- Closed, story `ed3d490e`: `4e68f9b2`, `c317f5ac`, `f5ae4c49`, `83ed701d`, `391f5419`, `86d9260e`, `4de9d983` first round; `4767d936` (one rework), `13728e1a` (two reworks), `e918a8ee` (one rework), `9e0cd346` (one rework).
- Invoker rulings: `4767d936` REQ-04 kept, external vectors obtained (Sionna `LDPC5GEncoder`, Apache-2.0; the crate's encoder reproduces every codeword); `13728e1a` REQ-04 amended to the crate's host-precondition contract; `63bad95d` gains a native and a portable M4RI family; `ple.scalar_base_max_cols` returns to 24 before the `63bad95d` merge; the two `tuning-coding-calibration` steps of `scripts/cargo-ci.sh` approved.
- `63bad95d` M4RI: the committed `50f0bd42` comparison is gf2 default build against M4RI built `-O3 -march=native`; no separate native M4RI build was needed. Native family (gf2 `-C target-cpu=native`, baked tuning) and portable family (M4RI `-O3`, no targeting) each frozen, smoked and queued in its own worktree.
- `feb60d0d` and `aff59c3c` merged and FAILED code-review on `@/invariant/no-dev-path-coupling`; `aff59c3c` waits on the escalation below.
- Filed outside the epic: `02a26b83`, `d1bfe986`, `07fe99b8`, `81c7d370`, `dd493649`, `f21b25ac`.
- Fifteen merged worktrees reclaimed without harvest.

## What to do next

- [ ] Collect the 2026-10-09 window from `.agents/bench-window/window.log`, then each campaign's own execution log (cell-start = cell-complete = the addendum's cell count, declared pairs, terminal `complete`, acceptance verdict). Read each launcher log's load at start.
- [ ] `63bad95d` native ladder: fresh worker in `agent-63bad95d-fam` commits the confirmation receipt and ledger.
- [ ] `bfc2ceab`: fresh worker in `agent-bfc2ceab` commits the second pilot, `run-spmm-check.sh freeze` (reads `r1-spmm-check-pilot2`), smoke, queue line for the confirmation. If the freezer refuses again the family has no pilot trial left: record the measured cost as the outcome REQ-04 admits, or escalate.
- [ ] `63bad95d` dispatch overhead: `run-profile-families.sh freeze 63bad95d-profile-dispatch-overhead pilot2`, smoke, queue `confirmation2`.
- [ ] `7dbe1393`: fresh worker in `agent-7dbe1393`: commit the profile record and pilot receipt, generate the profile tables, freeze the confirmation, smoke, queue.
- [ ] `63bad95d` M4RI families: commit both pilots, freeze, smoke, queue both confirmations. Merge order after their confirmations: `worktree-agent-7dbe1393`, then `worktree-agent-63bad95d-m4ri` as it stands, then `worktree-agent-63bad95d-pm4ri` (it contains the native branch).
- [ ] After the `63bad95d` confirmations: merge `main` into the family branch, regenerate ledgers and closures, restore `ple.scalar_base_max_cols` 24, correct `calibration-plan.md` section 3 and `survey/planned-families.json` (they call the `50f0bd42` comparison a portable comparator), record the REQ-11 exception citing `@/issue/817fc47d`, then gates.
- [ ] `aff59c3c`: apply the invoker's answer to the escalation, re-run code-review.
- [ ] `feb60d0d`: rework per `reviews/feb60d0d-r1.md` (manifest paths, REQ-05 evidence), then cargo-ci, code-review; research-review passed on `1c1866770`.
- [ ] Dispatch the ready story issues `a96a4cd6`, `395fe4e2`, `e4e2ba52`; `e4e2ba52` removes the inherent batch functions, which have no caller left.
- [ ] Reclaim each pinned worktree when its issue closes (`LEAD_CACHE_DIRS=none`).

## Traps — do not repeat these

- **Do NOT let session work load the host near the window.** The 2026-10-08 window started at a one-minute load above three; one disturbed stretch cost two families a pilot trial and a night each. Stop builds and gates by 01:30 local.
- **Do NOT brief P-03 as verifying the confirmation's corrected alpha.** `receipt.rs` (`verified_pilot_alpha`) and the freezer use the pilot's own corrected alpha; two workers corrected the lead.
- **Do NOT assume a comparator's build level from a plan's prose.** The lead asked the invoker about a "native M4RI family" while the pinned M4RI install was already `-march=native`; `calibration-plan.md` called it portable. Read the build record and the compile lines.
- **Do NOT write an issue criterion or an amendment stricter than the convention it names.** The lead's amended `13728e1a` REQ-04 said the failure "names the missing instruction set" while the crate's tests pass a `module::detect()` label; code-review failed it once more.
- **Do NOT tell workers which co-author trailer to use.** The session's common rules named the lead's model; sonnet workers followed. Wrong trailers on main: `99f1ade19`, `9d1ca38a2`, `b9340fbec`; on the pinned `bfc2ceab` branch: `222b02396`.
- **Do NOT merge a branch that adds a source file under `crates/` without the family closure check.** `cargo-ci`'s `family-producing-closures` step failed after `4e68f9b2`; regenerate with the family's `make-*-producing-inputs.py` inside or directly after the merge.
- **Do NOT evaluate `asm-artefact-present` after any later commit.** It reads `HEAD~1..HEAD`; evaluate it on the merge commit that carries the kernel source with its artefact.
- **Do NOT expect a criterion that names "the existing suites" to hold vacuously.** `9e0cd346` REQ-03 failed code-review because no byte-identity suite contained an NR pipeline; the rework added the case.
- **Do NOT read a worker's "waiting on a notification" as progress.** `13728e1a`'s worker idled 21 minutes after its build ended; a message restarted it. A worker that has not sent its final report may be messaged.
- **Do NOT start a gate chain before claims, queue lines and worktrees of the next dispatch are committed.** Gates dirty `.jit/`, the dispatch script refuses a dirty main and no commit lands during a gate; the second sub-wave waited 75 minutes for this.
- **Do NOT read one failing GPU or allocation-count test in a loaded run as a tree defect.** Three tests failed once each and passed before and after on the same tree (`@/issue/f21b25ac`).
- **Do NOT `cd` in the lead's shell, even to read a file.** It happened once at session start; use absolute paths.
- All traps of [handoff-27](handoff-27.md) and earlier remain in force.

## Open questions needing invoker input

- Question: in what form may a workspace crate depend on `dev/tools/tuning-campaign-support`?
  - Context: code-review failed `aff59c3c` on `crates/gf2-coding/Cargo.toml` naming `../../dev/tools/tuning-campaign-support`; `gf2-core` and `gf2-algebra` declare the same path, and the `harness-path-dependencies` exception covers survey harness manifests under `dev/active/` only.
  - Options: (A) one `[workspace.dependencies]` entry at the repository root with `workspace = true` in the three owner crates; (B) move the crate under `crates/`; (C) extend the named exception to owner-producer dev-dependencies.
  - Recommendation: A if the reviewer's reading admits a root-relative path in the workspace manifest, otherwise C; B moves a crate that committed campaigns pin by path.

## Reference artefacts

- Epic: `jit issue show 1a379447`; progress: `progress.json`; worker brief `worker-brief.md`; measurement contract `measurement-contract.md`.
- Reviews of this session: `reviews/4e68f9b2-r1.md`, `reviews/4767d936-r2.md`, `reviews/13728e1a-r1.md`, `reviews/13728e1a-r2.md`, `reviews/c317f5ac-r1.md`, `reviews/f5ae4c49-r1.md`, `reviews/83ed701d-r1.md`, `reviews/391f5419-r1.md`, `reviews/86d9260e-r1.md`, `reviews/e918a8ee-r1.md`, `reviews/e918a8ee-r2.md`, `reviews/9e0cd346-r1.md`, `reviews/9e0cd346-r2.md`, `reviews/4de9d983-r1.md`, `reviews/feb60d0d-r1.md`, `reviews/aff59c3c-r1.md`.
- Window: `bench-window/run-window.sh`, `bench-window/queue.tsv`, state `.agents/bench-window/`.
- Entry checks: `bfc2ceab/decision-table.md`; on branch `worktree-agent-bfc2ceab`: `bfc2ceab/survey/spmm-second-pilot-pairs.json`, `dev/bench_results/bfc2ceab/run-spmm-check.sh`.
- Calibration, on branch `worktree-agent-63bad95d-fam`: `63bad95d/campaign-findings.md`, `63bad95d/campaigns/63bad95d-profile-dispatch-overhead-confirmation-superseded.md`, `dev/bench_results/63bad95d/run-profile-families.sh`. On `worktree-agent-63bad95d-m4ri`: `63bad95d/native-m4ri-plan.md`. On `worktree-agent-63bad95d-pm4ri`: `63bad95d/portable-m4ri-plan.md`, `2037941f-profile-and-optimize-mid-range-buffer-operations/m4ri-portable-build-record.txt`.
- Lane gap, on branch `worktree-agent-7dbe1393`: `7dbe1393/attribution.md`, `7dbe1393/measurement-plan.md`, `7dbe1393/campaigns/matvec-ragged-tail.json`.
- Story `ed3d490e`: `dev/active/ed3d490e-zen3-ldpc-frame-simd/plan.md`; decoders `crates/gf2-coding/src/ldpc/batch.rs`, `ldpc/route.rs`, `ldpc/nr_5g/batch.rs`, `ldpc/dvb_t2/concat/batch.rs`; kernels `crates/gf2-kernels-simd/src/x86/ldpc_lanes.rs`; external vectors `crates/gf2-coding/data/ldpc/nr_5g/PROVENANCE.md`; arms `dev/active/ed3d490e-zen3-ldpc-frame-simd/survey/arms/`.
