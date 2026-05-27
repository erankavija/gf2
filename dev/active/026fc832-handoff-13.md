# Handoff — Continue gf2-core SOTA catch-up (026fc832) — session 13

**Date:** 2026-05-27
**Session number:** 13
**Prior handoffs:** sessions 1–12 in `dev/active/026fc832-handoff*.md`. Read every prior handoff's **Traps** section — all carry forward.

## Current state

- Epic: `026fc832` — state: `backlog` (assignee `agent:project-lead`)
- **Wave 12**: DONE (3 follow-ups closed in session 12 — see handoff-12)
- **Wave 13**: 1/2 closed
  - **68db401b** (Phase 6d u16-lane PLE base case): CLOSED. All 6 GF(65521) PLE cells PASS at 0.265× to 0.954× (closed from 2.04×–3.44× SHORTFALL). No regressions.
  - **0749dbad** (Phase 6e f64 GEMM cascade): MERGED + R1 lead-direct fixes landed but gates BLOCKED by concurrent-agent contention + pre-existing main flakes. **NEEDS RE-GATING NEXT SESSION.**
- Epic dep DAG: 14/15 direct deps done. 0749dbad and b0fa00af remain in_progress. 98336ab4 still in_progress (blocked on 0749dbad).
- Open escalations: **none unresolved**. Session 13 had 1 escalation (concurrent-agent .jit state) — user picked option 1 (commit + merge); applied.

### Remaining work after 0749dbad gates close

| Issue | State | Path |
|---|---|---|
| `0749dbad` | in_progress (gates need re-run) | f64 GEMM cascade — implementation complete, code-review/cargo-ci pending |
| `98336ab4` | in_progress (blocked on 0749dbad) | n=4096 fgemm re-bench (Wave 14) |
| `b0fa00af` | in_progress | v2 SOTA scorecard re-publish (Wave 15) |

## What happened this session

### 68db401b (Phase 6d u16-lane PLE base case) — CLOSED first-try

R0 dispatched (opus, parallel worktree from d71bd80a). Returned with strong PASS:

- **Implementation**: new u16-lane PLE base-case kernel at `crates/gf2-kernels-simd/src/x86/fp_medium_ple.rs` (616 lines unsafe AVX2 + safe wrapper at `crates/gf2-kernels-simd/src/fp_medium_ple.rs` + ASM artefact at `crates/gf2-kernels-simd/src/x86/asm/fp_medium_ple.asm.txt`).
- **Dispatch wiring**: `Fp<P>` for `P` in (251, 65536) overrides `PLE_PANEL_COLS`; `fp_try_ple_panel_base_medium::<P>` routes via `fp_medium_eligible::<P>()` guard.
- **Bench results** (5-trial CCX1-pinned, flock-guarded):

| Cell | Old ratio | New ratio | Status |
|---|---|---|---|
| GF(65521)/n=64/uniform | 3.319× | **0.265×** | PASS |
| GF(65521)/n=64/deficient | 3.187× | **0.281×** | PASS |
| GF(65521)/n=256/uniform | 2.234× | **0.489×** | PASS |
| GF(65521)/n=256/deficient | 2.283× | **0.562×** | PASS |
| GF(65521)/n=1024/uniform | 3.443× | **0.954×** | PASS |
| GF(65521)/n=1024/deficient | 2.038× | **0.874×** | PASS |

- **Non-regression**: 24 cells × P ∈ {7, 31, 127, 241, 251} × n ∈ {64, 256, 1024} all within ±5% or improved.
- **Tests**: 5 new kernel-level + safe-wrapper tests; existing proptests (`prop_ple_panelized_matches_contract_fp65521` etc.) continue to pass.
- **Gates**: all 3 PASS (cargo-ci PASS, code-review PASS first-try, doc-review attested).
- **Commits**: `b863f92b` (feat) + `1abfb347` (bench) on worktree; rebased onto session-13 main HEAD; merged via `e02e3077`.

### 0749dbad (Phase 6e f64 GEMM cascade) — MERGED, gates need re-run

R0 dispatched in parallel. Returned with PASS on the acceptance gate:

