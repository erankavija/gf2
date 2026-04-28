# Handoff — gf2-core PPC-spiral performance sweep (babcf05e) — session 6

**Date:** 2026-04-28
**Session number:** 6
**Prior handoffs:** `babcf05e-handoff.md` (s1), `babcf05e-handoff-2.md` (s2), `babcf05e-handoff-3.md` (s3)

> Note: there is no `babcf05e-handoff-4.md` predecessor — sessions 4 and 5 closed without writing a handoff. Per progress.json `notes` (entries for sessions 4 and 5), session 4 expanded the epic with the algorithmic-gap story `3abb755e` + tasks `085cbd0b`/`59c487c3`/`e7ab802d`/`577b9e7f` and closed `c69d2055` and `cad241e6`; session 5 closed `8e4b189c`, `5223bb04`, and the parent story `d76f6931` with one user-approved aggregate amendment for A3. This handoff is therefore the *fourth* numbered handoff but covers session *six*.

## Current state

- Epic: `babcf05e` — state: **in_progress**, claimed by `agent:project-lead`
- Wave in progress: **wave 4 (Tier B + C interleaved)** — sub-wave **4a in progress, NOT closed**
- Children summary: 7 done in earlier sessions; 4 currently `in_progress` (claimed by `agent:claude-wt-*`); the rest backlog/ready as before
- Active claims (NOT released; per-worktree branches still hold their work):
  - `ec286cee` (C1) — `agent:claude-wt-ec286cee`
  - `86c09a51` (C3) — `agent:claude-wt-86c09a51`
  - `1c1c4242` (B1) — `agent:claude-wt-1c1c4242`
  - `54a0e75c` (B2) — `agent:claude-wt-54a0e75c`
- Open escalations: none
- Progress file: `dev/active/babcf05e-progress.json` (refreshed this session)
- Branch state: `main` clean at `c8d71af`. **No worktree work merged.** Per-worker WIP preserved on the worker branches (see Reference artefacts).

### Issue map at handoff

| Issue | Wave | State | Worktree branch | Worker commits | WIP-preserve | Notes |
|---|---|---|---|---|---|---|
| `c7791a20`–`d76f6931` | 1–3 | done | — | — | — | Closed in sessions 1–5 |
| `ec286cee` (C1) | 4a | in_progress | `worktree-agent-aabe7a5530874a3fa` | 4 (V0+V3+V4 combined → API → tests → bench/manifest/asm) | clean tree | Looks complete; needs integration-rebase (see TRAP 1) |
| `86c09a51` (C3) | 4a | in_progress | `worktree-agent-ad855fb0f45a05064` | 1 clean (V0 pin only) + 1 lead-preserve WIP | `16dbba3` | Bulk SIMD work is in WIP commit, NOT clean spiral steps |
| `1c1c4242` (B1) | 4a | in_progress | `worktree-agent-a7514de96318093e7` | 4 clean (V0, manifest, V4, V3) + 1 lead-preserve WIP | `d773986` | V3 reports 65× at n=4096; V7 cache-tile in WIP commit |
| `54a0e75c` (B2) | 4a | in_progress | `worktree-agent-ad0ba67f63572849c` | 0 clean + 1 lead-preserve WIP | `5e7c138` | All work uncommitted at API failure; preserved as a single blob |
| Sub-wave 4b: `19bc3199` (B3), `7c954fb5` (C2), `3168d114` (C4) | 4b | backlog/ready | — | — | — | Awaiting B1 close (B3 dep) and bandwidth |
| Wave 6 (D1, D2), 7 (E1, E2), 8 (`59c487c3`, `e7ab802d`, `577b9e7f`) | later | backlog | — | — | — | Wave-8 tasks block on C1 / C3 / B3 closes |

## What just happened (session 6)

Concrete log:

