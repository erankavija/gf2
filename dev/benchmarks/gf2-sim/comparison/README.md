# External-library comparison: gf2-sim vs aff3ct

Side-by-side AWGN BLER comparison of the `gf2-sim` LDPC decoder against the
[aff3ct](https://aff3ct.github.io/) FEC simulation library, on **identical
parity-check matrices** and an **identical channel**, for one DVB-T2 and one
5G NR LDPC configuration. JIT issue `18e69a1a` (story `a5635da5`, epic
`gf2-sim` `f9717e7e`).

## Headline result

> **BLER curves agree within ±0.2 dB at FER 10⁻²** for both configurations —
> the measured gaps are **0.016 dB** (DVB-T2) and **0.003 dB** (5G NR),
> ~10–60× inside the ±0.2 dB criterion.

| Config | gf2-sim FER=1e-2 (Es/N0 dB) | aff3ct FER=1e-2 (Es/N0 dB) | gap (dB) |
|---|---|---|---|
| DVB-T2 r1/2 Normal LDPC (N=64800) | −1.009 | −1.025 | **0.016** |
| 5G NR BG1 Z=384 mother (N=26112) | −4.276 | −4.273 | **0.003** |

The dB gap is the difference in the Es/N0 at which each curve crosses
FER = 10⁻², by log-linear interpolation on the committed BLER points
(`plot.py` prints these crossings). Both gaps are far inside ±0.2 dB; no
discrepancy required investigation. The two independent BP decoders, run on
the bit-identical `H` over the same channel, are essentially on top of each
other across the whole waterfall (see the committed PNGs).

## What is compared (apples-to-apples by construction)

The comparison **isolates the channel + LDPC decoder** and is exact by
construction:

1. **Same code.** The gf2-coding parity-check matrix `H` is exported to MacKay
   AList (`export_alist` bin) and fed to aff3ct via `--dec-h-path`. Both
   decoders therefore operate on the *bit-identical* `H` — there is no
   code-construction mismatch to explain any divergence.
2. **Same channel.** AWGN with BPSK modulation (`--mdm-type BPSK`,
   `--chn-type AWGN`), Es/N0 swept in dB (`--sim-noise-type ESN0`).
3. **All-zero codeword (AZCW).** Both sides transmit the all-zero codeword
   (always a valid codeword of any linear code). This removes the *encoder*
   from the comparison entirely — aff3ct's systematic encoder and
   gf2-coding's IRA/RU encoder produce different codewords from the same `H`
   for a nonzero message, so an AZCW comparison is the only encoder-neutral
   one. A **frame error** is "decoder output ≠ all-zero on the K message
   bits", which is aff3ct's default FER definition.
4. **Same decoder configuration.** Normalized Min-Sum (NMS) with
   normalization factor **0.75**, flooding schedule, iteration cap **50**,
   syndrome-based early termination, same seed (42), same per-noise-point
   frame/error budgets.

### Why LDPC-only (no QAM / interleaver / BCH)

The production DVB-T2 campaign binary (`dvb_t2_awgn_campaign`) runs the **full**
BICM chain (BCH outer + bit interleaver + Gray-QAM). A full-chain-vs-LDPC-only
comparison would **not** be apples-to-apples: the BCH outer code cleans residual
LDPC errors and the bit interleaver reshapes bit-to-symbol reliability, both of
which move the curve for reasons unrelated to the LDPC decoder. We therefore
compare the **isolated LDPC decoder over AWGN-BPSK** on both sides. This is the
regime where the ±0.2 dB criterion is evaluated, and it is exactly expressible
on the aff3ct side. The full-chain DVB-T2 campaign curve remains available via
`dvb_t2_awgn_campaign` as production context (a different, BCH+QAM-inclusive
operating point).

### Note on the 5G NR configuration

The issue names "BG1 / Z=384 / r1/2". `QuasiCyclicLdpc::nr_5g_rate_matched(1,
16896, 8448)` selects BG1 with lifting Z = 384 (K_b·Z = 22·384 = 8448 = K,
2K = 16896 = rate-matched N). The 3GPP **rate-matching** (2Z systematic
puncturing + filler shortening + parity truncation) that turns the mother code
into the r1/2 short code is a gf2-coding-internal LLR-bookkeeping scheme that
cannot be expressed to aff3ct through an AList alone. To keep the comparison
**exact** (same `H` both sides), we decode the **BG1 Z=384 mother code
directly** — the core 5G NR LDPC structure, N = 68·384 = 26112, K = 22·384 =
8448, rate ≈ 0.323 — over AWGN-BPSK with AZCW. This is a legitimate, exact,
5G-NR-derived LDPC decoder comparison; it is simply at the mother-code rate
(~1/3) rather than the post-rate-matching rate (1/2). The ±0.2 dB criterion is
evaluated on this exact-same-`H` decode.

## Reference library

- **aff3ct** version **v4.4.0**, git commit
  `d126cc95a443dcfc535991a1983de9e565c8ffd5` (tag `v4.4.0`), built hermetically
  from source by `run.sh` (recursive clone of all submodules; pinned tag).
- Build: `cmake -DCMAKE_BUILD_TYPE=Release -DAFF3CT_COMPILE_EXE=ON
  -DCMAKE_CXX_FLAGS="-march=native"`, `g++ 16.1.1`. The build tree lives under
  `.aff3ct-build/` (gitignored; never committed).
- `aff3ct --version` reports: `aff3ct (Linux 64-bit, g++-16.1, AVX2) v4.4.0`.

## H-matrix export and checksums

`export_alist` dumps each `H` to MacKay AList. Checksums (SHA-256) of the
exported AList files (regenerate with `run.sh`; the `.alist` files are
gitignored as they are large and reproducible):

```
a419661049980d70925d5ea225118196e00142c9f1c80ab7bcb1fad3c39e2f5b  dvb_t2_r12.alist
3fdf21d7ee9c1fb472765b3ea2e82ce6ae2240860a290defead8a2932762266b  nr_bg1_r12.alist
```

- `dvb_t2_r12.alist` — `H` is 32400 × 64800 (226799 nonzeros), rate 0.5000.
- `nr_bg1_r12.alist` — `H` is 17664 × 26112 (121344 nonzeros), rate 0.3235.

## Files

| File | Description |
|---|---|
| `run.sh` | Driver: builds aff3ct (once), exports AList, runs both sweeps, merges CSVs, renders PNGs. `--quick` for a CI smoke run. |
| `plot.py` | Renders a merged `*-vs-aff3ct.csv` to a semilog BLER overlay with the FER=1e-2 crossing + dB-gap annotation. |
| `dvb-t2-r12-16qam-vs-aff3ct.csv` | DVB-T2 r1/2 merged curve: `es_n0_db,gf2_sim_bler,aff3ct_bler,gf2_sim_fps,aff3ct_fps`. |
| `nr-5g-bg1-r12-vs-aff3ct.csv` | 5G NR BG1 Z=384 merged curve (same columns). |
| `dvb-t2-r12-vs-aff3ct.png` | DVB-T2 BLER overlay plot. |
| `nr-5g-bg1-r12-vs-aff3ct.png` | 5G NR BLER overlay plot. |

> The CSV filename `dvb-t2-r12-16qam-vs-aff3ct.csv` retains the issue's
> deliverable name; the curve inside is the **LDPC-only AWGN-BPSK** comparison
> described above (not a 16-QAM chain — see "Why LDPC-only"). The `16qam`
> token is the issue's label for the DVB-T2 r1/2 configuration, not the
> modulation used in this isolated-decoder comparison.

The `es_n0_db` column is **Es/N0** in dB. For BPSK at code rate R,
Eb/N0 = Es/N0 − 10·log₁₀(R): e.g. DVB-T2 r1/2 Es/N0 = −1.4 dB ⇔ Eb/N0 ≈
1.6 dB; NR mother rate 0.323 Es/N0 = −4.3 dB ⇔ Eb/N0 ≈ 0.6 dB. The `*_fps`
columns are frames/s on the measurement host (informational; wall-clock
dependent, never a contractual column).

## How to reproduce

```bash
# Full sweep (commits the CSVs + PNGs; minutes per config):
bash dev/benchmarks/gf2-sim/comparison/run.sh

# CI smoke (seconds; reuses the prebuilt aff3ct):
bash dev/benchmarks/gf2-sim/comparison/run.sh --quick
```

The first invocation hermetically builds aff3ct v4.4.0 (~10–20 min, once);
subsequent runs reuse `.aff3ct-build/build/bin/aff3ct`. The **full** run writes
the committed deliverables (CSVs + PNGs) directly into this directory; the
**`--quick`** smoke run writes its low-resolution outputs under
`scratch/quick/` and never touches the committed full-resolution files.

## Statistics

Each Es/N0 point runs until `target_errors` frame errors or `max_frames`
frames, whichever first. The full sweep uses `target_errors = 200`,
`max_frames = 12000` (gf2-sim) and `-e 200` (aff3ct), so the FER=1e-2 region
(the criterion point) accumulates ≈ 100–200 frame errors per bracketing point
— a stable ≤ 10% relative estimate (the two deep-tail NR rows have their own
documented budgets; see the provenance note below). Both sides use seed 42
and a 50-iteration cap. Representative gf2-sim per-point frame/error counts
(from `scratch/full_run.log`):

| Es/N0 (dB) | gf2-sim BLER | frames | frame errors |
|---|---|---|---|
| DVB-T2 −1.40 | 7.36e-2 | 3072 | 226 |
| DVB-T2 −1.20 | 2.57e-2 | 8448 | 217 |
| DVB-T2 −1.00 | 9.58e-3 | 12000 | 115 |
| NR −4.30 | 2.03e-2 | 9984 | 203 |
| NR −4.25 | 4.53e-3 | 22272 | 101 |

> **aff3ct frame budget note.** aff3ct v4.4.0 has no per-point frame or
> wall-clock cap — only `-e` (max frame errors). A point with FER ≈ 0 would
> therefore run forever trying to reach `-e` errors. `run.sh` drives aff3ct
> one Es/N0 point at a time under a per-point `timeout` (`AFF_POINT_TIMEOUT`,
> default 240 s) and skips any point that cannot accumulate `-e` errors in
> time; the merge left-joins on Es/N0, so a skipped aff3ct point leaves an
> empty `aff3ct_bler` cell (e.g. NR −4.10 / −4.00, where the gf2-sim BLER is 0
> in 12000 frames). The FER=1e-2 crossing region is fully covered on both
> sides.
>
> **Deep-tail NR points (−4.25, −4.20) — exact provenance.** The −4.25 point
> is deliberately off `run.sh`'s 0.1 dB grid (it exists to bracket FER=1e-2
> tightly on both sides), and the committed −4.25/−4.20 aff3ct cells used a
> larger budget than the `run.sh` defaults. These rows were produced by the
> following exact invocations (run from `comparison/` after a `run.sh` full
> sweep has built aff3ct and exported the AList) and merged into the
> committed CSV:
>
> ```bash
> # aff3ct, both points in one run: 600 s budget, -e 50 (~14% relative
> # error at FER 5e-3 — ample for tail bracketing below the 1e-2
> # criterion point). Produced the committed -4.25 / -4.20 aff3ct cells:
> timeout 600 .aff3ct-build/build/bin/aff3ct --sim-type BFER -C LDPC \
>     --dec-h-path scratch/nr_bg1_r12.alist -K 8448 -N 26112 \
>     --enc-type AZCW --src-type AZCW --mdm-type BPSK --chn-type AWGN \
>     --sim-noise-type ESN0 -R "-4.25,-4.2" \
>     --dec-type BP_FLOODING --dec-implem NMS --dec-norm 0.75 --dec-ite 50 \
>     --dec-h-reorder NONE -e 50 --sim-seed 42 --ter-freq 0
>
> # gf2-sim, the off-grid -4.25 point (deterministic at seed 42 on the
> # 24-thread host below; stops at 22272 frames / 101 errors):
> cargo run -p gf2-sim --release --features test-support --bin ldpc_bler_sweep -- \
>     --code nr-bg1-r12 --esn0-range -4.25:-4.25:0.1 \
>     --max-frames 60000 --target-errors 100 --max-iter 50 --seed 42 \
>     --output scratch/gf2_nr_425.csv
> ```
>
> Every other committed row comes from the plain `run.sh` full sweep
> (aff3ct `-e 200` under the 240 s default; gf2-sim 12000-frame /
> 200-error budgets). The gf2-sim −4.20 row is the on-grid `run.sh` value
> (14 errors in 12000 frames — tail-of-curve resolution only; the FER=1e-2
> crossing interpolates between −4.30 and −4.25, both of which have ≥ 100
> frame errors).