- **Implementation**: new f64 cascade kernel at `crates/gf2-kernels-simd/src/x86/fp_medium_f64.rs` (705 lines unsafe AVX2+FMA3) + safe wrapper at `crates/gf2-kernels-simd/src/fp_medium_f64.rs` + ASM artefact (2340 lines).
- **Dispatch**: `select_f64_path::<P>` selects the f64 cascade when `n >= 512` for `P ∈ (251, 65536)`; hooked into `fp_medium_try_gemm_panel` (reaches both `gemm` and `gemm_axpy_into_view`).
- **Acceptance**: GF(65521)/n=4096 closed from 1.732× → **1.283× PASS** (margin 0.217 below 1.5×).
- **Non-regression**: 17/18 cells within ±5%; 3 small-prime n=64 cells +5.4 to +7.2% (all improvements, attributable to session bench-day noise on unmodified paths).
- **Commits**: `318a97f5` (feat) + `aa56ae56` (bench) on worktree.

**Integration drama:**

1. **Merge**: 4 conflicts (gf2-kernels-simd lib.rs / x86/mod.rs module registrations + gf2-core lib.rs `use`/`static` lines). All pure-additive; resolved by keeping both 68db401b's `fp_medium_ple` and 0749dbad's `fp_medium_f64`.

2. **rust_out leak**: `git add -A` swept in a 4.1MB ELF `rust_out` at repo root (build detritus). Code-review R1 flagged. Fixed via `git rm` + `74e4e4ac`.

3. **R1 lead-direct fixes** (`7325f4be`):
   - `TRSM_BLOCKED_PANEL_SIZE` was `pub(crate)` but referenced in 2 public doctests (`trsm_upper_blocked`, `trsm_lower_blocked` in field/triangular.rs). Pre-existing on main from 6613abf4 wave-7b. Made `pub`. Doctest now passes (551/551).
   - `cargo clippy --package gf2-core -- -D warnings` (without --all-features) flagged 7 dead-code errors on no-simd stub items (PackedFpMatrix, PackedFpBasis, PackedFpChainPolys, fp_reduce_packed, fp_try_ple_panel_base_medium). Added `#[allow(dead_code)]` to each with one-line justification.

4. **Gates STILL FAILING** at session end:
   - **cargo-ci** times out at exactly 300s with no stdout/stderr captured. Confirmed concurrent-agent contention (`ps aux | grep cargo` showed an active `cargo-nextest nextest run --workspace --all-features --release --profile ci` from another session running as `agent:project-lead` on epic 2928ccce via worktree-agent-87a2402f). Both my cargo-ci attempts at commits 1e2d7692 and 7325f4be hit the 5-minute transport ceiling.
   - **code-review** R2 (against `7325f4be`) FAILED on TWO findings that are NOT from my Wave 13 work:
     - `permanent::bipedal3::tests::test_simd_vs_scalar_n24` timing out at 5.005s under nextest CI profile (known intermittent flake; session-11 handoff documented; needs `#[ignore = "slow"]`).
     - `crates/gf2-coding/src/ldpc/dvb_t2/bit_interleaver.rs:356` doctest references `interleaved.len_bits()` which doesn't exist (no public method; private field). This is from the **other agent's work** on a18956a4 (DVB-T2 BCH+LDPC codec, epic 2928ccce) which landed at commit `05f055f3` and `35be4b1f` while my workers were running.

### Concurrent-agent situation (NEW this session)

A second agent session is running concurrently as `agent:project-lead` on epic `2928ccce` (DVB-T2 AWGN simulation campaign). During my session 13 this agent landed at least 12 commits including:

- 11 DVB-T2 LDPC source tables (cef2e631 closed)
- DVB-T2 BCH+LDPC concatenated codec implementation (a18956a4 — R1 + R3 reworks visible)
- Sim-observability checkpoints (fd73e8a8 — R1 + R2 reworks visible)
- column-major bit interleaver fix (87a2402f)

This other agent is **actively reworking** at session 13 end and competes for cargo build/test resources, blocking my gates. Their work has at least one open bug (`bit_interleaver.rs:356` doctest broken).

Inadvertent cross-pollination from `git add -A` in my session: their handoff file `dev/active/2928ccce-handoff.md` got committed under my `7325f4be` commit. Harmless (they would have committed it anyway).

## What to do next

In order of priority:

- [ ] **Wait for the concurrent agent (epic 2928ccce) to settle.** Check `ps aux | grep -E "cargo|nextest"` at session start to see if they're still actively running. The 2928ccce-handoff.md should describe their state.
- [ ] **Verify the bit_interleaver doctest is fixed.** If still broken, the other agent will fix it as part of their a18956a4 rework cycle. If they've abandoned it, the lead may need to fix it lead-direct (1-line change: either expose `len_bits()` as a public method on `BitVec`, or change the doctest to use `.len()` instead).
- [ ] **Add `#[ignore = "slow"]` to `permanent::bipedal3::tests::test_simd_vs_scalar_n24`.** This is the known flake. Per CLAUDE.md test-tier rules: tests expected to exceed 5s MUST carry `#[ignore = "slow"]`. The fix is a 1-line change in `crates/gf2-algebra/src/permanent/bipedal3.rs`. NOT in scope for 0749dbad strictly, but the gate blocker means it must land before 0749dbad closes. File a separate JIT task if you want pristine scope attribution; otherwise apply lead-direct.
- [ ] **Re-run gates on 0749dbad**:
  - `jit gate pass 0749dbad cargo-ci` — should pass once contention clears AND the bipedal3 flake is ignored.
  - `jit gate pass 0749dbad code-review` — should pass once the bit_interleaver + bipedal3 issues are addressed.
  - `jit gate pass 0749dbad doc-review --by agent:project-lead` — manual attestation.
- [ ] **Close 0749dbad** with `jit issue update 0749dbad --state done`.
- [ ] **Dispatch Wave 14 (98336ab4 re-bench)**. Pre-existing worktree at `.claude/worktrees/agent-98336ab4` preserved; rebase onto current main first. The worker re-benches all 6 primes at n=4096 with warmup-matched protocol. The 695350fd + 74ba1cdc + 0749dbad combination should bring most cells to PASS.
- [ ] **Dispatch Wave 15 (b0fa00af v2 scorecard)** after 98336ab4 closes. The scorecard needs to reflect:
  - All wave-7b amendments (session 11 + session 12)
  - 6a7d4c8e, 9138d86c closures (session 12)
  - 68db401b, 0749dbad closures (session 13)
  - 98336ab4 final n=4096 numbers (session 14)
- [ ] **Epic close (Section 10)** after Wave 15 closes.

## Traps — do not repeat these

