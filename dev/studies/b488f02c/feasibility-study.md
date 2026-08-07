# Feasibility study: permanent-zero-fraction sampling campaign

> **Status:** initialized — capability inventory and analysis skeleton complete;
> measurement sections marked TO MEASURE are the substance of task `b488f02c`.

## 1. Objective

Assess feasibility of an empirical test of the Ghasemi–Gross–Kopparty conjecture
[GGK2025]:

$$
\Pr[\mathrm{per}(A) = 0] = \frac{1}{q} + o(1)
\quad\text{for uniform } A \in \mathbb{F}_q^{n \times n},\ q \text{ odd},
$$

contrasted with the determinant, where $\Pr[\det(A) = 0] = 1/q + \Omega_q(1)$
(for $q=3$: $\lim_n \Pr[\det = 0] = \prod_{i\ge1}(1 - 3^{-i})$-derived value
$\approx 0.4399$). No published numerics exist for the square case. The
deliverable of the campaign is the empirical curve $n \mapsto \Pr[\mathrm{per}(A)=0]$
per $q \in \{3, 5, 7\}$ with confidence intervals, and an assessment of the
convergence shape toward $1/q$.

## 2. Capability inventory (verified in-tree)

- **Kernels**: `gf2-algebra` provides `permanent_bipedal3` / `permanent_bipedal5` /
  `permanent_bipedal7` over packed representations (`Packed5`: 64 lanes per
  u64-triple; `Packed7`: 16 lanes per u64-pair), on CPU scalar, AVX2, and rayon
  batch paths, plus a HIP/ROCm GPU batch dispatcher (`hip` feature, gfx1030-tuned).
- **Measured baselines** (from the gf2-algebra-permanent epic, M16):
  - single-thread AVX2 ~10.6× over the in-tree Rust reference at $n = 36$, $q = 3$
    (AMD Ryzen 9 5900X; `dev/benchmarks/gf2_algebra_permanent/s1_speedup-2026-05-11.csv`);
  - GPU batch 28.65× / 30.32× over CPU-SIMD at $n = 24$ / $28$, batch $M = 256$
    (RX 6950 XT; `dev/benchmarks/gf2_algebra_permanent/s5_gpu_crossover-2026-05-15.csv`).
  These are speedup ratios; absolute matrices/second at campaign-relevant $n$ is
  TO MEASURE (§4).
- **Correctness**: bipedal $\mathbb{F}_3$ add/sub/mul/neg are Lean4-verified against
  `Fp<3>` semantics (proofs/Gf2Algebra); bounded Ryser correctness proof in progress.
- **Randomness**: `gf2-core` has `rand`-feature generators for bit/matrix/field
  types. Whether a *uniform packed $\mathbb{F}_q$ matrix* sampler exists for the
  `gf2-algebra` packed types is TO VERIFY — if absent it is a campaign-blocking gap
  (§5, G1).

## 3. Cost model and reach

Ryser evaluation costs $\Theta(2^n \cdot n)$ primitive packed ops per matrix; each
increment of $n$ doubles per-sample cost. The statistical side: for zero-fraction
$p \approx 1/q$, the standard error after $N$ samples is
$\mathrm{se}(N) = \sqrt{p(1-p)/N}$, so

- $N = 10^5 \Rightarrow \mathrm{se} \approx 1.5\times10^{-3}$ (q=3),
- $N = 10^7 \Rightarrow \mathrm{se} \approx 1.5\times10^{-4}$.

The scientifically binding constraint is not detecting the gross determinant-vs-
permanent gap (visible in hundreds of samples) but resolving the *drift* of
$\Pr[\mathrm{per}=0]$ toward $1/q$ as $n$ grows: the deviation magnitude at each
$n$ is unknown a priori, so the sampling plan must be adaptive — sample until the
CI separates the measured point from both $1/q$ and its neighbors, subject to the
wall-clock budget. Expected practical ceiling from the cost model is roughly
$n \approx 20$–$26$ at $N \ge 10^5$ on available hardware; the exact envelope is
TO MEASURE (§4).

## 4. Measurement plan (TO MEASURE — REQ-01, REQ-02)

| Measurement | Path | Grid | Output |
|---|---|---|---|
| Batched throughput (matrices/s) | CPU scalar / AVX2 / rayon | $q \in \{3,5,7\}$, $n \in \{12, 16, 20, 24, 28\}$ | table + CSV with hardware/version metadata |
| Batched throughput (matrices/s) | HIP GPU, $M \in \{256, 1024\}$ | same grid | table + CSV |
| Derived envelope | — | target $\mathrm{se} \in \{10^{-3}, 10^{-4}\}$ | feasible $(q, n, N)$ under 24 h / 7 d budgets |

## 5. Gap analysis (initial; effort estimates TO COMPLETE — REQ-03)

- **G1 — uniform sampler**: i.i.d. uniform $\mathbb{F}_q^{n\times n}$ generation
  directly into packed layout, seeded, reproducible. Existence TO VERIFY; if
  missing, small (rejection-free packing from a word RNG needs care at lane
  boundaries).
- **G2 — streaming statistics**: zero-fraction accumulator with Wilson/Clopper–
  Pearson CIs, checkpointable across campaign restarts. Missing; small.
- **G3 — campaign integration**: the FEC simulation runner (TOML campaigns,
  checkpointing, CSV/JSON) is coding-oriented; either adapt it or write a thin
  dedicated driver in `dev/research/`. Decision TO MAKE.
- **G4 — dataset format**: versioned dataset layout (per-$q$, per-$n$ counts +
  seeds + environment) for publication alongside the analysis report. Missing;
  small.
- **G5 — rectangular validation mode**: $n \times k$ sampling for the proven
  $\sim k/q^n$ regime (pipeline correctness check, epic REQ-04). Missing; small —
  reuses G1/G2.

## 6. Open questions

- Convergence-shape methodology: what parametric families for the $o(1)$ term
  (e.g. $c/q^{\alpha n}$ vs $c/n^{\beta}$) are distinguishable within the feasible
  $(n, N)$ envelope?
- Is a determinantal companion curve (same samples, $\det$ instead of
  $\mathrm{per}$) worth the marginal cost for the comparison figure (epic REQ-05)?
- GPU numerical path: confirm the HIP kernels expose per-matrix results (not only
  batch aggregates) so zero-counting is exact.

## 7. Recommendation

Go/no-go with campaign design (sampling plan, seeding scheme, storage layout,
analysis method): TO COMPLETE after §4 measurements (REQ-04).

## References

- [GGK2025] Ghasemi, Gross, Kopparty — Permanental Rank versus Determinantal Rank
  of Random Matrices over Finite Fields. APPROX/RANDOM 2025. arXiv:2512.03221;
  ECCC TR25-206.
- [HKS2026] Hunter, Kwan, Sauermann — Permanents of Random Matrices over Finite
  Fields. arXiv:2603.15856.
- [Scheinerman2024] Scheinerman — Fast Computation of Permanents over F_3 via F_2
  Arithmetic. arXiv:2407.20205.
