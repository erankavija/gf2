# Epic 6efb756b — Session Handoff — 2026-04-16

## State of play

| Item                              | Status                                                                      |
|-----------------------------------|-----------------------------------------------------------------------------|
| Epic `6efb756b`                   | **in_progress**, 3 of 4 direct children done                                |
| `d9db86e6` reference data         | done                                                                        |
| `3ce1cd12` Phase 3 / Fig 7 GLDPC  | done (1024, 646 dimension accepted + documented)                            |
| `831bfc4a` Phase 4 / Figs 8–10    | **done** — all gates green after paper-aligned SOGRAND rerun                |
| `45649554` Phase 2 / Figs 4–6     | **in_progress**, code-review v5 FAIL — residual paper-alignment gap         |

User directive for the next session is explicit: "**Rigorous path.
Success criteria cannot be relaxed. You may consult additional
external references. Proceed retrying. Reset the counter.**"

## What landed this session

| Commit    | What it did                                                                            |
|-----------|----------------------------------------------------------------------------------------|
| `b925b5c` | Phase 4 BCJR first pass (now superseded by SOGRAND rerun)                              |
| `6199244` | **Core algorithmic fix**: 1-line ORBGRAND with ascending-`wt` enumeration + `OneLineIntercept::{Auto,Basic,Fixed}` + per-component `list_bler_stop_threshold` on `OrbGrandConfig`. Turbo-level list-BLER short-circuit removed. Huge paper-alignment improvement. |
| `32c7a54` | Phase 2 Fig 4/6 + Phase 4 Figs 8/9/10 paper-aligned SOGRAND campaign rerun             |
| `e69ed48` | Phase 4 reports rewritten around the SOGRAND rerun (dropped stale BCJR text)           |
| `9bdc7b4` | `list_bler_threshold = 1e-4` added to canonical `phase2_fig5.toml`                     |
| `fcec59d` | Canonical Phase 2 Fig 4/5/6 runs, `min_errors = 100`                                   |
| `3edbe58` | Remaining `list_bler_threshold` doc drift cleaned up                                   |

## Residual gap — what the reviewer keeps flagging

The reviewer's v5 verdict (`.jit/issues/45649554-…json`, most recent
`code-review` run) cites four facts, all correct:

1. **Fig 4 still misses paper** at mid-SNR:
   - 1.5 dB: `0.0181` vs paper `0.00774` → **2.3×**
   - 2.0 dB: `0.00435` vs paper `8.65e-4` → **5.0×**
2. **Fig 5 still misses paper and our own LDPC-SP** at the waterfall:
   - 2.75 dB: product `0.0972` vs paper `0.0161` and LDPC-SP `0.0119`
   - 3.00 dB: product `1.46e-3` vs paper `2.9e-4` and LDPC-SP `1.4e-4`
3. **Fig 5 tail underflows `min_errors=100`** at 3.25/3.50/3.75 dB
   (only 17/11/1 frame errors observed with `max_frames = 100000`).
4. Fig 6 canonical reproduces the paper headline (product beats
   LDPC-NMS and LDPC-SP at every SNR 0–4 dB); that figure is not
   the blocker.

## The most promising unexplored lead: paper's even-code correction

**While preparing this handoff I identified a concrete algorithmic
gap the prior sub-agents and I missed in the earlier probes.**

The paper's SOGRAND reference formulation applies **two** even-code
corrections to the block-APP computation that our implementation
does not:

1. The "not-tested" probability mass `P_notGuess` is initialised to
   `prob_parity(hard_parity, |LLR|)` rather than `1.0`. That is,
   the noise can only realise the parity of the hard decision, so
   the untested pool is constrained to the parity-consistent half
   of the pattern space from the start. Our
   `cumulative_log_prob` is maintained correctly (parity-skipped
   patterns are not accumulated), but the corresponding
   "untested" term we use in the APP denominator,
   `log1mexp(cumulative_log_prob)`, implicitly treats the full
   `1.0` mass as the cap — **over-counting by ≈2× on balanced
   parity distributions**.

2. The codebook-ratio prior in eq. (17) uses `2^-(s − 1)` rather
   than `2^-s` for even codes — a second factor of ≈2. Our
   `log_codebook_ratio(n, k)` always uses the full `s`.

Together these are a ~4× error in the "not-found" weight for even
codes. **eBCH(n, n − 1)-extended codes are always even** (`eBCH(16,11)`,
`eBCH(16,7)`, `eBCH(32,26)`, `eBCH(64,57)`), and the CRC(25,15) we use
for Fig 4 has even parity under its generator polynomial as well.
That aligns with the empirical pattern: Fig 6 (rate 0.19, small `n`)
still works because the prior's absolute magnitude is tiny there;
Fig 5 (rate 0.79, large `n`) is where the ~4× error on the
not-found weight bites hardest — exactly where the reviewer shows
us missing the paper.

Fig 6 also agrees with this hypothesis: very low rate → `2^-s`
is `2^-9 ≈ 0.002`, so even a 4× error on a term that is already
~0.002 in absolute magnitude barely moves the APP. Fig 5's
`2^-s = 2^-7 ≈ 0.008` is 4× larger to start with, and the 4× even-code
error doubles it again, giving a dominant miscalibration right
in the SNR regime where the APP drives extrinsic extraction.

## Recommended next step (counter reset, rigorous path)