- **Resumed epic** at session 6 entry. Read all prior handoffs. Validated branch state at `5a9393c`, 41 commits ahead of origin.
- **Strategic interview (3 questions, AskUserQuestion):**
  - Wave priority → user picked **algorithmic-value-first** (Tier-C parallel + B1+B2 alongside).
  - criterion-1.5x vs `[aspirational]` → user picked **keep both, treat gate as operative threshold**. **No mid-flight `[aspirational]→[hard]` amendments.**
  - Untracked `dev/bench_results/2026-04-26-gf2-simd.csv` → user picked **delete**. Done.
- **Pre-dispatch criterion audit:**
  - B1's `[hard]` baseline-name literal (`ppc-v0-2026-04-25`) is stale placeholder; canonical is `ppc-v0-2026-04-27`. Per "gate as operative threshold," routed clarification through the dispatch prompt (no JIT description amendment).
  - C3 / C4 issue Notes sections name benches that disagree with the manifest. Manifest is the canonical reference; routed via dispatch prompts.
- **Cleanup:** removed 4 stale closed-issue worktrees (5223bb04, 8e4b189c, cad241e6, d4740d85) via `git worktree remove`. Other locked worktrees (`a07deebd`, `a3150d79`, `a435fc501…`, `a518c2b4…`, `a53e038e`, `a5e4c815`, `a9d50c7b`, `aec4113c`) left untouched.
- **JIT claims** of `ec286cee`, `86c09a51`, `1c1c4242`, `54a0e75c` to `agent:claude-wt-<short-id>`. Committed at `c8d71af` together with the session-6 progress.json update.
- **Dispatch:** 4 parallel workers in background via Agent `isolation: "worktree"`, `subagent_type: general-purpose`. Each prompt included the full PPC-spiral protocol, manifest-canonical clarifications, the c69d2055 hoist lesson, and the c69d2055/e555c46d sse4.1 register-spill trap.
- **All 4 workers terminated with API 529 (auth service unavailable)** after roughly 40 minutes each. Lost the workers' final narrative reports; the on-disk work in each worktree is the source of truth.
- **Lead-preserve commits** on each worktree branch (NOT on main) for C3, B1, B2 to capture the uncommitted WIP so it survives across sessions:
  - C3: `16dbba3` (1034 insertions — full montgomery.rs kernel + asm + perf-stat + bench + dispatch wiring + manifest pin)
  - B1: `d773986` (110 insertions on `matrix.rs` — V7 cache-tile via `MACRO_TILE_BLOCKS=8` outer loop above `TRANSPOSE_CACHE_TILE_THRESHOLD_BLOCKS=16`)
  - B2: `5e7c138` (1007 insertions across 5 files — 288 lines on `alg/m4rm.rs`, bench restructure on `m4rm_components.rs`, new example `examples/m4rm_gray_table_perfstat.rs`, new asm artefact `alg/asm/m4rm.asm.txt`, new perf-stat `dev/bench_results/2026-04-28-b2-perf-stat.txt`)
  - Note: B2's `dev/bench_results/2026-04-28-b2-perf-stat.txt` was found leaked into main's working tree post-dispatch (worker apparently used an absolute path); lead moved it back into B2's worktree and amended the WIP commit to include it. Main's `dev/benchmarks/ppc-baselines.json` was also briefly modified by the C1 worker's leak (matched C1's committed manifest amendment); lead reverted main to HEAD to keep the no-merge invariant. Both leaks were discovered before the handoff commit and recorded under TRAP 6 below.
- **NOT merged.** No worktree work is on `main`. Lead inspected each worktree's `git log` / `git diff main...HEAD` / `git diff` for handoff content; lead did NOT execute any `cargo`/`cargo nextest`/`./scripts/verify-lean.sh` against the worker artefacts.

### Per-worker observations from the inspection

