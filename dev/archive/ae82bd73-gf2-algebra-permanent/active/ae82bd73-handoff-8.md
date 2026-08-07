# ae82bd73 (Permanents F_3/F_5/F_7) — session 6 handoff

**Date:** 2026-05-12 (session 6 close)
**Branch:** main (HEAD `3fdf45f3`)
**Session focus:** close W3 sub-wave 3b's S2; surface S1's measured-vs-criterion gap; queue S3 amendment

## What landed this session

- **S2 (4513209c — Parallel scaling 1..N cores at ≥0.85x linear, n≥28)** — DONE after 2 code-review rounds. Re-shaped after round 1 from "single matrix, K timings" to "K independent matrices, paired per-matrix scaling factor" with Student-t 95% CI on the mean. Worst lower CI bound: 0.8665 at n=28, T=12 — still above 0.85. Determinism verified: bit-identical `Fp<3>` output across all thread counts at every seeded matrix. Three artefacts attached: `crates/gf2-algebra/examples/parallel_scaling_sweep.rs`, `dev/benchmarks/gf2_algebra_permanent/s2_parallel_scaling-2026-05-11.csv`, `dev/plans/s2_parallel_scaling.md`. Also pulled `today_yyyy_mm_dd` + `unix_secs_to_ymd` into `gf2_algebra::testutil` (SSOT — was triplicated across three examples).

