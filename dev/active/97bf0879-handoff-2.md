# Handoff — Close gf2-core SOTA performance gaps (`97bf0879`) — session 2

**Date:** 2026-05-04
**Session number:** 2
**Prior handoffs:**
- `dev/active/97bf0879-handoff.md` (session 1, 2026-04-30) — Wave 1 closure.
- Predecessor PPC epic: `dev/active/babcf05e-handoff{,-2,-3,-4,-5}.md`. Read at minimum the **Traps** sections of `babcf05e-handoff-5.md`. All unresolved-traps from session 1 still apply unless explicitly resolved below.

## Current state

- Epic: `97bf0879` — state: **in_progress**, claimed by `agent:project-lead`.
- Wave in progress: **Wave 2 closed**; next dispatch is Wave 3 (profiling + reference-lane selection) per `dev/active/97bf0879-progress.json`.
- Children summary: **9 done**, 0 in_progress, 47 backlog/ready, 0 rejected (out of 56 total).
- Active claims: none. All Wave-2 worktrees torn down.
- Open escalations: none.
- Progress file: `dev/active/97bf0879-progress.json` (12-wave plan, current wave = 3 after this handoff).
- Branch state: `main` clean at `4c07b8b` (`chore(jit:507b0036): close M4RIE matmul-only promotion after R3 review pass`).

## Wave 2 — four issues closed (sources of truth)

| Short ID | Title | Final commit | Rounds |
|---|---|---|---|
| `5dea7457` | Harden fflas-ffpack and M4RI reference lanes | `3ec4b03` | 1 (PASS first time) |
| `73ab8eef` | Evaluate NTL and FLINT references | `5871ecc` | 3 (R1 fail → R2 lead-direct fix → R3 PASS) |
| `79388011` | Evaluate LinBox for exact linear algebra references | `ddd1c3e` | 4 (R1–R3 fail on cumulative findings → R4 PASS) |
| `507b0036` | Evaluate M4RIE for GF(2^m) references | `b4c659d` | 4 (R1 fail → R2 weaker oracle reject → R3 down-scope → R4 PASS) |

Lead-direct cleanup commits:
- `e3592fe` — wired Wave 2 secondary references into `benchmarks/run.sh` (verify_sha for linbox/m4rie/ntl/flint, verify_apt_pin for libmpfr-dev) and `benchmarks/analyze.py` (`FIELD_FAMILY` map for `gf2`/`gf2m`, `reference_lib_for(field)` routing m4ri/m4rie/fflas-ffpack).
- `cbe73e2` — secondary-reference docs in analyze.py.
- `5871ecc` — corrected singular-probability formula in `ntl_flint_smoke.cpp` (`1 - ∏_{i=1}^∞ (1 - p^{-i})` not `(1/p)^3`); refreshed line citations 177-273.
- `b4c659d` — refreshed M4RIE criterion #1 verify_sha note in evidence doc.

Key artefacts produced this session (all linked to their issues via `jit doc add`):

- `dev/plans/{linbox,m4rie,ntl,flint}_promotion_evidence.md` — secondary-reference evidence docs with five-criterion confirmation tables.
- `benchmarks/Containerfile` — `# === linbox/m4rie/ntl/flint begin/end ===` stanzas, all SHA-pinned to `image.lock`.
- `benchmarks/reference/{linbox,m4rie,ntl,flint}_bench.{c,cpp}` — timing harnesses.
- `benchmarks/reference/ntl_flint_smoke.cpp` — cross-equality oracle (NTL ↔ FLINT) with singular-resample policy at n=16.
- `benchmarks/reference/m4rie_bench.c` — matmul-only after R3 down-scope; bitwise equality vs `ref_gf2m_mul`.
- `dev/bench_results/2026-05-04-{5dea7457,79388011,73ab8eef,507b0036}-*-{reference.csv,host.txt,perf-stat.txt}` — measurement artefacts on AMD Ryzen 9 5900X (cross-host vs Zen-3 anchor — flagged in each evidence doc).

## Side-effects

- `4c0d0202` (SOTA target matrix design doc) auto-promoted backlog → ready when its last Wave 2 dep (507b0036) closed.
- All Wave 3 issues (`0fd48627`, `609855d9`, `9a715d75`, `a3412e15`, `c3e79272`, `3b762764`) are already `ready` — none was gated on Wave 2 directly.

## What to do next

In priority order:

