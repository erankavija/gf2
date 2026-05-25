# Handoff — Continue gf2-core SOTA catch-up (026fc832) — session 4

**Date:** 2026-05-25
**Session number:** 4
**Prior handoffs:** session 1, session 2, session 3 (`dev/active/026fc832-handoff*.md`)

## Current state

- Epic: `026fc832` — state: `backlog` (gated on wave 4 + wave 5; assignee `agent:project-lead`)
- Wave in progress: **wave 4 — 41096af5 claimed but worker NOT yet dispatched** (stop signal received from user)
- Children summary: 10 done (615db3b9, 27bb2f75, 5ce13bae, aaa847cf, 52cce970, a70b1c70, bd9c6e13, 68cdf4c8, 91429c1c, fc182ed5), 1 in_progress (41096af5, claimed by agent:claude but no worker dispatched), 3 backlog (873cbec1, e8a0c47a, b0fa00af — all chained via 41096af5)
- Active claims: epic itself + 41096af5 claimed by agent:claude (lead-claimed; no worker)
- Open escalations: **session-4 escalation RESOLVED** — user picked option 1 (wire route A as default for n≥512 + amend GF(251)/n=256 to [aspirational])
- Progress file: `dev/active/026fc832-progress.json` (still session-3 schema; needs update — wave 3 complete, fc182ed5 done, 41096af5 claimed)

## What just happened

- Resumed session 4; dispatched fc182ed5 (route C integer panel, opus, on main, single worker).
- fc182ed5 returned `a16a477c` with first-try PASS on all 3 gates (no rework needed — first issue this entire epic to PASS first try):
  - Goto/BLIS 4×24×256 panel kernel; AtomicBool toggle pattern reused from 68cdf4c8 R1
  - GF(251) cells: n=64 −17% REGRESSION vs Candidate C; n=256 0.499 SHORTFALL; n=1024 0.540 SHORTFALL
  - **Decisive: no in-Rust route uniformly clears 1.5× of fflas at GF(251); only route A clears at n=1024**
- Wave 3 closed: 3/3 prototypes done.
- Pre-dispatch criterion audit on 41096af5's SC#7 — the decision rule's "neither clears" branch fires. Escalated to user.
- User chose option 1: wire route A as default for GF(251)/n≥512; keep Candidate C for n<512; amend GF(251)/n=256 to [aspirational]; route B research-only (already in dev/research/); route C dormant behind set_route_c_gf251_enabled (Wave-5 cleanup decision).
- Claimed 41096af5 (commit `060ee1eb`). **Worker NOT dispatched** — user signaled stop.

## What to do next

In order of priority:

- [ ] **Dispatch 41096af5 with the user's option-1 direction baked in.** The dispatch prompt must include:
  - Decision rule branch: "neither clears" → user-approved option 1 (wire route A for n≥512)
  - Concrete production change: in `crates/gf2-core/src/gfp/simd_ops.rs`, modify `select_f32_path<const P, ...>(_m, _k, n)` to add a rule: if `P == 251 && n >= 512`, return `true` (Candidate F path selected for production at this prime+size). Keep the AtomicBool toggle `set_route_a_gf251_enabled` for explicit override.
  - SC#3's "N_THRESH_PRIME updated" is best read here as a special-case dispatch update (special-case for p=251 + n threshold); do NOT lower N_THRESH_PRIME from 252 (that would route all small primes through Candidate F).
  - Amendment: GF(251)/n=256 to [aspirational] in the appropriate evidence doc (7a106fe4 already marks GF(251) [aspirational] family-wide; the wire-in formalizes this in the production dispatch rule).
  - The decision doc at `dev/bench_results/2026-05-25-41096af5-route-selection-decision.md` must include the side-by-side comparison table from the 3 prototype evidence docs (already aggregated in fc182ed5's evidence § "Sibling-route comparison").
  - Verification: 5-trial CCX1-pinned at GF(251)/n in {64, 256, 1024} after wire-in; n=1024 must clear 0.667 ratio (it does in pre-wire-in route-A bench); n=256 should be within 5% of pre-wire-in Candidate C (since the new dispatch routes n=256 → Candidate C). GF(7)/GF(31)/GF(127) at n in {256, 1024} non-regression (delta ≤ 5%).
  - Model: sonnet — production-code change is small (one dispatch rule), bench is mechanical, decision-doc is comparison-table-from-existing-evidence.
- [ ] Review fc182ed5 closure precedent — first-try PASS happened because the prompt explicitly listed every trap from prior sessions. Replicate that pattern for 41096af5.
- [ ] After 41096af5 closes: dispatch wave 5 in parallel via worktrees (3 issues: e8a0c47a Phase 2 GF(p) generalization, 873cbec1 Phase 4 ext-field GEMM design, b0fa00af Phase 5 terminal scorecard). All three depend on 41096af5.
- [ ] Epic close (Section 10).

## Traps — do not repeat these

**Carry forward** (link, don't copy): session 1, 2, 3 handoffs' Traps sections. All carry forward.

**New session-4 traps:**

- **The `select_f32_path` dispatch in `gfp/simd_ops.rs` currently ignores `_m`, `_k`, `_n` (all underscored).** When wiring route A as default for n≥512, the signature must change to use `n` (drop the underscore). Verify the worker actually consults `n` — a dispatch rule that ignores `n` would route either everything-or-nothing.

- **`N_THRESH_PRIME = 252` is workspace-wide.** Lowering it from 252 to 251 to enable route A for GF(251) would also route GF(241)/GF(127)/GF(31)/GF(7) through Candidate F. SC#3 says "N_THRESH_PRIME updated" but the cleanest fix is a **special-case** dispatch rule for `P == 251 && n >= 512` alongside the existing N_THRESH_PRIME comparison. Don't lower N_THRESH_PRIME globally.

- **The user's option-1 amendment ("GF(251)/n=256 to [aspirational]") is a no-op in JIT issue scope** — there's no JIT issue with a [hard] criterion specifically on GF(251)/n=256. The 7a106fe4 evidence doc already marks GF(251) family-wide as [aspirational] (per session-1 close of 615db3b9). The amendment is the production-dispatch logic recording the [aspirational] status as the default behavior for n<512.

## Open questions needing user input

None unresolved. The session-4 escalation was resolved (route A wire-in for n≥512 + GF(251)/n=256 [aspirational]).

## Reference artefacts

- Epic: `jit issue show 026fc832`
- Session-1/2/3 handoffs: `dev/active/026fc832-handoff*.md`
- All 3 prototype evidence docs:
  - `dev/bench_results/2026-05-24-68cdf4c8-route-a-f32-cascade.md` (route A: n=1024 PASS 0.679, n=256 SHORTFALL 0.547)
  - `dev/bench_results/2026-05-24-91429c1c-route-b-blas.md` (route B: both SHORTFALL, research-only)
  - `dev/bench_results/2026-05-24-fc182ed5-route-c-integer-panel.md` (route C: both SHORTFALL + n=64 −17% REGRESSION)
- Phase 0 baseline: `dev/bench_results/2026-05-24-a70b1c70-phase0-controls.md`
- Predecessor scorecard (to be superseded by Wave-5 b0fa00af): `dev/bench_results/2026-05-08-2cfc4372-sota-scorecard.md`
- Worktree dispatch protocol: `.claude/skills/project-lead/references/worktree-dispatch-protocol.md`
- Reference host: AMD Ryzen 9 5900X (Zen 3), AVX2+BMI2+VAES+VPCLMULQDQ, no AVX-512