**C1 (`ec286cee`)** — looks the most production-ready of the four:
- 4 commits, clean working tree.
- 2506 insertions across 15 files; new `crates/gf2-core/src/gf2m/batch.rs` (290 lines), new `gf2-kernels-simd/src/x86/gf2m_batch.rs` (524 lines), 532-line asm artefact, proptest equivalence tests (312 lines), perf-stat capture, bench leaves added to `gf2m_mul_strategies.rs`.
- Manifest delta updates C1 to `gf2m_mul_crossover/gf2m_batch_unroll4` with a sibling `baseline_bench_target` `gf2m_mul_crossover/pclmulqdq_barrett_loop_v0` (current-vs-current). The amendment-note explains apples-to-apples reasoning. Reasonable.
- **Spec-compliance concern:** the issue requires "one commit per spiral step (V3 / V4)"; the worker shipped `V3+V4` in a single commit (`6c51893`). Reviewer Tier 2 may flag this as `[hard]` non-compliant; lead should decide whether to ask the worker to split in rework, or accept on substance.
- **Untriaged scope question:** the manifest delta also reverts A2 (and other Wave-3 amendments). That is **not a worker scope violation** — see TRAP 1 below; it is a stale-base artefact. Integration must NOT take the worker's manifest verbatim; it must rebase the worker onto current main first.

**C3 (`86c09a51`)** — only V0 committed cleanly:
- 1 clean commit (V0 baseline pin) + 1 lead-preserve WIP (`16dbba3`) holding the bulk of the SIMD implementation.
- Untracked-then-preserved files: `crates/gf2-kernels-simd/src/montgomery.rs` (186 lines), `crates/gf2-kernels-simd/src/x86/montgomery.rs` (467 lines), asm artefact (179 lines), perf-stat capture (25 lines).
- Modified files in WIP: `crates/gf2-core/src/gfp/simd_ops.rs` (+64 dispatch wiring), `crates/gf2-core/src/lib.rs` (+29 — likely `maybe_montgomery` accessor), `crates/gf2-core/Cargo.toml`, registry edits in both kernel-simd `lib.rs`/`x86/mod.rs`.
- **Lean was NOT verified.** The worker did not commit a `proofs/` re-extraction nor a record of `./scripts/verify-lean.sh` passing. The C3 issue carries a `[hard]` Lean-build criterion — this is currently unsatisfied.
- **Spec compliance issue:** issue requires "one commit per spiral step." Worker has V0 only as a clean step; V3 (32×4 limb decomposition) is bundled into the lead-preserve WIP, not isolated as a spiral commit.
- **TRAP 1 risk is high here**: cad241e6 already shipped `gf2-kernels-simd/src/fp_generic.rs` + `x86/fp_generic.rs` as a sibling generic-Fp SIMD path on main. The worker's new `montgomery.rs`/`x86/montgomery.rs` does not reference that path. On rebase the worker may collide with or duplicate the cad241e6 generic kernel. Manual integration review is essential.

**B1 (`1c1c4242`)** — strong V3/V4 result, V7 in WIP:
- 4 clean commits: V0 baseline pin (`a6b4d0f`), manifest commit-hash pin (`785930e`), V4 Hacker's Delight bit-twiddle (`5887148`), V3 AVX2 YMM-wide (`6444afd`).
- `BitMatrix::transpose` is wired to dispatch via `crate::simd::maybe_transpose()` with scalar fallback (`gf2_kernels_simd::transpose::transpose_64x64_scalar`). Wiring lives in `crates/gf2-core/src/matrix.rs:881–898`.
- V3 commit message reports criterion median speedups against `ppc-v0-2026-04-27`: **n=64 → ~15×; n=256 → ~83×; n=1024 → ~88×; n=4096 → ~65×**. The aspirational `≥10×` at n=4096 is met (and the criterion-1.5x gate threshold is comfortably exceeded). These are worker-reported numbers, not lead-verified.
- WIP commit (`d773986`) adds V7 cache-tiling to `transpose_blocked` via `MACRO_TILE_BLOCKS=8` outer loop, gated by `TRANSPOSE_CACHE_TILE_THRESHOLD_BLOCKS=16`. Code reads consistent but is unverified — neither cargo-ci nor the bench has been re-run.
- **Spec-compliance:** issue requires `[hard]` "one commit per spiral step (V4 / V3 / V7)." V4 and V3 are clean; V7 is in WIP. Need to either commit V7 cleanly in rework or escalate.
- **Stale-baseline-name handled at dispatch:** worker used `ppc-v0-2026-04-27` per lead clarification. The literal `ppc-v0-2026-04-25` in the `[hard]` criterion text is unchanged in JIT. Reviewer may flag this; per session-6 user direction (gate as operative threshold) the lead expects the reviewer to read intent, not literal date.