1. **Add `log_prob_parity(absl, target_is_odd)`** to
   `crates/gf2-coding/src/grand/orbgrand.rs`, implementing the
   closed form
   `0.5·(1 + ∏ tanh(|L_i|/2))` (even-parity target) or
   `0.5·(1 − ∏)` (odd-parity target). Stable-form log via
   per-bit `log(tanh(|L|/2))`.
2. **Thread a `log_parity_cap` into the ORBGRAND list-BLER stop
   check**: replace `log1mexp(cumulative_log_prob)` with
   `log_cap_minus_exp(cumulative_log_prob, log_parity_cap)` where
   `log_parity_cap = 0.0` for odd codes and
   `log_prob_parity(absl, hard_parity)` for even codes. This
   matches the paper's `P_notGuess` starting point.
3. **Adjust `log_codebook_ratio`** to take an even-code flag and
   subtract `1` from `s = n − k` when the code is even (add
   `ln(2)` to the ratio, mathematically equivalent to the paper's
   `s--`).
4. **Mirror both adjustments in `compute_block_apps`** in
   `crates/gf2-coding/src/grand/sogrand.rs`, where the final APP
   denominator is computed for the SOGRAND soft-output pipeline.
5. **Probe re-measurement.** Rerun
   `cargo run --release --example sogrand_crc_probe`
   against the baseline/aligned pair; if the fix is correct the
   aligned BLER at 1.0 dB should drop noticeably below ~0.10
   without additional changes. Then rerun the `phase2_fig4_verify`
   + `phase2_fig6_verify` sweeps to confirm no regressions, and
   only then the canonical Fig 4 / Fig 5 / Fig 6 campaigns.
6. **Only after the canonical run clears acceptance** for all
   three figures: update
   `dev/active/45649554-paper-alignment-resolution.md`, rerun
   cargo-ci + code-review, transition `45649554` to `done`, then
   close the epic with the completion report at
   `dev/active/6efb756b-completion-report.md`.
7. **Fallback:** if the even-code correction alone does not close
   Fig 5, the remaining tuning knobs (in likelihood order) are
   (a) Pyndiah-style `alpha_final` late-iteration ramp,
   (b) APP-LLR clamp `±20 → ±60` (already verified safe in a
   smoke), (c) extrinsic scaling schedule. A joint sweep over
   these knobs is the mechanical next step after the even-code
   fix; `dev/campaigns/phase2_fig4_verify.toml` is the right
   cheap harness.

## External references worth consulting (without copying code)

- **Duffy, An, Médard 2022** *IEEE Trans. Signal Processing* 70,
  4528–4542 — the prose definition of 1-line ORBGRAND, the auto-IC
  heuristic, and the `P_notGuess` / `prob_parity` initialisation.
- **Yuan, Médard, Galligan, Duffy SO-GRAND** (`~/Projects/so-grand/main.tex`)
  — the paper whose figures we reproduce. §V covers the block-turbo
  composition; Fig 8 caption documents the `L = 4` / list-BLER stop.
- **Condo 2021** *IEEE Globecom* "High-performance low-complexity
  error pattern generation for ORBGRAND decoding" — landslide /
  mountain-build recurrence used by the reference impl.
- `kenrduffy/SOGRAND-C` on GitHub — **non-commercial academic
  license**. Do **not** copy code from it. Read only for
  algorithm-structure sanity; independent re-implementation from
  the paper prose is required.

## Quick-start for the next session

```bash
cd /home/vkaskivuo/Projects/gf2

# 1. Sanity check from the current session's state
git log --oneline -10
jit issue show 45649554 --json | jq '.gates_status'
cargo test --workspace --all-features --release

# 2. The probe + canonical verify harness already exists
cargo run -p gf2-coding --release --example sogrand_crc_probe
cargo run -p gf2-coding --release --all-features --bin sim_runner -- \
    dev/campaigns/phase2_fig4_verify.toml --parallel

# 3. Canonical rerun after the fix is verified
for t in phase2_fig4 phase2_fig6 phase2_fig5; do
    cargo run -p gf2-coding --release --all-features --bin sim_runner -- \
        dev/campaigns/${t}.toml --parallel
done

# 4. Gate reruns (async — each takes 3-10 min)
jit gate pass 45649554 cargo-ci   &
jit gate pass 45649554 code-review &
wait
```

## Open JIT items

- `45649554` still owned by `agent:claude`; gates: `cargo-ci` PASS,
  `code-review` FAIL (v5), `doc-review` PASS, `tdd-reminder` PASS.
- `6efb756b` owned by `agent:project-lead`; will remain in_progress
  until `45649554` closes.
- No stray leases; `jit recover` was clean at start of session and
  no new leases were taken without release.

## Files to re-read on resume

- `dev/active/45649554-paper-alignment-resolution.md` — canonical
  tables + residual-gap analysis.
- `dev/active/6efb756b-completion-report.md` — epic-level summary
  (will need a small edit under "Key autonomous decisions" once
  Phase 2 closes).
- `crates/gf2-coding/src/grand/orbgrand.rs` — where the pattern
  iterator and `list_bler_stop_threshold` now live; also where the
  new `log_prob_parity` helper should land.
- `crates/gf2-coding/src/grand/sogrand.rs` — `compute_block_apps`
  and the `log_codebook_ratio` / `log1mexp` helpers that need the
  even-code corrections.
- `crates/gf2-coding/examples/sogrand_crc_probe.rs` — the
  ready-made probe harness; a quick parity-cap A/B test belongs
  here before the canonical rerun.