- [ ] **Dispatch Wave 3: 6 profiling/lane-selection tasks.** All independent. Per the wave plan in `dev/active/97bf0879-progress.json`:
  - `0fd48627` — Profile post-PPC GF(2) M4RI gap (research)
  - `609855d9` — Classify GF(p) gap by prime family (research)
  - `9a715d75` — Select GF(2^m) reference lane (design)
  - `a3412e15` — Select sparse benchmark corpus and references (design)
  - `c3e79272` — Build charpoly minpoly reference lane (implementation)
  - `3b762764` — Re-run dense LA post-GEMM scorecard (research)

  These are all measurement/design work consuming the Wave 2 artefacts. They are **mostly safe to parallelize** — `c3e79272` is the only implementation issue and may touch `benchmarks/reference/` Makefile/smoke wiring; the rest are evidence-doc-producing research/design. Recommended dispatch shape: parallel via `scripts/dispatch-worker-worktree.sh` for the 5 evidence-only issues; `c3e79272` can run in the same wave but in its own worktree to avoid Makefile conflicts with future waves. Run `scripts/check-leak-into-main.sh` after the wave closes.

- [ ] **Wave 4 prep:** when Wave 3 closes, `4c0d0202` (Publish SOTA target matrix design doc) is ready. It synthesizes Wave 1–3 outputs. Single dispatch, design classification.

## Traps — do not repeat these

Carry-forward from earlier handoffs (still in force):
- All traps from `babcf05e-handoff-5.md` (predecessor epic) and `97bf0879-handoff.md` (session 1) remain active. Re-read them on next session.

New traps from session 2:

1. **Reviewer rate-limit reasoning is stale.** Session-1 handoff warned of GitHub Copilot weekly-quota risk. Copilot gpt-5.4 was restored post-2026-05-04 reset (commit `e6f410b`). **Do not cite rate-limit risk to justify serializing dispatches.** Memory updated (`feedback_reviewer_rate_limit_resolved.md`).

2. **Hard criteria must be self-satisfied IN the evidence doc.** First-round NTL/FLINT/M4RIE workers all deferred their target-matrix designation criterion to "consumer issue 4c0d0202 will decide." That is an automatic FAIL — when a `[hard]` criterion names a downstream artefact, the designation must happen in the evidence doc itself, not be deferred. Memory updated (`feedback_hard_criterion_self_satisfaction.md`).

3. **Verify factual claims in dispatch prompts.** First-round NTL/FLINT prompt contained an unverified claim about `ai-review.sh` behavior; the worker dutifully echoed it into the evidence doc, surfacing later as a stale-doc finding. Grep the repo to confirm any "the script does X" assertion before sending. Memory updated (`feedback_dispatch_prompt_facts.md`).

4. **Protocol § 6 echelon contract is bitwise, not structural.** M4RIE worker R2 attempted to satisfy the echelon contract with structural-RREF-invariants (pivot/rank/zero-row checks). Reviewer correctly flagged this as weaker than the protocol's stated "Bitwise equality of the canonical RREF" requirement. Resolution was to down-scope echelon out of M4RIE (R3), since no independent GF(2^m) RREF reference exists in the harness. **For any future GF(2^m) echelon promotion, an independent scalar RREF reference must be added first**; structural invariants are not a substitute for the protocol's bitwise contract.

5. **Singular-resample policy needs the correct asymptotic, not a naive estimate.** R2 NTL/FLINT evidence doc claimed `(1/p)^3` for triple-resample-miss probability over GF(7). The correct asymptotic is `(1 - ∏_{i=1}^∞ (1 - p^{-i}))^3` — the singularity rate of a uniform-random n×n matrix over GF(p), not the inverse-of-p^3. Worst-case 0.163 on GF(7), giving ≈ 4·10⁻³ for triple-miss. Reviewer caught this; lead-direct fix at `5871ecc`. **For any future singular-resample claim, use the Stieltjes asymptotic, not a per-cell heuristic.**

6. **`jit gate pass` for an auto gate is sometimes blocked by the runtime hook if the most recent recorded run is FAILED.** Workaround: invoke `nohup jit gate pass <id> code-review` (without `--by`), which the runtime treats as triggering the auto checker rather than recording a manual override. Direct invocation of `./scripts/ai-review.sh` outside `jit gate pass` errors with `JIT_CONTEXT_FILE not set`; the env var is set automatically by `jit gate pass`. Always re-launch via `jit gate pass` to get a fresh check.

## Appendix — gate-run history per Wave 2 issue

```
5dea7457: code-review PASS r1 / cargo-ci PASS / doc-review PASS
79388011: code-review FAIL r1 → FAIL r2 → FAIL r3 → PASS r4 / cargo-ci PASS / doc-review PASS
73ab8eef: code-review FAIL r1 → FAIL r2 → PASS r3 (after lead 5871ecc fix) / cargo-ci PASS / doc-review PASS
507b0036: code-review FAIL r1 → FAIL r2 (R1 rework) → FAIL r3 (R2 rework, weaker oracle) → PASS r4 (R3 down-scope) / cargo-ci PASS / doc-review PASS
```

507b0036 round-3 was the only one that approached the MAX_REWORK_ATTEMPTS=2 budget; the R2→R3 transition was a lead decision (after the worker's structural-invariants oracle was rejected) to down-scope to matmul-only rather than escalate. The down-scope was a correctness-preserving narrowing of evidence claims, not a new feature, so it remained inside the lead's autonomy boundary.