**B2 (`54a0e75c`)** — most at-risk; zero clean commits:
- All work is in a single lead-preserve WIP commit (`5e7c138`).
- Touches: 288-line addition to `crates/gf2-core/src/alg/m4rm.rs`, 44-line restructure of `crates/gf2-core/benches/m4rm_components.rs` (preserves `gray_table_generation_k8` leaf), new example `crates/gf2-core/examples/m4rm_gray_table_perfstat.rs`, new asm artefact `crates/gf2-core/src/alg/asm/m4rm.asm.txt` (note: irregular path — convention is `crates/gf2-kernels-simd/src/x86/asm/`).
- **Unsafe-isolation invariant: PRESERVED.** `grep unsafe` on `alg/m4rm.rs` returned no hits. The 288-line addition is auto-vectorisable scalar (the file's prelude doc says "under `target_feature = avx2` the same loop body" — relying on rustc auto-vec). The c69d2055 hoist is reused (`maybe_simd()` for `xor_fn`). No new SIMD module needed.
- **Asm artefact path is irregular**: lives under `gf2-core/src/alg/asm/` not `gf2-kernels-simd/src/x86/asm/`. The `asm-artefact-present` gate may not see it. Reviewer or rework will likely require the artefact be moved.
- **Spec-compliance:** zero clean commits. Issue requires multiple `[hard]` deliverables (V2 ILP, V3 SIMD, V6 prefetch); they are bundled into one WIP commit with no spiral-step separation. Will need a full re-commit with proper splitting on rework, or fresh re-dispatch.

## What to do next

In priority order:

- [ ] **Resolve TRAP 1 first (worktree base mismatch).** All 4 worker branches' merge-base with main is `854b37d` (session-3 close). Main is now at `c8d71af`, 41 commits ahead. Each worker branch is therefore stale-based and naïve `git merge` would clobber Wave-3 closures. For each surviving worker:
  1. `git -C <worktree> fetch . main` (no-op for shared worktrees), then `git rebase main` from inside the worktree branch.
  2. Resolve conflicts. Expected hot spots:
     - C3: `crates/gf2-kernels-simd/src/lib.rs`, `src/x86/mod.rs`, `crates/gf2-core/src/gfp/simd_ops.rs` — **collisions with cad241e6's `fp_generic.rs`**. Likely require manual unification.
     - B1: `crates/gf2-core/src/matrix.rs` — collision with main's matvec/matmul wiring (8e4b189c, 5223bb04). Worker's transpose work is in disjoint methods; conflicts should be small.
     - B2: `crates/gf2-core/src/alg/m4rm.rs` — collision with c69d2055's dispatch hoist (already at file `73 lines` modified on main). Worker's 288-line addition needs to merge with the hoisted XOR dispatch carefully.
     - All: `dev/benchmarks/ppc-baselines.json` — worker manifests show pre-A2-amendment shape; main has the post-amendment shape. Take main's A2 entry, then layer the worker's per-kernel updates (B1/B2/C1/C3) on top.
- [ ] **Decide salvage vs re-dispatch per worker.** Recommended:
  - C1: salvage. Rebase, run cargo-ci + asm gate; if Tier 2 review accepts the V3+V4-combined commit, close after rework on whatever the reviewer flags. If reviewer demands V3/V4 split, dispatch a small rework.
  - B1: salvage. Rebase, commit V7 cleanly (drop the `wip(jit:1c1c4242)` preserve commit and replace with a proper `perf(jit:1c1c4242): V7 — cache-tiled outer loop` commit). Run cargo-ci + bench geomean.
  - C3: heavy review or restart. Lean compliance is unverified, V3 is in a WIP blob, and there's a known structural collision with cad241e6's fp_generic. Probably re-dispatch with the cad241e6 fp_generic codepath cited as a starting point ("integrate, don't duplicate") — or split into two issues: one for the kernel, one for `MontgomeryFns` wiring.
  - B2: re-dispatch. Zero clean commits, irregular asm artefact path, no per-spiral-step separation. The 288-line `alg/m4rm.rs` blob may contain useful material but its structure isn't recoverable without the worker's rationale narrative (which the API 529 swallowed). Cleaner to restart with the lead-preserve WIP attached as a reference doc.
- [ ] **Validate the user's session-6 strategic direction with concrete data once integration completes.** The "gate as operative threshold" stance assumes the criterion-1.5x gate is reachable on real hardware for each kernel. B1's V3 hit ≥65× — comfortably gate-passing. C1 / C3 / B2 numbers are not yet verified; if any of them cannot pass the gate after a clean rebase, return to the user for amendment guidance per `feedback_pre_dispatch_criterion_audit`.
- [ ] **Sub-wave 4b dispatch (C2, C4, B3).** Wait until 4a worker integration settles. B3 still depends on B1 close. Use what was learned about `isolation: worktree` base SHAs (TRAP 1) to set up dispatch correctly: explicitly rebase or branch from current `main` HEAD before dispatching.
- [ ] **Wave 8 prep.** `59c487c3`/`e7ab802d`/`577b9e7f` block on B3/C3/C1 respectively. Strategy doc `dev/active/085cbd0b-benchmark-gap-strategy.md` already attached. Per the strategy doc, `e7ab802d`'s accumulator-bound design is escalation-worthy when it requires API/trait changes — surface that question to the user when the time comes.

## Traps — do not repeat these

**Carried forward (still binding):**

- All session 1–3 traps remain in force. The most relevant for session 7:
  - Workers do NOT run `jit gate pass/fail/define` or `jit issue update`. Lead does all state transitions.
  - Mid-flight `[hard]` criterion amendments break flow — `feedback_pre_dispatch_criterion_audit` is the response. Audit before dispatch.
  - `~/.cargo/bin/cargo` was a stub historically; rustup reinstalled in `c7e91dfd`. If anything looks suspiciously fast (sub-second cargo-ci), re-verify.
  - `c69d2055`'s description still embeds the falsified ThinLTO claim; per user direction, dispatched as-is. The reviewer may surface it; tell the reviewer it's known.
  - cad241e6 / d4740d85 lessons: Charon→Aeneas extraction is sensitive to dispatch routing. Keep `gfp/` public API stable; route SIMD via existing `simd::maybe_*` pattern.
  - The asm-artefact-present gate is keyed off file paths under `crates/gf2-kernels-simd/src/x86/`. Asm artefacts placed elsewhere (e.g., `gf2-core/src/alg/asm/`) will not satisfy the gate — see B2 above.

**New from session 6:**

1. **TRAP 1 — Agent `isolation: "worktree"` branched from a stale ancestor, NOT from current `main` HEAD.** All 4 worker worktrees in this session (`agent-aabe7a55…`, `agent-ad855fb…`, `agent-a7514de…`, `agent-ad0ba67…`) have merge-base with main = `854b37d` (session-3 close), 41 commits behind `c8d71af`. Workers therefore did not see Wave-3 closures (cad241e6 / c69d2055 / 8e4b189c / 5223bb04 / d4740d85), the A2 manifest amendment, or the session-6 claim commit. Their per-file diffs vs current main include both their own changes AND apparent "reverts" of changes that simply weren't on the worktree branch. **Mitigation for next session:** before dispatching with `isolation: "worktree"`, confirm the worktree's HEAD is at current main with `git -C <worktree-path> log -1` after creation. If the worktree HEAD is not at current main, manually `git rebase main` from inside the worktree before handing off to the worker. Or use manual worktree creation (`git worktree add -b worktree-agent-<id> .claude/worktrees/agent-<id> main`) to guarantee the correct base. The Agent tool's automatic worktree creation should not be trusted to use current HEAD.

2. **TRAP 2 — API 529 mass-failure across all 4 parallel agents.** All 4 workers in this session terminated with `API Error: 529 Authentication service is temporarily unavailable` simultaneously after ~40 min wall-clock each, swallowing their final reports. The on-disk worktree state is the only signal. **Mitigation:** when dispatching multiple parallel workers, design dispatch prompts so workers commit per-spiral-step (and don't bundle final reports into a single closing message). The C1 worker had committed everything before failure, so its final-report loss didn't lose code — only the narrative. The C3 / B2 workers still had substantial uncommitted work because they were saving up commits for the end. **Dispatch-prompt rule for next session: tell workers to commit each spiral step as it's done, do NOT batch commits.**

3. **TRAP 3 — irregular asm-artefact placement.** B2 worker placed asm at `crates/gf2-core/src/alg/asm/m4rm.asm.txt` instead of `crates/gf2-kernels-simd/src/x86/asm/`. The `asm-artefact-present` gate looks for siblings of modified files under `src/x86/`. B2's worker did not modify any `gf2-kernels-simd/src/x86/` files (auto-vec scalar approach), so the gate would vacuously pass — but the artefact won't be discovered. **Dispatch-prompt rule:** when a kernel is auto-vec-only (no `gf2-kernels-simd` SIMD module), the worker still owes an asm artefact, and it should live alongside the modified Rust file (i.e., under `gf2-core/src/<path>.asm.txt` next to the source) AND be referenced from the issue's report. Or change the convention to always require the artefact under `gf2-kernels-simd/src/x86/asm/` even for auto-vec scalar kernels — the reviewer needs to see the lowered SIMD instructions either way.

4. **TRAP 4 — V0 baseline name reuse.** B1 used `ppc-v0-2026-04-27` per the lead's dispatch clarification, even though the literal `[hard]` criterion text says `2026-04-25`. The next reviewer (Tier 2 success-criteria check) may take the literal text at face value and FAIL. If it does, escalate per `feedback_pre_dispatch_criterion_audit`'s sibling pattern (escalate the amendment THEN, rather than fight the literal text). Memory feedback memo updated.

5. **TRAP 5 — bundled spiral-step commits.** C1 shipped V3+V4 in one commit (`6c51893`); the issue text says "one commit per spiral step." This is recurringly seductive when V3 and V4 share a small bit of scaffolding. Reviewer Tier 2 may FAIL. **Dispatch-prompt rule:** add an explicit "one commit per V-step. Do not bundle, even if it feels natural" line to per-task dispatch prompts.

6. **TRAP 6 — workers can leak files into main's working tree.** Despite running under `isolation: "worktree"`, the C1 worker's manifest changes appeared in main's checkout as unstaged modifications (matching the committed C1 worktree state — i.e., the same edit applied to both trees). The B2 worker wrote `dev/bench_results/2026-04-28-b2-perf-stat.txt` directly to main's tree, never to its worktree's `dev/bench_results/`. Cause is unclear — possibly the worker used absolute paths, or `cd`'d to the parent repo at some point, or a script resolved relative paths against `$GIT_DIR/..` rather than the worktree root. **Mitigation for next session:** after every parallel worktree dispatch, explicitly run `git status` on main BEFORE doing any commits or progress-file edits. If main shows worker-source modifications, restore them (`git restore <path>`) and move stray files to the appropriate worktree. The lead-preserve workflow must include this leak-check as a hard step.

## Open questions needing user input

- Question: **Per-worker salvage strategy for the 4 in-flight 4a issues.**
  - Context: All 4 workers branched from a stale base (TRAP 1), API-529 cut their final reports, and 3 of 4 left substantial uncommitted WIP. Lead has preserved the WIP on each worker branch but has NOT integrated.
  - Options:
    A. **Rebase + cargo-ci-per-worker, then re-dispatch any failures.** Most expensive (~4 cargo-ci runs + 4 review cycles), highest fidelity to the workers' effort.
    B. **Rebase C1 + B1 (the two with clean spiral commits), re-dispatch C3 + B2 from scratch (with cad241e6 / c69d2055 cited as integration points).** Mixed — preserves the strong work, restarts the weak work. Lead's recommendation.
    C. **Re-dispatch all four.** Simplest from a session-discipline perspective; loses ~3000 lines of worker effort that *might* salvage.
  - Recommendation: B.
- Question: **How tolerant is the reviewer of the session-6 strategic stance ("gate as operative threshold")?**
  - Context: Workers shipped on the assumption the literal `[hard]` text could be read intent-wise (e.g., B1's stale baseline date; C3's manifest disagreeing with issue Notes). If the AI reviewer takes the literal text at face value, every dispatched task this wave will FAIL Tier 2 on a textual mismatch even when the implementation is sound.
  - Options:
    A. Continue with the gate-as-operative-threshold stance and accept that R0 reviews will surface the literal-vs-intent gap; amend (with single-shot user approvals) when surfaced.
    B. Pre-amend the affected `[hard]` criteria now (B1 baseline date; C3/C4 Notes-section bench names) before re-running 4a reviews. Costs 2–3 amendments now to save 2–3 R0 cycles.
    C. Flag the AI-reviewer prompt itself to read criteria with intent rather than literal text — but that's a `scripts/code-review-prompt.md` change with broader implications, escalation-worthy on its own.
  - Recommendation: B for B1 only (the date is genuinely stale); A for C3/C4 (the Notes-section bench names are informational, not in `[hard]` lines).

## Reference artefacts

- Epic: `jit issue show babcf05e`
- Progress file: `dev/active/babcf05e-progress.json` (refreshed this session)
- Spec doc: `dev/plans/gf2_core_ppc_spiral.md`
- Strategy doc: `dev/active/085cbd0b-benchmark-gap-strategy.md`
- ppc-baselines manifest: `dev/benchmarks/ppc-baselines.json` (canonical reference; main shape, not the worker-divergent shapes)
- Cross-epic handoff chain: `dev/active/bb85c68a-handoff*.md`
- Prior handoffs: `babcf05e-handoff.md`, `babcf05e-handoff-2.md`, `babcf05e-handoff-3.md`

### Session-6 worktree paths and branches

| Issue | Worktree path | Branch | Last commit | WIP-preserve commit |
|---|---|---|---|---|
| `ec286cee` (C1) | `.claude/worktrees/agent-aabe7a5530874a3fa` | `worktree-agent-aabe7a5530874a3fa` | `7d8fd1a` (clean) | none — clean tree |
| `86c09a51` (C3) | `.claude/worktrees/agent-ad855fb0f45a05064` | `worktree-agent-ad855fb0f45a05064` | `9c0b0fc` (V0) | `16dbba3` (V3 WIP) |
| `1c1c4242` (B1) | `.claude/worktrees/agent-a7514de96318093e7` | `worktree-agent-a7514de96318093e7` | `6444afd` (V3) | `d773986` (V7 WIP) |
| `54a0e75c` (B2) | `.claude/worktrees/agent-ad0ba67f63572849c` | `worktree-agent-ad0ba67f63572849c` | (no clean) | `5e7c138` (all work) |

### Key session-6 commits on `main`

- `c8d71af` — claim sub-wave 4a + record session 6 plan in progress.json. **No code changes.**

### Outstanding tooling / housekeeping

- Untracked dev/bench_results/2026-04-26-gf2-simd.csv was deleted per user (session-6 strategic interview); `085cbd0b-benchmark-gap-strategy.md:68` had a "do not delete" note about that file but it was unrelated to the strategy and the user explicitly chose deletion. Future workers reading the strategy doc may notice the comment; not load-bearing.
- Older locked agent worktrees (`a07deebd`, `a3150d79`, `a435fc501…`, `a518c2b4…`, `a53e038e`, `a5e4c815`, `a9d50c7b`, `aec4113c`) still present from past sessions. They are `[locked]` so `git worktree remove` won't reach them. Whether they need cleanup is a pre-existing question; not addressed this session.