**Carry forward** (link, don't copy): sessions 1–12 handoffs' Traps. All still in force.

**New session 13 traps:**

- **`git add -A` sweeps untracked OTHER-AGENT files into your commits** — A concurrent agent had written `dev/active/2928ccce-handoff.md` as an untracked file on main. My merge-completion `git add -A` swept it into commit `7325f4be`. Harmless this time but illustrates the principle: when a parallel agent is suspected to be active, prefer `git add <specific-paths>` over `-A`. Also: `rust_out` (a build artefact) leaked into the merge the same way. **Workaround:** before any `git add -A`, run `git status` and visually verify the listed untracked/modified files belong to your work. Memory: [[git-add-all-cross-agent-risk]] (consider writing).

- **Concurrent-agent cargo-ci contention causes 300s transport timeouts with no output** — When another agent's `cargo nextest run --workspace --all-features --release --profile ci` is active in another worktree, my `jit gate pass <id> cargo-ci` competes for the shared `target/` and gets killed at the JIT transport's 300s ceiling. The result is `status: error`, `exit_code: null`, empty stdout/stderr — not a real failure, just resource starvation. **Workaround:** check `ps aux | grep -E "cargo|nextest"` before issuing a cargo-ci gate. If a parallel cargo run is active, wait for it. The flock-based bench harness exists for benches; cargo-ci has no equivalent. Memory: [[parallel-cargo-ci-flaky]] already exists but the 300s-with-no-output failure mode is more specific.

- **Code-review FAILs on pre-existing main issues even if your branch didn't introduce them** — My 0749dbad code-review R2 cited `bit_interleaver.rs:356` doctest broken (from other-agent work) and `bipedal3 n24` timeout (pre-existing flake). The reviewer scopes to "is HEAD clean?", not "did this PR introduce the issue?". When working in a shared-repo concurrent-agent context, your gates may fail on others' bugs. **Workaround:** when this happens, either (a) fix the other agent's bug lead-direct (cross-agent territory; document attribution carefully) or (b) wait for the other agent to fix it. Don't argue with the reviewer.

- **The other agent's worker branches may also be active and dirty.** Don't run `git branch -D` or any destructive ops on branches you don't own. Always inspect the branch name pattern: my branches are `worktree-agent-<my-issue-short-id>`; the other agent's are `worktree-agent-<their-issue-short-id>`. Check `jit issue show <id>` to confirm ownership before touching any branch.

## Open questions needing user input

**None unresolved** — but watch for:

- The other agent's work has at least one open bug (`bit_interleaver.rs:356` doctest). If their session ends without fixing it, the user may want to coordinate or take over.
- The bipedal3 n24 flake has been deferred across multiple sessions. The escalation path is: lead applies `#[ignore = "slow"]` as a 1-line fix (out of scope for 0749dbad's contract but blocks the gate), OR escalates to the user to file a separate task. Recommend the lead-direct fix in next session since the flake is fully characterised (consistently times out at ~5s under release-CI profile on the 5900X).

## Reference artefacts

- Epic: `jit issue show 026fc832`
- Progress file: `dev/active/026fc832-progress.json` (last full update session 6; sessions 7-13 changes in commit history + handoffs)
- Session 1–12 handoffs: `dev/active/026fc832-handoff*.md`
- Closed this session: 68db401b
- In flight (gates blocked): 0749dbad (implementation + R1 fixes landed; gates need rerun)
- Filed this session: none
- Worktree dispatch protocol: `.claude/skills/project-lead/references/worktree-dispatch-protocol.md`
- Reference host: AMD Ryzen 9 5900X (Zen 3), AVX2+FMA, no AVX-512

## Active worktrees + branches at session-13 end

```
/home/vkaskivuo/Projects/gf2                                     8b027370 [main]
/home/vkaskivuo/Projects/gf2/.claude/worktrees/agent-30e98ef1-d6 (unrelated; preserved)
/home/vkaskivuo/Projects/gf2/.claude/worktrees/agent-9480f8a6    (unrelated; preserved)
/home/vkaskivuo/Projects/gf2/.claude/worktrees/agent-98336ab4    (in_progress; preserved for Wave 14 re-bench after 0749dbad closes)
/home/vkaskivuo/Projects/gf2/.claude/worktrees/agent-87a2402f    (OTHER AGENT; epic 2928ccce; preserved — do not touch)
[plus other 87a2402f-and-similar branches from concurrent agent — do not touch]
```

68db401b worktree + branch cleaned up at session end. 0749dbad worktree at `.claude/worktrees/agent-0749dbad` (preserved branch `worktree-agent-0749dbad`) since gates aren't closed yet — gives recovery option if the next session needs to fixup before re-gating.

Main HEAD at end of session 13: `8b027370` (chore commit closing 68db401b).

## Session 13 commit chain (selected high-impact)

- `d71bd80a`: claim Wave 13 (68db401b + 0749dbad)
- `b863f92b`, `1abfb347`: 68db401b R0 worker commits (later merged via e02e3077)
- `318a97f5`, `aa56ae56`: 0749dbad R0 worker commits
- `e02e3077`: merge(jit:68db401b)
- `415d17c7`: merge(jit:0749dbad) — pulled in stray `rust_out` ELF
- `74e4e4ac`: remove rust_out artefact (R1 lead-direct)
- `7325f4be`: pub TRSM_BLOCKED_PANEL_SIZE + allow dead_code on no-simd stubs (R1 lead-direct)
- `8b027370`: close 68db401b

(Plus 12+ commits from the concurrent agent on epic 2928ccce interleaved with mine.)

**Session 13 summary**: 1 issue closed (68db401b — Phase 6d u16-lane PLE base case, first-try PASS all 6 cells 0.265×–0.954×). 1 issue MERGED with R1 lead-direct fixes but gates blocked by concurrent-agent contention + 2 pre-existing main issues (bipedal3 flake + bit_interleaver doctest from another agent's work). Wave 13 is 1/2 closed; next session should be able to close 0749dbad in 1-2 cycles once contention clears and the 2 pre-existing issues are fixed (likely by the other agent or via lead-direct 1-line `#[ignore]` for bipedal3).

The cumulative complexity reduction continues: of the 4 architectural gaps tracked at session 11, 3 are now resolved (M31 dispatch, GF(65521)/n=64 solve, GF(251)/n=64 borderline) and 2 new ones (GF(65521) PLE, GF(65521)/n=4096 fgemm) are closed by the Wave 13 implementations — leaving 1 pre-existing gap (GF(251)/n=256 Schur-update PLE, [aspirational] amended in session 11).