- **S1 (c98ed603 — 50x ST speedup vs T8 at n=36)** — IN PROGRESS. Worker dispatched with the user-approved "dedicated S1 bench file" shape. Measured ratios on the dev host (Ryzen 9 5900X, AVX2-only):
  - n=24 (Criterion, 10 samples): 6.89×
  - n=28 (Criterion, 10 samples): 8.01×
  - n=32 (offline, 1 sample):     9.10×
  - n=36 (in-progress, kicked off this session): expected 2-3 hr ref + ~15 min bipedal3.
  
  Extrapolating the n=24/28/32 geometric trend (ratios growing 1.16× per +4 in n), n=36 lands around **~10.6×**, not 50×. **The headline epic criterion is empirically falsified by ~5× on the Rust-vs-Rust comparison.** The 50× and the paper's 86.9× figures are Julia-vs-bipedal — i.e. they include the Julia JIT/GC overhead vs Rust, which inflates the ratio. The Rust `permanent_mod3_reference` is itself much faster than the Julia reference, leaving only the bipedal encoding's ~10× constant-factor advantage.

  **User escalation (this session):** asked "How should we resolve?" with three options. User answered "How about GPU?" plus "Yes, run n=36 ref now". Interpretation: ship the measured CPU number honestly; pivot the 50× target to a GPU contender follow-up (W5's HIP F_3 kernel `ad55b777` is the natural home).

## Status

**W2** — DONE (prior session)
**W3 sub-wave 3a** — DONE (T12, T14, T15) — prior session
**T13 (cross-wave SIMD dispatch)** — DONE — prior session
**W3 sub-wave 3b:**
- S1 (c98ed603) — IN PROGRESS. n=36 measurement running in background (~2-3 hr). After it finishes, criterion 2 needs amendment from "≥50×" to the measured value with an aspirational follow-up to W5/GPU for the 50× target.
- S2 (4513209c) — DONE this session.
- S3 (363556e6) — open escalation: dev host is AVX2-only; user direction is "Defer AVX-512 to 7f809931. We can only do AVX2 at the moment." Scope-down: AVX2-only sweep with one-host CSV, AVX-512 row [aspirational] pointing at 7f809931.

**W4 (F_5 / F_7)** — pending; `6917eb85` (F_5 packed), `56c5dabc` (F_7 packed), `1f769232` (SIMD F_5+F_7) all ready.

**W5 (GPU HIP)** — pending; `ad55b777` (F_3 HIP kernel) is the natural home for the 50× GPU contender follow-up.

**W6 (Lean)** — pending; 4 issues, 2 with approved sketches.

**W7 (Reporting)** — pending; 5 issues for final epic artefacts.

## Open escalations / decisions for next session

### Escalation A — S1 50× criterion amendment

Per the data and the user's "How about GPU?" cue, the S1 criterion 2 should be amended:

> **From:** `[hard]` Measured speedup ≥ 50× at n=36 on the dev host.
> **To:** `[hard]` Measured speedup of permanent_bipedal3 (SIMD path) over permanent_mod3_reference at n=36 on the dev host is recorded in the CSV with explicit ratio. The aspirational 50× target is pursued via the HIP/ROCm GPU contender (W5 `ad55b777`) — file a follow-up `[hard]` issue `S1g 50× GPU speedup vs T8 at n=36` blocking ad55b777's completion.

Next session: confirm this exact amendment text with the user via `AskUserQuestion`, then update S1's JIT description with neutral phrasing, file the S1g follow-up, attach S1's writeup, transition S1 to done.

### Escalation B — S3 AVX2-only scope

User direction already given: "Defer AVX-512 to 7f809931. We can only do AVX2 at the moment." Next session:
1. Amend S3 (363556e6) criterion 1+2: AVX2-only, with the AVX-512 vs AVX2 throughput ratio criterion deferred to 7f809931 (and made [aspirational] for S3 itself).
2. Run an AVX2-only one-host CSV — single-host data is the entire S3 deliverable in this scoped form.
3. Writeup notes the deferred-to-7f809931 portion with explicit issue-link.

## What worked / what to repeat

- **AskUserQuestion for multi-question batches** — S1 (50× gap) + S3 (AVX-512 host) bundled into one user touch. Saves user time vs sequential asks.
- **K-matrix CI refactor in response to one reviewer round** — S2 round 1 surfaced 5 findings (95% CI missing, K=1 vs K matrices, SSOT, two stale-narrative items); single rework round addressing all five passed round 2 cleanly. Pre-emptively grepping for related issues before committing rework saves cycles.
- **`gf2_algebra::testutil` for the date helpers** — pulled `today_yyyy_mm_dd` + `unix_secs_to_ymd` (with full doc-test coverage) into the crate's SSOT module. Three examples now share, no more triplication.

## Traps — do not repeat these

(Carrying forward all traps from handoffs 1–7. New traps from this session:)

- **Trap S1**: A `[hard]` criterion mandating ≥50× CPU-SIMD speedup over a *Rust* reference is a category error if the underlying 50× figure comes from a *Julia* paper. The Julia JIT/GC overhead is a multiplicative factor (~5-8×) on top of the bipedal encoding's pure ~10× constant-factor win. **When sizing a perf criterion against a paper, always identify whether the paper's baseline is Julia/Python/Rust/C/Fortran — and if it's Julia or Python and your baseline is Rust, divide the paper's ratio by ~5-10× to get the realistic Rust-vs-Rust expectation.** The remaining 5-10× should be sought on accelerators (GPU/multi-thread/AVX-512) rather than from a different SIMD-CPU implementation.

- **Trap S2 round 1**: Reviewer flagged "95% CI not computed" — criterion text said "scaling factor ≥ 0.85 within 95% CI" but the code only checked point estimate. **When a criterion says "within X% CI" or "within ε of Y", the gate check MUST compute the CI/tolerance, not the point estimate.** Single-matrix K-timings + Bessel-corrected std + Student-t critical value is the minimum acceptable shape.

- **Trap S2 round 1, finding 3**: SSOT violation in date helpers across three examples (parallel_scaling_sweep, parallel_chunk_sweep, paper_repro_slope) — the previous worker (T15) copy-pasted `today_yyyy_mm_dd` + `unix_secs_to_ymd` into parallel_chunk_sweep from paper_repro_slope; T15 review didn't catch it. **When introducing a new example that touches an existing utility surface (date formatting, CSV header, hardware fingerprint), check whether the utility already exists in `testutil` or could be promoted there. Don't duplicate even if the existing copy looks "example-local".**

- **Trap S1 worker dispatch**: The S1 worker's runtime estimate for n=36 was 10 hours/sample, off by ~4-5× from the realistic 2-3 hour figure. The 10-hour estimate came from naive 2^4 scaling on n=32's 501s (gives ~8000s = 2.2 hr), but the worker also factored in a "10× safety margin" for sample-to-sample variance. **For one-shot benchmarks where a single sample is enough, the variance margin is unnecessary; use the geometric extrapolation directly. 2-3 hours is the right wall-clock estimate for n=36 ref on a 5900X.**

## Active worktrees

None.

## Active background processes

- **bsk0xd53o**: `S1_OFFLINE=1 S1_OFFLINE_MAX_N=36 cargo bench -p gf2-algebra --features "simd test-support" --bench s1_n36_speedup`. Started ~00:30 UTC 2026-05-12. Expected wall-clock 2-3 hr. Output streams to `/tmp/claude-1000/-home-vkaskivuo-Projects-gf2/9525251e-d2e7-46f6-8403-95da8c68b818/tasks/bsk0xd53o.output` and appends to the S1 CSV.

## Session-6 metrics

- **Issues closed:** S2 (4513209c) — 2 review rounds, 0 reworks (lead-direct rewrites for both rounds).
- **Issues in progress:** S1 (c98ed603) — measurement-running, criterion amendment pending.
- **User escalations resolved:** 2 (S2 round-1 amendment via AskUserQuestion; S1 50× gap + S3 AVX-512 via AskUserQuestion).
- **User escalations open:** 1 (S1 criterion-2 amendment text).
- **Tests passing on HEAD:** 322 (gf2-algebra release).

## Next-session priorities

1. **After S1 background run finishes**: read CSV, confirm n=36 ratio, then `AskUserQuestion` for the S1 criterion-2 amendment text and the GPU follow-up shape.
2. Apply S1 amendment via neutral-phrasing `jit issue update --description`.
3. Trigger S1 code-review + criterion-1.5x gates.
4. Move to S3 amendment + AVX2-only run.
5. After S1 + S3 close, W4 (F_5/F_7 packed types) can be dispatched in parallel via worktrees.
6. W5/W6/W7 follow.