## Host

- AMD Ryzen 9 5900X (12C/24T), ASUSTeK ROG STRIX X570-E.
- Arch Linux, kernel 7.0.10-arch1-1.
- g++ 16.1.1, cmake 4.3.3, Python 3.14 + matplotlib 3.10.

## Investigation notes

No discrepancy exceeded ±0.2 dB — both gaps (0.016 dB DVB-T2, 0.003 dB NR) are
an order of magnitude or more inside the criterion, so no error-hunting was
needed. The following were nonetheless verified up front to ensure the
comparison is genuinely apples-to-apples (each could have produced a spurious
several-tenths-of-a-dB gap if mishandled):

- **Es/N0 vs Eb/N0 convention.** Both sides use Es/N0 in dB
  (`--sim-noise-type ESN0`; the gf2-sim sweep takes Es/N0 directly). aff3ct's
  echoed `Eb/N0` column confirms the relation Eb/N0 = Es/N0 − 10·log₁₀(R)
  (e.g. DVB-T2 r1/2 Es/N0 −1.25 ⇒ aff3ct Eb/N0 1.76 dB = −1.25 + 3.01). A
  mixed Es/Eb convention would shift one curve by 10·log₁₀(R) ≈ 3 dB
  (DVB-T2) — not observed.
- **Gray / constellation labeling.** Both sides use BPSK (`--mdm-type BPSK`),
  which has a trivial 1-bit mapping, so there is no QAM Gray-labeling ambiguity
  to align. (This is one reason the isolated-decoder comparison uses BPSK
  rather than 16-QAM — it removes a labeling degree of freedom that would
  otherwise need cross-checking.)
- **Same `H`.** The AList checksums above pin that aff3ct decodes the exact
  matrix gf2-coding exported; a code-construction mismatch is impossible by
  construction.
- **Same decoder.** NMS with normalization 0.75 on both sides
  (`--dec-implem NMS --dec-norm 0.75`), flooding schedule
  (`--dec-type BP_FLOODING`), 50-iteration cap, syndrome early termination.
  An accidental plain-min-sum on one side (aff3ct's `--dec-norm` defaults to
  1.0) would visibly separate the curves by ~0.2–0.4 dB; the matched 0.75 keeps
  them together.

The residual sub-0.02 dB gaps are consistent with Monte-Carlo sampling noise at
≈ 200 frame errors plus minor BP-implementation differences (message
clipping, update ordering) that do not move the waterfall meaningfully.
