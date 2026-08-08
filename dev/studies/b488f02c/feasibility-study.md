# Feasibility study: permanent-zero-fraction sampling campaign

> **Status:** complete. Measurements taken 2026-08-07 on the project benchmark
> host; every number in §4 traces to a committed CSV in this directory.

## 1. Objective

Assess feasibility of an empirical test of the Ghasemi–Gross–Kopparty conjecture
[GGK2025]:

$$
\Pr[\mathrm{per}(A) = 0] = \frac{1}{q} + o(1)
\quad\text{for uniform } A \in \mathbb{F}_q^{n \times n},\ q \text{ odd},
$$

contrasted with the determinant, where $\Pr[\det(A) = 0] = 1/q + \Omega_q(1)$
(for $q=3$: $\lim_n \Pr[\det = 0] = \prod_{i\ge1}(1 - 3^{-i})$-derived value
$\approx 0.4399$). The deliverable of the campaign is the empirical curve
$n \mapsto \Pr[\mathrm{per}(A)=0]$ per $q \in \{3, 5, 7\}$ with confidence
intervals, and an assessment of the convergence shape toward $1/q$.

### 1.1 Prior numerics exist for $q = 3$

The initialized version of this study asserted that "no published numerics exist
for the square case". **That is false**, and the error is material enough to
change the campaign's novelty claim, so it is corrected rather than quietly
dropped. [Scheinerman2024] §4 already reports exactly this measurement for
$q = 3$:

- **Table 3** gives exact counts $z(n) = |\{A \in \mathbb{F}_3^{n \times n} :
  \mathrm{perm}(A) = 0\}|$ by full enumeration for $n \le 5$:
  $z(1) = 1$, $z(2) = 33$, $z(3) = 8\,163$, $z(4) = 17\,116\,353$,
  $z(5) = 317\,193\,401\,763$, with fractions
  $0.3333, 0.4074, 0.4147, 0.3976, 0.3744$.
- **Table 4** gives Monte Carlo zero counts for $6 \le n \le 30$, at $10^{11}$
  trials for $n \le 10$, declining by decades to $10^{6}$ for $n \ge 28$. At
  the sizes this study times: $10^{10}$ trials at $n=12$, $10^{9}$ at $n=16$,
  $10^{8}$ at $n=20$, $10^{7}$ at $n=24$, $10^{6}$ at $n=28$.
- **Conjecture 4.1** is the same statement the campaign tests: "As
  $n \to \infty$ the distribution of $\mathrm{perm}(A)$ for
  $A \in \mathbb{F}_3^{n \times n}$ approaches the uniform distribution.
  Equivalently, $\lim_{n\to\infty} z(n)/3^{n^2} = 1/3$."
- The paper states its own resolution limit: "For $n \le 13$ the proportions are
  statistically distinguishable from $1/3$, but for larger $n$ they are not."
  Its simulations "took about one full day on 128 processors" — roughly
  $3.1 \times 10^3$ processor-hours.

Table 3's small entries are independently derivable, and the harness derives two
of them by brute force as a transcription check: over $\mathbb{F}_3$,
$\mathrm{per}\left(\begin{smallmatrix}a&b\\c&d\end{smallmatrix}\right) = ad+bc$,
and counting solutions of $ad = -bc$ gives $N(0)^2 + 2N(1)N(2) = 25 + 4 + 4 = 33$
out of $81$, matching $z(2) = 33$ and $0.4074$; full enumeration of the $3^9$
matrices of order 3 reproduces $z(3) = 8\,163$.

[Scheinerman2024] is therefore the **prior-art baseline** for $q = 3$: exact for
$n \le 5$, Monte Carlo for $6 \le n \le 30$. §7.6 states this campaign's delta
against it cell by cell, on achieved precision rather than raw trial count. No
published numerics are known to us for $q \in \{5, 7\}$.

## 2. Capability inventory (verified in-tree)

Every claim in this section was checked against the source on 2026-08-07. Line
references are to the state of `crates/` recorded as `deps_source_sha` in every
receipt (`195f8254`), alongside the harness commit `12bb1e3b` that produced the
measurements.

- **F_3**: `permanent_bipedal3` (`permanent/bipedal3.rs:165`) dispatches to a
  scalar single-word kernel or an AVX2 single-word kernel for $n \le 63$, and to
  a multi-word path for $64 \le n \le 255$. `permanent_bipedal3_parallel`
  (`permanent/parallel_bipedal3.rs:102`) splits **one matrix's** Gray-code walk
  across rayon workers in chunks of $2^{16}$ subsets.
- **F_5**: `permanent_bipedal5` (`permanent/bipedal5.rs:108`), scalar only,
  asserts $n \le 63$.
- **F_7**: `permanent_bipedal7` (`permanent/bipedal7.rs:110`), scalar only,
  asserts $n \le \texttt{Packed7::LANES} = 16$.
- **GPU**: `gf2_algebra::gpu::permanent_batch_bipedal{3,5,7}` (`gpu.rs`), behind
  the `hip` feature, assert $1 \le n \le 63$ for all three fields.
- **Correctness**: bipedal $\mathbb{F}_3$ add/sub/mul/neg are Lean4-verified
  against `Fp<3>` semantics (`proofs/Gf2Algebra`); bounded Ryser correctness
  proof in progress.

Three inventory claims inherited from epic `b8206228` do not survive the code
read, and are recorded here rather than silently corrected:

1. The epic describes CPU coverage as "scalar, AVX2, rayon" for the campaign's
   three fields. **AVX2 exists only for $\mathbb{F}_3$**, and **rayon exists only
   for $\mathbb{F}_3$ and only within a single matrix**. `parallel.rs:5-7` states
   that the F_5/F_7 parallel companions "remain a follow-up". There is no
   in-tree rayon path across a *batch* of matrices for any field, which is the
   parallelism a sampling campaign actually needs.
2. Every in-tree permanent entry point is square-only — `permanent_ryser`
   (`permanent/ryser.rs:96-103`) asserts `matrix.len() == n * n`. This does
   *not* contradict the epic's "no new numeric kernels" claim, because
   [GGK2025]'s rectangular event is permanental-rank deficiency, which decomposes
   into square $k \times k$ permanents; see gap G5 for why an earlier reading of
   this study got that wrong in both directions.
3. `permanent_bipedal7`'s $n \le 16$ bound means **$q = 7$ has no CPU path at all
   for $n > 16$**, so the campaign's $q = 7$ arm above $n = 16$ depends on a
   single backend.

**Randomness (gap G1, confirmed missing).** No uniform $\mathbb{F}_q$ matrix
sampler suitable for a published statistic exists in-tree.
`gf2_algebra::testutil::random_matrix` (`testutil.rs:38`) draws
`Lcg::next_u64() % P`. The modulo bias of that reduction is at most
$q \cdot 2^{-64}$ and is immaterial. The generator is the problem:
`gf2_core::rng::Lcg` is the MMIX linear congruential generator modulo $2^{64}$
(`rng.rs:70-76`), whose own module documentation states it is "**not** a
cryptographic RNG … Use `rand`/`rand_chacha` for anything security-sensitive"
(`rng.rs:18-20`). Consecutive LCG outputs lie on a coarse lattice, and one
sampled matrix is $n^2$ *consecutive* draws, so the entries of a single matrix
carry deterministic linear structure. The statistic under study is itself
algebraic, so that structure cannot be assumed harmless.

## 3. Cost model and reach

Ryser evaluation costs $\Theta(2^n \cdot n)$ primitive packed ops per matrix, so
each increment of $n$ roughly doubles per-sample cost. On the statistical side,
for zero-fraction $p$ the standard error after $N$ samples is
$\mathrm{se}(N) = \sqrt{p(1-p)/N}$, so a target standard error $\mathrm{SE}$
needs

$$
N = \left\lceil \frac{p(1-p)}{\mathrm{SE}^2} \right\rceil .
$$

The campaign uses the conjectured $p = 1/q$ as the planning estimate and reports
the conservative $p = 1/2$ column alongside it, since $p(1-p)$ is maximised at
$1/2$ and no proportion needs more samples than that column states.

| target SE | $N$ at $p=1/3$ | $N$ at $p=1/5$ | $N$ at $p=1/7$ | $N$ at $p=1/2$ |
|---|---|---|---|---|
| $10^{-3}$ | 222 223 | 160 000 | 122 449 | 250 000 |
| $10^{-4}$ | 22 222 223 | 16 000 000 | 12 244 898 | 25 000 000 |

The scientifically binding constraint is not detecting the gross
determinant-versus-permanent gap, which is visible in hundreds of samples, but
resolving the *drift* of $\Pr[\mathrm{per}=0]$ toward $1/q$ as $n$ grows. The
deviation magnitude at each $n$ is unknown a priori.

The initialized version of this study proposed to "sample until the CI separates
the measured point from both $1/q$ and its neighbors". **That design is
withdrawn.** Stopping when the interval reaches a desired relation to the value
being estimated is optional stopping on the statistic itself, which biases the
estimate and invalidates the nominal coverage of the reported interval. §7
replaces it with a pre-registered fixed $N$ per cell, chosen from the envelope
before any campaign data is drawn.

## 4. Measurements (REQ-01, REQ-02)

All measurements were taken on 2026-08-07 on the project benchmark host, which
was otherwise idle. Artifacts in this directory, each carrying a preamble with
git SHA, rustc and ROCm versions, CPU and GPU model, governor, thread count, and
the exact invocation:

| Artifact | Contents |
|---|---|
| `equivalence-2026-08-07.csv` | cross-backend per-matrix agreement |
| `throughput-2026-08-07.csv` | the `(q, n, backend)` grid, 105 cells |
| `sustained-2026-08-07.csv` | minutes-scale streaming runs per backend |
| `envelope-2026-08-07.csv` | REQ-02 envelope, incl. the prior-art comparison |
| `zero-fraction-2026-08-07.csv` | pooled zero fractions with Wilson intervals |
| `cargo-tree-2026-08-07.txt` | resolved dependency graph (the harness gitignores `Cargo.lock`) |
| `gpu-hang-2026-08-07.log` | receipt for the GPU fault at $M = 4096$ (§4.5) |

Every CSV preamble records `harness_source_sha` and `harness_source_dirty`
alongside the repository SHA. The harness SHA is the one that matters: the
repository carries `.jit/` workflow state that other agents commit
independently, so a whole-repo dirty flag says nothing about whether the
measured code was committed. **All five receipts were produced by one
executable, `binary_sha256`
`e951b9ccee00531bd501433a49cbda8321fb05e61af294c2c15d68bd384edd33`, built at
harness source commit `12bb1e3b` with `harness_source_dirty: false`,
`deps_source_sha: 195f8254` and `deps_source_dirty: false`.** One commit
post-dates that state — `8e3a7e27`, which rewords a comment in `src/main.rs`
and changes nothing else — and a forced recompile at it produces the
byte-identical executable, checked with `cmp` against the binary that produced
these receipts. The pin therefore covers the harness tip, and a reproduction
built there reproduces these exact stream indices.

`binary_sha256` is what makes the reproduction claim checkable, and it is
load-bearing rather than decorative: a source SHA describes the checkout, while
the hash is taken over the executable that actually ran. The two can disagree —
a checkout advances the moment a commit lands, an executable only when it is
rebuilt — and in this study's own history they did.

Three earlier receipt sets were discarded rather than reinterpreted, and the
last of the three is the reason `binary_sha256` is quoted here:

1. One recorded a whole-repo SHA that predated the harness entirely.
2. One predated the stream-disjointness and warm-up-determinism fixes that §4.7
   shows were load-bearing.
3. One was internally inconsistent about the code that produced it. Its
   equivalence and throughput files recorded `harness_source_sha: 12bb1e3b`
   while carrying the CSV preamble text that `12bb1e3b` had just deleted,
   because the executable was never rebuilt after that commit: the git-derived
   source SHA advanced, the binary did not, and `binary_sha256` correctly
   recorded the older executable. Its sustained, envelope and zero-fraction
   files still recorded the pre-`12bb1e3b` source SHA, its throughput file held
   50 of the 105 cells from an interrupted grid, and its envelope and
   zero-fraction files were derived from a throughput file other than the one
   committed beside them.

That set is superseded by the present one, which was regenerated end to end —
equivalence, grid, sustained, envelope, zero fractions — from the single binary
named above, with each derived stage reading the throughput and sustained files
committed here. The harness stamps provenance faithfully; nothing in it checks
that the executable it is running was built from the source SHA it reports, so
the guard against that failure is regenerating every receipt from one build,
which is what these five are.

Host: AMD Ryzen 9 5900X (12 cores / 24 threads, `powersave` governor), AMD
Radeon RX 6950 XT (gfx1030, 80 compute units), rustc 1.97.0, ROCm/HIP 7.2.

### 4.1 Backend equivalence (precondition)

Before any timing counted as evidence, all six backends were compared per matrix
on shared inputs drawn from a reserved seed stream. **Every backend returned
byte-identical permanents for every matrix**, for $q \in \{3,5,7\}$ at
$n \in \{8, 12, 16, 20\}$ (F_7 to $n = 16$, its kernel bound), 512 matrices per
cell. Backends without a kernel for a given field are recorded with the reason
rather than dropped. This satisfies `@/inv/backend-behavioral-equivalence` for
the paths the campaign would use.

### 4.2 Protocol

Each cell times the **composite campaign hot path** — generate, evaluate,
reduce to a $q$-bin histogram, and append-plus-flush a shard record — not the
kernel alone, and reports the four component times separately. Rates are summed
matrices over summed time; per-repetition rates are never averaged, because the
mean of reciprocals is not the reciprocal of the mean.

- Machine warmed under full rayon load for 90 s before the first cell.
- Per cell: untimed warm-up $\ge 3$ s, then repetitions until both $\ge 5$
  repetitions and $\ge 5$ s of timed work, capped at 120 s. **The minimum is
  applied uniformly except where the cap binds first**; those cells report fewer
  than 5 repetitions and their `rep_sd_s` should be read accordingly.
- Batch size is calibrated per cell from a single-matrix probe to target a 2 s
  repetition, floored at the thread count for the rayon paths and capped at
  65 536; GPU cells use the fixed $M \in \{256, 1024\}$ the task specifies.
- Single-thread cells are pinned to physical core 0 via `sched_setaffinity`;
  rayon cells release the mask to all 24 logical CPUs. The recorded
  `cpu_mhz_mean` is a mean over *all* logical CPUs, so it understates the active
  core's clock on single-thread cells; temperatures are per-package.
- Cell execution order is **randomised within each $n$, then ordered by
  ascending $n$** — a seeded Fisher–Yates shuffle followed by a stable sort on
  $n$; `order_index` records the result. This is *not* full randomisation, and
  the cost is that $n$ correlates with elapsed time. It is deliberate: censoring
  projects from a measured rate at a smaller $n$ on the same
  $(q, \text{backend}, M)$, so a cell must not run before its reference exists,
  and without the ordering a cell with no reference falls back to probing — at
  $q{=}7$, $n{=}28$ that probe alone costs 42 minutes. The residual risk is
  bounded rather than assumed away: the machine is warmed to steady state before
  cell 0, within-stratum order is randomised, and §4.5's sustained runs bound
  drift across a 180 s window below 1 % on every path, so a slow monotone
  trend cannot masquerade as an $n$ effect at the magnitudes here.
- Matrices are drawn by exact rejection from ChaCha20 with per-cell
  domain-separated streams (§2, G1); each cell owns $10^5$ stream indices.

**Forcing the CPU paths.** `permanent_bipedal3` dispatches internally, so the
grid never calls it: `cpu_scalar` calls `permanent_bipedal3_singleword` and
`cpu_avx2` calls `permanent_bipedal3_singleword_simd` with an explicitly
detected AVX2 bundle. F_5 and F_7 have only scalar kernels.

**What a GPU timing includes.** Everything in
`gf2_algebra::gpu::permanent_batch_bipedal*`: host serialisation of the packed
matrices to a row-major byte buffer, two `hipMalloc`s, the H2D copy, the kernel
launch, `hipDeviceSynchronize`, the D2H copy, and both frees — **per call**,
since the dispatcher keeps no persistent device buffers and uses no stream
overlap. Zero counting and shard I/O are timed outside it, in the reduce and
store phases.

### 4.3 Cell outcomes

Of 105 cells: **61 measured, 36 unsupported, 8 censored.** No cell was silently
skipped.

All eight censored cells are GPU cells at $(q, n) \in \{5, 7\} \times \{24, 28\}$,
each projected from its own measured rate at $n = 20$:

| cell | projected rate | implied repetition | verdict |
|---|---|---|---|
| $q{=}5$, $n{=}24$, $M{=}256$ | 1.687 | 152 s | over the 120 s cap |
| $q{=}5$, $n{=}24$, $M{=}1024$ | 3.329 | 308 s | over |
| $q{=}5$, $n{=}28$, $M{=}256$ | 0.0904 | 2832 s | far over |
| $q{=}5$, $n{=}28$, $M{=}1024$ | 0.1784 | 5741 s | far over |
| $q{=}7$, $n{=}24$, $M{=}256$ | 1.457 | 176 s | over |
| $q{=}7$, $n{=}24$, $M{=}1024$ | 3.018 | 339 s | over |
| $q{=}7$, $n{=}28$, $M{=}256$ | 0.0781 | 3279 s | far over |
| $q{=}7$, $n{=}28$, $M{=}1024$ | 0.1617 | 6334 s | far over |

Rates are matrices/second and are **estimates**. Applying the pessimism §4.3
measures does not rescue any of them: the closest, $q{=}5$ at $n{=}24$ and
$M{=}256$, would still miss the 120 s cap. The $n = 24$ cells are marginal and
the $n = 28$ cells miss by one to two orders of magnitude.

**What censoring costs differs sharply between the two fields.** At $q = 5$ it
costs the envelope nothing: CPU batch rayon is measured at every one of those
sizes and is the faster path anyway, so the GPU cell was never going to be
selected. At $q = 7$ it costs everything. `permanent_bipedal7` stops at
$n \le 16$, so $q{=}7$ at $n \in \{24, 28\}$ has **no supported CPU path at
all**, and with both GPU batch sizes censored those two cells carry no rate from
any backend — which is why §4.6 lists them as "no rate" rather than assigning
one. They are the only cells in the grid with no measured throughput on any
path, and the reason the $q = 7$ arm's frontier stops at $n = 20$.

*Unsupported* cells cite the kernel bound that forbids them: F_7 above $n = 16$
(`permanent_bipedal7` asserts $n \le$ `Packed7::LANES`), and the AVX2 and
rayon-intra paths for F_5 and F_7, which `gf2-algebra` does not implement.

*Censored* cells were not attempted at their batch size and **carry no measured
rate**. This section is the study's restatement of the censoring contract; the
normative version lives in the harness's `protocol` module docs and the CSV
preamble repeats it, and the three must not diverge.

**A censored row carries an estimate, not a bound.** Its
`composite_matrices_per_s` is `NaN`. Its `projected_matrices_per_s` is obtained
by scaling a *measured batched rate* from another $n$ on the same
$(q, \text{backend}, \text{batch size})$ through Ryser's exact $n \cdot 2^n$
work model, with the reference size in `projection_reference_n`.

**Two inferences from the single-matrix probe do not work, and measurement
disproves both.** An earlier revision of this study published
$W/\text{probe}$ — compute-unit count over probe time — as an upper bound on the
batched rate. **That was wrong.** On the superseded grid that probed $q = 3$,
$n = 28$, the probe was 28.04 s and $W = 80$, giving 2.85 matrices/s, against a
measured 8.52 matrices/s at $M = 256$ in the same file: the claimed upper bound
was exceeded threefold by a measurement beside it. The current grid measures
**8.53 matrices/s** at that cell and records `probe_matrix_s` as `NaN` there, so
the probe half of the comparison is quoted from the superseded receipt and is
not re-derivable from the committed file. That is a consequence of fixing the
rule rather than a gap in the evidence: a probe is now taken only where a cell
has no projection reference — the smallest $n$ on each
$(q, \text{backend}, M)$ chain, 45 of the 105 cells — and the large-$n$ GPU
cells that motivated the false bound are precisely the ones that no longer pay
for one. The model fails for two compounding
reasons — a compute unit hosts more than one workgroup concurrently, so the
resident-block count is not $W$; and a one-matrix probe pays the entire
per-launch cost (two allocations, both transfers, the synchronisation) that a
real batch amortises over $M$ matrices. The reciprocal $1/\text{probe}$ is no
better in the other direction, understating the device by nearly two orders of
magnitude. **The probe is a latency. No bound on a throughput follows from it,
in either direction.** Recorded per `@/inv/falsification-preserved`.

**What the projection is worth, measured.** Scaling a measured batched rate by
the work ratio can be validated against this grid's own $q = 3$ GPU chain at
$M = 256$:

| $M$ | projection | predicted | measured | error |
|---|---|---|---|---|
| 256 | $n = 20 \rightarrow 24$ | 111.28 | 136.42 | $-18.4\%$ |
| 256 | $n = 24 \rightarrow 28$ | 7.308 | 8.533 | $-14.4\%$ |
| 1024 | $n = 20 \rightarrow 24$ | 253.51 | 308.49 | $-17.8\%$ |
| 1024 | $n = 24 \rightarrow 28$ | 16.526 | 19.271 | $-14.2\%$ |

The projection is consistently **low**, by 14–18 % across both batch sizes and
both steps, because longer kernels amortise per-launch overhead better than the
reference does. This is the figure the CSV preamble defers to rather than
quoting, so a re-measurement cannot leave a stale percentage behind. It is
therefore mildly pessimistic: a cell whose projected rate misses the budget by
an order of magnitude is confidently infeasible, while one that misses by 20 %
is not, and is attempted rather than censored.

**The rule.** A cell is censored when its projected rate implies a repetition
longer than the 120 s per-cell cap — for a fixed-batch cell, $M / \hat R$; for
an adaptive cell, $1 / \hat R$, since adaptive cells size themselves and only a
single unaffordable matrix can defeat them. The cap is checked only *after* a
repetition completes, so without this rule one oversized repetition would run to
the end regardless. A cell with no reference yet falls back to a probe and is
censored only if one matrix alone exceeds the cap — a latency statement, again
carrying no rate.

### 4.4 Measured throughput

Composite matrices/second, best backend per $(q, n)$ shown in bold in the
per-cell CSV. Selected rows (full grid in `throughput-2026-08-07.csv`):

| $q$ | $n$ | scalar | AVX2 | rayon batch | rayon intra | GPU $M{=}256$ | GPU $M{=}1024$ |
|---|---|---|---|---|---|---|---|
| 3 | 12 | 51 030 | 18 340 | 289 970 | 37 540 | 217 400 | **314 490** |
| 3 | 16 | 3 643 | 1 193 | 36 310 | 4 409 | 30 180 | **61 260** |
| 3 | 20 | 229.2 | 74.76 | 2 498 | 2 988 | 2 137 | **4 867** |
| 3 | 24 | 14.35 | 4.639 | 155.8 | 297.6 | 136.4 | **308.5** |
| 3 | 28 | 0.897 | 0.289 | 9.847 | **19.33** | 8.533 | 19.27 |
| 5 | 12 | 6 823 | — | **73 040** | — | 11 430 | 23 290 |
| 5 | 16 | 327.9 | — | **4 163** | — | 617.6 | 1 223 |
| 5 | 20 | 16.42 | — | **218.3** | — | 32.40 | 63.92 |
| 5 | 24 | 0.852 | — | **11.85** | — | censored | censored |
| 5 | 28 | 0.0461 | — | **0.648** | — | censored | censored |
| 7 | 12 | 6 502 | — | **72 480** | — | 10 050 | 21 570 |
| 7 | 16 | 313.1 | — | **3 745** | — | 536.3 | 1 109 |
| 7 | 20 | unsupported | — | unsupported | — | 27.98 | **57.94** |
| 7 | 24 | unsupported | — | unsupported | — | censored | censored |
| 7 | 28 | unsupported | — | unsupported | — | censored | censored |

Four results carry consequences beyond the envelope.

**The public F_3 dispatcher selects the slower path (gap G6).** The scalar
single-word kernel beats the AVX2 single-word kernel by **2.78x-3.10x** at every
$n$ measured — the ratio is smallest at $n=12$ and rises monotonically with $n$,
and the two rates are in the table above — yet `permanent_bipedal3` prefers AVX2
whenever the CPU supports it. The cause is documented in the kernel itself: the SIMD path
zero-pads a single Bipedal3 word into a 4-element AVX2 lane, so three of four
lanes carry no data. This reproduces the ratio already visible in
`dev/benchmarks/gf2_algebra_permanent/s3_cross_cpu-2026-05-12.csv` (scalar at
0.317-0.319 of the AVX2 time), now on an independent harness and RNG.

**The GPU leads at $q=3$ up to $n = 24$, and loses at $q \in \{5,7\}$
throughout.** For $q = 3$ the GPU at $M = 1024$ is the fastest path through
$n = 24$, but its margin over intra-matrix rayon collapses across the top of the
range: 1.63x at $n = 20$, 1.04x at $n = 24$, and **0.997x at $n = 28$**, where
the two are indistinguishable and which one leads is inside the run-to-run
dispersion. For $q = 5$ and $q = 7$ the CPU batch-rayon path wins wherever both
are supported, by roughly a factor of three at every shared $n$ in both fields.
The F_7 GPU kernel is the weakest of the three: its LUT-based arithmetic leaves
it censored above $n = 20$.

An earlier revision of this study reported the opposite at $n = 28$ — that
intra-matrix rayon beat the GPU by 2.3x. **That was an artifact of the
superseded censoring rule**, which declined the $q{=}3$, $n{=}28$, $M{=}1024$
cell and left only $M{=}256$ to compare against. With the corrected rule that
cell is measured, and the table above shows it statistically tied with
intra-matrix rayon. Recorded per
`@/inv/falsification-preserved`; it is the clearest illustration of why a
censoring rule that hides affordable cells is a correctness problem and not a
scheduling convenience.

**This qualifies the 2026-05-15 GPU crossover receipt.** That receipt reports
the GPU beating "CPU SIMD" by 28.65x at $n=24$ and 30.32x at $n=28$ for $q=3$,
both at $M=256$. Those ratios reproduce here — GPU $M{=}256$ over `cpu_avx2`
gives 29.4x at $n=24$ and 29.5x at $n=28$ — but the baseline is the AVX2
single-thread path, which §4.4 has just shown to be the *slower* of the two
single-thread CPU paths, and unparallelised besides. Restated against the best
CPU path measured here, the same $M{=}256$ configuration is **0.46x at $n=24$**
and **0.44x at $n=28$**, and the honest headline is the $M{=}1024$ comparison
above: a 1.04x edge at $n=24$ and a tie at $n=28$, not 30x. This study's
$q=3$, $n=28$, $M=256$ rate agrees with the receipt's 8.490 matrices/s to within
0.5 %, so the two measurements agree where they measure the same thing; the
divergence is entirely in the choice of CPU baseline.

**Generation and I/O are not the bottleneck, but are not free at small $n$.**
The composite rate falls below the eval-only rate by under 2 % at $n \ge 20$,
but by about half at $q=3$, $n=12$ on both the GPU and batch rayon, where
sampling and packing tens of thousands of matrices dominates a sub-millisecond
kernel. Since the campaign's useful cells are the large ones, this shifts no
conclusion, but it is why the envelope is derived from composite rather than
kernel rates.

### 4.5 Sustained throughput

A cell that runs for five seconds can ride a boost window; a campaign runs for
twelve hours and cannot. Each backend family was therefore streamed through the
composite hot path continuously for 180 s, with the rate over the first and last
quarter of the window recorded separately so boost decay is visible rather than
averaged away.

| $q$ | $n$ | backend | $M$ | sustained | grid cell | ratio | first→last quarter |
|---|---|---|---|---|---|---|---|
| 3 | 24 | scalar | 8 | 14.412 | 14.345 | 1.005 | 14.408 → 14.418 |
| 3 | 24 | AVX2 | 4 | 4.701 | 4.639 | 1.014 | 4.700 → 4.702 |
| 3 | 24 | rayon batch | 96 | 157.103 | 155.787 | 1.008 | 157.814 → 156.684 |
| 3 | 24 | rayon intra | 24 | 298.853 | 297.604 | 1.004 | 298.521 → 299.041 |
| 3 | 24 | GPU | 1024 | 311.188 | 308.493 | 1.009 | 311.671 → 310.272 |
| 3 | 24 | GPU | 2048 | 208.474 | — | — | 208.265 → 208.361 |
| 5 | 20 | rayon batch | 96 | 215.230 | 218.260 | 0.986 | 214.198 → 215.969 |
| 5 | 20 | GPU | 1024 | 63.947 | 63.924 | 1.000 | 63.976 → 63.936 |
| 7 | 16 | rayon batch | 512 | 3671.692 | 3745.397 | 0.980 | 3665.113 → 3678.551 |
| 7 | 20 | GPU | 1024 | 57.949 | 57.937 | 1.000 | 57.974 → 57.943 |

Rates are matrices/second; "grid cell" is the same $(q, n, \text{backend})$ cell
from §4.4. The batch sizes coincide only on the GPU rows, where $M$ is fixed by
the schedule; the CPU rows let the grid calibrate its own $M$ against a 2 s
repetition, so the comparison is between a short cell and a long one at the same
backend rather than at an identical batch size. Each run draws from its own
reserved stream range, recorded in the CSV's `stream_first` column, so no two
runs share matrices. Two conclusions.

**The short-cell protocol holds.** Every run lands within 2 % of its grid cell,
and boost decay across a 180 s window never exceeds 0.9 % (largest
first-to-last-quarter drift: 0.83 % on batch rayon). The five-second cells are
not riding a boost window, so the envelope built on them is sound.

**$M = 1024$ is the GPU optimum, not a starting point.** At $q{=}3$, $n{=}24$
doubling the batch to $M = 2048$ costs a **33 % loss** in sustained rate. With
the fault described next, the batch-size question is settled in both directions:
larger is slower before it is dangerous.

**The GPU batch ceiling is a watchdog limit, and it was found by hitting it.**
The task names $M \in \{256, 1024\}$ as starting points, so larger batches were
streamed to test whether they are the ceiling. At $M = 4096$, $q=3$, $n=24$ the
device **hung** — `HW Exception by GPU node-1 … reason :GPU Hang` — and took the
process down with it. The mechanism is straightforward: at $M = 1024$ one launch
occupies the device for about 3.3 s, so $M = 4096$ asks for roughly 13 s of
uninterrupted kernel time on a display-attached card, past its hang detection.

That fault happened in a **superseded** session, before the censoring rework,
and it killed the run before a CSV row could be written. Its only receipt is
therefore `gpu-hang-2026-08-07.log`, committed beside the CSVs, which preserves
the log lines and the post-fault `rocm-smi` state showing the device recovered
on its own. Two things follow for the trustworthiness of everything else here:
the observation is **not reproducible from a committed CSV row**, and is
reported as an operational observation rather than as a measurement; and every
committed number postdates it, taken at harness source commit `12bb1e3b` after
a full cross-backend equivalence re-check passed on all six backends
(`equivalence-2026-08-07.csv`, itself generated after the fault). The
$M = 2048$ row in the sustained table is the surviving in-band probe of the
ceiling.

Three consequences for the campaign, none of which the short cells could have
revealed:

1. **The ceiling on GPU shard size is wall-clock per launch, not memory or
   occupancy.** $M \times n \times n$ bytes is trivial at these sizes; the
   binding constraint is that one launch must finish inside the watchdog.
2. **The constraint tightens with $n$.** Per-launch time scales as
   $M \cdot n \cdot 2^n / W$, so the largest safe $M$ falls by roughly half for
   each increment of $n$. A shard size chosen at $n = 20$ is not safe at
   $n = 24$.
3. **The campaign should cap per-launch time, not batch size**, and pick $M$ per
   $(q, n)$ from the measured per-matrix cost — with a margin, since a hang
   costs the whole in-flight shard and, on this host, the process. This argues
   for the modest shard sizes the storage layout in §7.4 assumes, and against
   the intuition that bigger batches are always better.

### 4.6 Attainable envelope (REQ-02)

Derived in `envelope-2026-08-07.csv` from the best measured **composite** rate at
each $(q, n)$, under a **12 h** wall-clock budget per cell. A 15 % operational
reserve is withheld for checkpointing, dataset compaction, restart after a
failed shard, and residual throttling, leaving $3.672 \times 10^4$ s of
productive compute. Required samples are
$N = \lceil p(1-p)/\mathrm{SE}^2 \rceil$ with the planning estimate $p = 1/q$;
the conservative $p = 1/2$ column is in the CSV.

| $q$ | $n$ | best path | rate | $N$ for SE $10^{-3}$ | hours | $N$ for SE $10^{-4}$ | hours |
|---|---|---|---|---|---|---|---|
| 3 | 12 | GPU $M{=}1024$ | 314 490 | 222 223 | 0.00 | 22 222 223 | 0.02 |
| 3 | 16 | GPU $M{=}1024$ | 61 260 | 222 223 | 0.00 | 22 222 223 | 0.10 |
| 3 | 20 | GPU $M{=}1024$ | 4 867 | 222 223 | 0.01 | 22 222 223 | 1.27 |
| 3 | 24 | GPU $M{=}1024$ | 308.5 | 222 223 | 0.20 | 22 222 223 | 20.01 (x) |
| 3 | 28 | rayon intra | 19.33 | 222 223 | 3.19 | 22 222 223 | 319.3 (x) |
| 5 | 12 | rayon batch | 73 040 | 160 000 | 0.00 | 16 000 000 | 0.06 |
| 5 | 16 | rayon batch | 4 163 | 160 000 | 0.01 | 16 000 000 | 1.07 |
| 5 | 20 | rayon batch | 218.3 | 160 000 | 0.20 | 16 000 000 | 20.36 (x) |
| 5 | 24 | rayon batch | 11.85 | 160 000 | 3.75 | 16 000 000 | 375.0 (x) |
| 5 | 28 | rayon batch | 0.648 | 160 000 | 68.61 (x) | 16 000 000 | 6861 (x) |
| 7 | 12 | rayon batch | 72 480 | 122 449 | 0.00 | 12 244 898 | 0.05 |
| 7 | 16 | rayon batch | 3 745 | 122 449 | 0.01 | 12 244 898 | 0.91 |
| 7 | 20 | GPU $M{=}1024$ | 57.94 | 122 449 | 0.59 | 12 244 898 | 58.71 (x) |
| 7 | 24 | — | no rate | — | — | — | — |
| 7 | 28 | — | no rate | — | — | — | — |

"(x)" marks a cell that does not fit the 12 h budget. **Feasible frontier at
SE $= 10^{-3}$: $q{=}3$ to $n = 28$, $q{=}5$ to $n = 24$, $q{=}7$ to $n = 20$.
At SE $= 10^{-4}$: $q{=}3$ to $n = 20$, $q{=}5$ to $n = 16$, $q{=}7$ to
$n = 16$.** The $q = 7$ cells at $n \in \{24, 28\}$ carry no rate (§4.3); their
projections put them one to two orders of magnitude outside the budget.

One qualification the SE $= 10^{-3}$ column deserves: it is not the interesting
target. At $n = 28$, $q = 3$ a standard error of $10^{-3}$ cannot resolve a
deviation from $1/3$ of the size [Scheinerman2024] reports there
($\approx 4 \times 10^{-4}$), so feasibility in that column buys a measurement
that cannot settle anything on its own.

**Against the prior art.** The envelope CSV carries [Scheinerman2024]'s per-$n$
standard error and 95 % Wilson half-width beside this campaign's, with their
ratio and a classification:

| $n$ | prior SE | prior trials | this budget's SE | ratio | verdict |
|---|---|---|---|---|---|
| 12 | $4.714\times10^{-6}$ | $10^{10}$ | $4.387\times10^{-6}$ | 1.07 | matches |
| 16 | $1.491\times10^{-5}$ | $10^{9}$ | $9.940\times10^{-6}$ | 1.50 | **exceeds** |
| 20 | $4.714\times10^{-5}$ | $10^{8}$ | $3.526\times10^{-5}$ | 1.34 | **exceeds** |
| 24 | $1.491\times10^{-4}$ | $10^{7}$ | $1.401\times10^{-4}$ | 1.06 | matches |
| 28 | $4.715\times10^{-4}$ | $10^{6}$ | $5.595\times10^{-4}$ | 0.84 | below |

A 12 h budget on this host beats the published precision at $n = 16$ and
$n = 20$, matches it at $n = 12$ and $n = 24$, and falls short at $n = 28$.

**No efficiency claim is drawn from that.** It is tempting to note that the
prior work spent about $3.1 \times 10^3$ processor-hours (one day on 128
processors) against this budget's $\approx 288$ thread-hours, and to attribute
the difference to the kernels. That attribution is not available: the paper
reports a processor *count* and no hardware — no model, clock, or year — so the
per-processor throughput is unknown, and the comparison would in any case pit a
CPU-only run against a budget whose best $q=3$ path is a GPU at every $n$. Two
unknowns (their hardware, our CPU/GPU split) sit between the numbers and any
statement about kernel quality. What the table supports is the narrow claim
made: **at these sizes, this budget reaches comparable or better precision**,
with the resource figures given as context rather than as a ratio.

### 4.7 Zero fractions observed in passing

The timing runs evaluate real permanents of uniformly sampled matrices, so they
produce genuine zero counts. These are **a by-product, not a campaign result**:
sample sizes are whatever the timing protocol needed, and no sampling plan was
pre-registered, so they carry intervals and no inference. Pooled per $(q, n)$
across backends — each cell draws from its own reserved stream range, so the
pooled samples are independent — in `zero-fraction-2026-08-07.csv`:

| $q$ | $n$ | matrices | $\hat p$ | 95 % Wilson | $z$ vs $1/q$ |
|---|---|---|---|---|---|
| 3 | 12 | 5 451 883 | 0.33339 | [0.33300, 0.33379] | $+0.29$ |
| 3 | 16 | 813 084 | 0.33400 | [0.33297, 0.33502] | $+1.27$ |
| 3 | 20 | 72 740 | 0.33354 | [0.33013, 0.33698] | $+0.12$ |
| 3 | 24 | 191 437 | 0.33455 | [0.33244, 0.33667] | $+1.13$ |
| 3 | 28 | 5 807 | 0.33666 | [0.32462, 0.34892] | $+0.54$ |
| 5 | 12 | 583 035 | 0.20014 | [0.19911, 0.20117] | $+0.27$ |
| 5 | 16 | 33 817 | 0.20020 | [0.19597, 0.20449] | $+0.09$ |
| 5 | 20 | 58 784 | 0.19982 | [0.19660, 0.20307] | $-0.11$ |
| 5 | 24 | 490 | 0.19592 | [0.16320, 0.23337] | $-0.23$ |
| 5 | 28 | 101 | 0.14852 | [0.09212, 0.23067] | $-1.29$ |
| 7 | 12 | 581 713 | 0.14401 | [0.14311, 0.14491] | $+2.51$ |
| 7 | 16 | 692 258 | 0.14318 | [0.14236, 0.14400] | $+0.76$ |
| 7 | 20 | 17 664 | 0.13779 | [0.13279, 0.14296] | $-1.92$ |

Grid and sustained samples are both pooled. Sustained runs reserve disjoint
stream ranges — recorded per run in the CSV's `stream_first` column — so their
zero counts are independent of one another and poolable, which was *not* true of
the superseded harness (see the retraction below). Disjointness from the *grid*
streams now holds by construction rather than by audit: the grid hands cell $i$
the range $1 + i \times 10^5$ and so cannot reach past $1.05 \times 10^7$ across
its 105 cells, while `SUSTAINED_STREAM_BASE` places the sustained runs at
$10^9 + j \times 10^5$, two orders of magnitude clear. An earlier harness based
the sustained runs at $10^6$, which is commensurate with the grid's allocation:
those ranges collided in index space and stayed disjoint in practice only
because no colliding pair happened to share a $(q, n)$, which had to be checked
cell by cell. §7.3 carries the general form of this requirement into the
campaign.

**A proved lower bound decides how to read this table.** [HKS2026] Theorem 1.3
(arXiv:2603.15856v1, p. 2, eq. 1.2), read first-hand rather than through a
summary, states: *"Fix a finite field $\mathbb{F}_q$ of odd characteristic. For
a uniformly random $n \times n$ matrix $A \in \mathbb{F}_q^{n\times n}$, we
have $\Pr[\mathrm{per}(A) = 0] \ge 1/q$ for all $n$"*, together with
$\Pr[\mathrm{per}(A) = 0] \le 1/q + C/q^3$ for all $n \ge 3$ (eq. 1.4). So a
measurement above $1/q$ is expected, and one whose interval lies strictly below
$1/q$ contradicts a theorem and indicts this pipeline rather than the theorem.

**Every one of the thirteen intervals satisfies the bound**; none lies strictly
below $1/q$. One cell excludes $1/q$ outright — $q{=}7$, $n{=}12$, whose
interval lies wholly *above* $1/7$ — which is the direction the theorem permits
and the upper bound constrains, not a failure. It is also the largest deviation
in either direction at $z = +2.51$, and one excursion past $2.5\sigma$ in 13
cells is close to the $0.16$ expected. Every positive excess sits inside
eq. 1.4's $C/q^3$ envelope, implying $C \le 0.40$ across all cells.

The cell that comes closest to contradicting the theorem is $q{=}7$, $n{=}20$ at
$z = -1.92$: its interval still covers $1/7$, but only just, clearing the bound
by about $10^{-4}$ at its upper edge. It is a by-product sample of 17 664
matrices, the deviation is under $2\sigma$, and the same $(q, n)$ read $z = +1.30$
on the superseded receipts — a swing that independent stream ranges at this
sample size produce routinely. Nothing is inferred from it either way; it is
named because a single cell drifting below $1/q$ is the signature the standing
acceptance test in §7.2 exists to catch, and the campaign will resample this
point under a pre-registered $N$. The pipeline passes its acceptance test at
every measured size.

**An earlier anomaly did not reproduce, and the reason is instructive.** A
superseded run of this study reported three cells beyond $2.9\sigma$ from
$1/q$ — $q{=}5$ at $n = 16$ and $n = 20$ *below* $1/5$, and $q{=}7$ at $n=12$
above $1/7$ — and recommended chasing them as a possible new result. Two
independent defects were behind that, both found by review and both now fixed:

1. **Every sustained run started at the same stream index**, so two runs at one
   $(q, n)$ drew overlapping matrices — the shorter run's sample was a
   stream-prefix of the longer one's. Pooling their zero counts double-counted
   matrices and shrank the intervals. This corrupted exactly the pooled
   $q{=}3$, $n{=}24$ and $q{=}5$, $n{=}20$ cells; re-deriving $q{=}5$, $n{=}20$
   from grid streams alone moved it from $z = -2.96$ to $z = -1.58$.
2. **Warm-up shared a stream counter with the timed repetitions**, and warm-up
   repeats on wall-clock, so which matrices a cell timed depended on machine
   speed and the recorded seed did not regenerate the recorded sample.

With disjoint streams and a deterministic timed start, independent
re-measurement puts $q{=}5$, $n{=}16$ at $0.20020$ ($z = +0.09$) where it had
read $0.19310$ ($z = -3.08$). **The apparent signal was measurement artefact and
sampling fluctuation, not structure.** Recorded rather than quietly dropped, per
`@/inv/falsification-preserved`: the earlier reading is what the study said, and
this is what independent re-measurement returned.

**What still holds.** These remain by-product samples with no pre-registered $N$
or stopping rule, so no inference rests on them either way, and the smallest
cells are tiny: $q{=}5$ at $n \ge 24$ carries a few hundred matrices, $q{=}3$ at
$n = 28$ a few thousand.
Four correctness anchors continue to pass and are the reason the table can be
read at all: the $q = 3$ arm agrees with [Scheinerman2024] at every $n$; the
kernels match an independent six-term permanent over all $q^9$ order-3 matrices,
with F_3 reproducing $z(3) = 8163$; sampler and kernel together recover the exact
order-3 zero fraction within $4\sigma$ over $4\times10^5$ draws; and no
deviation localises to a single backend.

**The design gain survives the retraction.** [HKS2026] Theorem 1.3 gives the
campaign a free, sharp, per-cell acceptance test — $\Pr \ge 1/q$ must hold at
every $n$ — that is strictly stronger than the order-3 anchors because it
applies at the sizes the campaign actually cares about, where enumeration says
nothing. §7.2 adopts it as a standing check.

## 5. Gap analysis (REQ-03)

Effort figures are **estimates**, expressed in ideal engineering days for one
agent working with review, and assume the harness in
`dev/research/permanent-sampling-feas/` is available to lift code from.

| ID | Gap | Status | Effort (est.) |
|---|---|---|---|
| G1 | Uniform $\mathbb{F}_q$ matrix sampler | Prototyped, needs productionising | 0.5 d |
| G2 | Streaming zero-fraction statistics with CIs | Prototyped, needs checkpointing | 1.0 d |
| G3 | Campaign runner | Missing; design decided below | 2.0 d |
| G4 | Versioned dataset format | Missing | 0.5 d |
| G5 | Permanental-rank predicate for the rectangular check | Missing; **no new kernel needed** | 1.0 d |
| G6 | `permanent_bipedal3` selects the slower path | Defect, confirmed by measurement | 0.5 d |
| G7 | No batch-parallel path for any field | Missing | 0.5 d |
| G8 | $q=7$ CPU ceiling at $n = 16$ | Structural limit | 4.0 d (optional) |

**G1 — uniform sampler.** The harness implements exact rejection sampling over
ChaCha20 (`sampler.rs`), with seeds derived from a 32-byte block encoding
`(root, q, n, stream)` so each cell and shard owns a disjoint, independently
addressable stream. Rejection is required because the sampler consumes bytes and
$256 \bmod 7 = 4$, a 2.8 % bias if ignored — three orders of magnitude larger
than the effect under study. Productionising means moving it behind a tested
API, with the entry-uniformity and stream-separation tests the prototype already
carries. *0.5 d.*

**G2 — streaming statistics.** The harness has Wilson score intervals
(`stats.rs`) and per-shard histogram accumulation. The campaign additionally
needs a checkpointable accumulator that survives restart without double-counting
a shard, and a Clopper–Pearson option for the small-count cells of the
rectangular validation, where the Wilson interval's normal approximation is
weakest. *1.0 d.*

**G3 — campaign runner. Decision: write a dedicated driver, do not adapt the
`gf2-sim` FEC campaign runner.** The FEC runner's campaign schema is
coding-domain — codes, modems, channels, SNR sweeps — while this campaign's axes
are $(q, n, \text{shard})$ and its per-point work is a permanent evaluation, not
a decode. Adapting it would mean either widening that schema to carry a second,
unrelated experiment type or forking it. `@/inv/convention-convergence` requires
a shared abstraction to have one form and forbids private parallel variants, but
it is aimed at duplicating a mechanism; the two campaigns share no axis and no
inner loop, so a separate driver is a distinct abstraction rather than a parallel
copy of an existing one. What the driver *should* reuse is the checkpoint and
CSV conventions, not the campaign type. *2.0 d.*

**G4 — dataset format.** A versioned layout with a manifest (root seed, git SHA,
toolchain, hardware, grid, per-cell $N$, schema version), per-shard records, a
pooled summary, and checksums. Because matrices are regenerable from
`(root, q, n, stream)`, shards store the $q$-bin histogram of permanent values
rather than the matrices, so storage is $O(\text{shards})$ and a whole cell costs
tens of kilobytes. *0.5 d.*

**G5 — rectangular validation.** Two successive framings of this gap were wrong,
and both corrections matter.

The initialized study called it "small — reuses G1/G2", which understated it. An
earlier revision of *this* document then called for a new $n \times k$ Ryser
kernel and estimated 3.0 d. **That framing was also wrong, for a mathematical
reason.** [GGK2025]'s rectangular event is not "the rectangular permanent
vanishes" — it is **permanental rank deficiency**. For an $n \times k$ matrix
$A$ with $k \le n$,

$$
\mathrm{per\text{-}rank}(A) < k
\iff
\text{every } k \times k \text{ row-submatrix of } A \text{ has zero permanent},
$$

so the predicate is a conjunction over the $\binom{n}{k}$ row subsets. A single
scalar "rectangular permanent" is not the quantity at all: it can vanish through
cancellation while some $k \times k$ submatrix has nonzero permanent, and it can
be nonzero while the rank condition is what one wants to test. Testing the event
therefore needs **no new kernel** — it enumerates row subsets and batches the
existing square $k \times k$ kernels, exiting early at the first nonzero
permanent (which is the overwhelmingly common case, so the expected work per
matrix is a small constant number of $k \times k$ permanents rather than
$\binom{n}{k}$ of them). *1.0 d, in the campaign driver.*

**The theorem's regime is not reachable by direct sampling, and the study says
so.** [GGK2025] Theorem 2.1 requires $k \le 0.1\sqrt{n}$, so even $k = 3$ needs
$n \ge 900$. The obstacle there is not Ryser cost — the submatrices are $3\times3$
— but the event probability itself: $\Pr \sim k/q^n \approx 3 \cdot 3^{-900}$,
which no Monte Carlo campaign can observe. Epic REQ-04 must therefore be read as
a **pipeline correctness check**, not a test of Theorem 2.1 in its proven range:

1. exact enumeration of the rank predicate on tiny $(n, k, q)$, and
   cross-implementation agreement against an independent brute-force
   permanental-rank routine;
2. estimation of $\Pr[\mathrm{per\text{-}rank} < k]$ at small $(n,k)$ where the
   event is observable — for $k=1$ the event is an all-zero column with
   probability exactly $q^{-n}$, giving $\approx 169$ events per $10^7$ samples
   at $q=3, n=10$; for $k=2$ the predicted $2 \cdot 3^{-n}$ is similarly
   observable — **explicitly noting that these $(n,k)$ lie outside the
   $k \le 0.1\sqrt{n}$ hypothesis**, so agreement supports the implementation
   and the $k/q^n$ heuristic, not the theorem;
3. in-regime estimation only as a possible rare-event follow-up (importance
   sampling or splitting), which is out of scope here.

**G6 — dispatcher selects the slower path.** `permanent_bipedal3` prefers the
AVX2 single-word kernel whenever AVX2 is detected, and the AVX2 single-word
kernel is slower than the scalar one: the SIMD path zero-pads a single
Bipedal3 word into a 4-element AVX2 lane, so three of four lanes carry no data.
This was already visible in `s3_cross_cpu-2026-05-12.csv` (scalar at 0.317–0.319
of the AVX2 time at $n = 16, 20, 24$) and is reconfirmed independently in §4.
The campaign must call `permanent_bipedal3_singleword` directly; the dispatcher
should also be fixed at its source, since every other caller of the public API is
silently paying the penalty. *0.5 d.*

**G7 — batch parallelism, and which parallelism to prefer.** No in-tree function
parallelises across matrices; `permanent_bipedal3_parallel` parallelises within
one matrix. The harness supplies batch parallelism from a `rayon` `par_iter` in
the driver, which needs no field-specific kernel and works for all three fields.

The measurements then contradict the obvious expectation that batching across
matrices is the better choice, and the campaign should follow the data rather
than the intuition. At $q = 3$ the in-tree **intra-matrix** path overtakes batch
rayon from $n = 20$ upward and leads it by roughly a factor of two at $n = 24$
and $n = 28$; the rates are in §4.4's table and §4.5's sustained table and are
not restated here. Batch rayon wins decisively only at small $n$, where each
matrix is short enough that per-matrix scheduling overhead dominates. The
plausible mechanism is working-set size: two dozen concurrent Gray walks each
carry their own column-sum state and column table, while one cooperatively
walked matrix shares them.

So the campaign driver needs **both**, selected per $(q, n)$ from measurement,
and F_5/F_7 have no intra-matrix path at all — which converts the "parallel
companions for F_5/F_7 remain a follow-up" note in `parallel.rs` from a
tidiness item into a throughput gap worth an estimate. *0.5 d for the driver-side
batch path; a further 1.5 d (est.) to port the intra-matrix Gray-code split to
F_5 and F_7, which is where the large-$n$ headroom for those fields is.*

**G8 — $q = 7$ above $n = 16$.** `Packed7` fits 16 lanes in a `u64`, so the CPU
kernel stops at $n = 16$ and the GPU is the only path beyond it. A multi-word
`Packed7` accumulator would restore a CPU fallback and remove the
single-backend dependency, but it is not required for the campaign to proceed.
*4.0 d, optional.*

## 6. Open questions

**Do the HIP kernels expose per-matrix results, so zero counting is exact?**
**Yes — resolved by code read and by measurement.**
`permanent_batch_bipedal{3,5,7}` return `Vec<Fp<q>>` of length $M$, one value per
input matrix in input order (`gpu.rs:264`, `:361`, `:467`). No aggregation
happens on the device. The equivalence run (§4.1) confirms the returned values
are identical to the CPU reference per matrix, not merely equal in aggregate.

**What is inside a GPU timing?** `permanent_gf3_batch_dispatch`
(`crates/gf2-kernels-hip/src/permanent/mod.rs:936`) performs two `hipMalloc`s,
one H2D copy, the kernel launch, `hipDeviceSynchronize`, one D2H copy, and frees
both buffers on drop — all per call, with no persistent device buffers and no
stream overlap. Host-side serialisation of the packed matrices into a row-major
byte buffer also happens per call, inside `gf2_algebra::gpu`. §4 reports the
consequences.

**Which parametric families for the $o(1)$ term are distinguishable?** Not
answerable by code read; it depends on the deviation magnitudes the campaign
measures. §7 states the analysis method and the honest limit: two families can be
told apart only while $|\delta(n)| = |p(n) - 1/q|$ exceeds a few standard errors
at several $n$, which the envelope in §4 bounds.

**Is a determinantal companion curve worth the marginal cost?** **Yes.**
Gaussian elimination is $O(n^3)$ against the permanent's $\Theta(n 2^n)$, so on
the same sampled matrices the determinant is free to within measurement noise at
every $n$ in the envelope. It also supplies a pipeline validation the permanent
cannot: $\Pr[\det = 0]$ has a known limit $1 - \prod_{i \ge 1}(1 - q^{-i})$, so a
measured determinant curve that misses its own limit falsifies the pipeline
before any permanent conclusion is drawn.

## 7. Recommendation (REQ-04)

**Verdict: GO**, with the scope set by §4.6's envelope and the first task
chosen by §4.7 rather than by the original plan.

The campaign is feasible. Under a 12 h budget per cell the measured composite
rates reach $n = 28$ for $q = 3$, $n = 24$ for $q = 5$, and $n = 20$ for $q = 7$
at a standard error of $10^{-3}$, and $n = 20 / 16 / 16$ at $10^{-4}$. No new
numeric kernel is required for the main curve. The infrastructure gaps are
small: G1-G4 and G6-G7 total about 5 engineering days by §5's estimates, and the
harness built for this study already implements the sampler, the statistics, and
the composite hot path.

Four findings shape what the campaign should be, and they are the substance of
this recommendation rather than caveats on it.

1. **Adopt [HKS2026] Theorem 1.3 as a standing acceptance test.** The theorem
   proves $\Pr[\mathrm{per} = 0] \ge 1/q$ for every $n$ at odd characteristic,
   so any cell whose interval lies strictly below $1/q$ indicts the pipeline
   rather than the theorem, and a below-$1/q$ result can never be reported as a
   finding. Run it on every cell for the campaign's life: it costs nothing to
   evaluate and is sharper than the order-3 enumeration anchors, which say
   nothing at the sizes the campaign cares about. §4.7 is the argument for it —
   an earlier revision of this study spent its top recommendation on chasing an
   apparent below-$1/q$ signal that turned out to be two measurement defects
   plus fluctuation, and this test would have classified it correctly on sight.
2. **The $q = 3$ arm is a reproduction-and-precision target, not new ground.**
   [Scheinerman2024] already publishes this curve to $n = 30$. This budget beats
   his precision at $n = 16$ and $n = 20$, matches at $n = 12$ and $n = 24$, and
   falls short at $n = 28$; extension beyond $n = 30$ is out of reach here by
   orders of magnitude. The $q = 3$ contribution is tighter intervals at two
   sizes, intervals where the source publishes none, an independently
   reproducible artifact trail, and a cross-implementation check of a published
   result — all worth doing, none of them "first numerics".
3. **The novelty is $q \in \{5, 7\}$**, for which no published numerics are
   known to us, and where the envelope reaches $n = 24$ and $n = 20$
   respectively at SE $= 10^{-3}$. Note what [HKS2026] leaves open and what it
   does not: $\Pr \ge 1/q$ and $\Pr \le 1/q + C/q^3$ are proved, so the campaign
   is not measuring whether the value sits near $1/q$ — that is settled — but
   *where in that $O(q^{-3})$ band it sits at finite $n$, and how it approaches
   the limit*. Framing any $q \in \{5,7\}$ result as "confirming $1/q$" would
   claim credit for a theorem.
4. **The scientific reach is bounded by resolution, not by $n$.**
   [Scheinerman2024] reports that his measured proportions stop being
   distinguishable from $1/3$ beyond $n = 13$, and this budget's per-cell sample
   sizes sit between 0.7x and 2.3x his across the measured $n$ — the same order,
   not a different regime. Expect to *characterise* the deviation
   only around $n \lesssim 14$-$16$ and to *bound* it above that. Spending the
   budget chasing $n = 28$ at $q = 3$ buys a wide interval around a value nobody
   can distinguish from $1/3$.

Recommended first breakdown: G1 and G2 (sampler and streaming statistics,
productionised from this harness, carrying the disjoint-stream and
deterministic-warm-up properties §4.7 shows are load-bearing), G4 (dataset
format), G6 (fix the F_3 dispatcher — a one-line selection bug costing every
caller ~3x), then G3 (campaign driver with the §7.2 acceptance test wired in),
then the $q \in \{5,7\}$ arms at the sizes §4.6 makes feasible, then $q = 3$ as
reproduction, then G5 and G7 as follow-ups.

### 7.1 What this campaign can and cannot establish

The conjecture's content is asymptotic: $o(1)$ is a statement about the limit
$n \to \infty$. **No finite grid of $n$ can test it**, and this study does not
claim otherwise. What the campaign produces is finite-$n$ evidence: a measured
$\Pr[\mathrm{per}(A) = 0]$ at each $n$ in the feasible range, with a stated
interval, together with an assessment of whether that sequence is consistent
with convergence to $1/q$ and inconsistent with convergence to some other
constant — which is exactly how the determinant, whose limit is a different
constant, distinguishes itself. Every conclusion drawn from this campaign must
be phrased at the measured sizes. A result that looks like convergence to $1/q$
over $n \le n_{\max}$ remains compatible with a limit elsewhere; a result that
clearly does *not* approach $1/q$ over that range is the stronger finding, since
it would contradict [GGK2025]'s conjecture in the regime actually measured.

### 7.2 Sampling plan

- **Pre-registered fixed $N$ per cell.** $N(q, n)$ is fixed from the envelope
  before any campaign data is drawn, and is not revised in response to the
  values observed. This replaces the withdrawn adaptive rule of §3; sequential
  stopping on the estimated proportion biases the estimate and breaks the
  interval's coverage.
- **Grid.** All $n$ from 4 up to the per-$q$ frontier, not only the sparse
  $\{12,16,20,24,28\}$ used for timing. Small $n$ cost almost nothing and the
  convergence-shape fit needs many points; the cost is dominated by the largest
  two or three $n$ regardless.
- **Exact anchors.** For the smallest sizes, $\Pr[\mathrm{per} = 0]$ is
  computable exactly by enumerating all $q^{n^2}$ matrices: $q=3$ up to $n=4$
  ($3^{16} \approx 4.3 \times 10^7$), $q=5$ and $q=7$ up to $n=3$
  ($5^9 \approx 2.0 \times 10^6$, $7^9 \approx 4.0 \times 10^7$). These give
  ground truth against which the sampler and the estimator are validated before
  any estimated cell is believed.
- **Standing acceptance test.** Every cell is checked against
  $\Pr[\mathrm{per} = 0] \ge 1/q$, proved for all $n$ at odd characteristic by
  [HKS2026] Theorem 1.3, and against the companion upper bound
  $\Pr \le 1/q + C/q^3$ for $n \ge 3$. A cell whose interval falls strictly
  below $1/q$ halts the campaign for pipeline investigation rather than being
  reported. This costs nothing to evaluate and applies at every $n$, unlike the
  order-3 enumeration anchors.
- **Determinant companion** on the same matrices, per §6.
- **Rectangular validation** (epic REQ-04) once G5 lands, framed as the
  three-part pipeline check described there rather than as a test of
  [GGK2025] Theorem 2.1 in its proven regime.
- **Comparison against [Scheinerman2024] Table 4 at $q = 3$** as a first-class
  output, per §7.6.

### 7.3 Seeding scheme

ChaCha20 (`rand_chacha` 0.9), seeded from a 32-byte block of four little-endian
`u64` words: campaign root, $q$, $n$, and stream index. Shard $s$ of cell
$(q, n)$ uses stream $\mathrm{base}(q,n) + s$; stream 0 is reserved for
validation.

**Stream allocation must be structurally disjoint, not coincidentally so.**
This study's own history makes the point: the harness first based its sustained
runs at $10^6 + j \times 10^5$ against a grid reserving $1 + i \times 10^5$ per
cell, which are commensurate — the ranges collided in index space, and the
pooled counts of the day were valid only because no colliding pair happened to
share a $(q, n)$. That was verified, not designed. Raising the sustained base to
$10^9$ (§4.7) fixed it for these receipts by putting the two allocations two
orders of magnitude apart, which is the minimum a campaign should inherit rather
than the whole of it. A campaign running for twelve hours per cell cannot audit
its way out of this: allocate each $(q, n, \text{purpose})$ a range from a
partition that cannot overlap by construction — distinct high-order bits, or a
per-purpose salt folded into `derive_seed` alongside the stream index — so that
reusing a matrix is impossible rather than merely unobserved. Three properties
follow, and all three matter operationally: any shard is reproducible from its
tuple alone, a lost or corrupted shard is redrawn without touching its
neighbours, and shards can be produced in any order or concurrently without
coordinating a shared generator state. The matrix layout
mapping is recorded with the scheme: draw $k$ becomes $A[k / n][k \bmod n]$,
row-major, before the packed constructor reorders it into the kernels' storage.

### 7.4 Storage layout

```text
dev/campaigns/permanent-zero-fraction/<campaign-id>/
  manifest.toml          root seed, harness source SHA, rustc/ROCm, hardware,
                         grid, per-cell N, shard size, schema version
  shards/q<q>/n<nn>/shard-<index>.csv
                         stream index, matrices, q-bin histogram of per(A)
  summary.csv            per (q, n): pooled counts, p_hat, Wilson interval
  checksums.sha256
```

`dev/campaigns` is one of the permanent documentation areas declared in
`.jit/config.toml` (`[documentation] permanent_paths`), which is why the
dataset lives there rather than under a newly invented directory: a published
dataset outlives the issue that produced it, so it does not belong in an
issue-scoped area like `dev/studies`, and inventing a path outside the
configured set would put it outside the repository's documentation contract.

Shards store the histogram of permanent values over the $q$ residue classes
rather than the matrices: matrices are regenerable from their stream tuple, so
storage is $O(\text{shards})$. The histogram also preserves the full value
distribution rather than only the zero indicator, which costs nothing and keeps
the door open to distributional questions — though note that the epic's REQ-05
asks specifically for a **permanental-versus-determinantal zero-fraction
comparison figure** and a parametric fit of the $o(1)$ term, not a
whole-distribution uniformity comparison; that latter framing belongs to
[Scheinerman2024]'s Conjecture 4.1 and to the separate `perm_uniformity` work,
not to REQ-05. Datasets are versioned by campaign id and never overwritten in
place.

### 7.5 Analysis method

Per cell, $\hat p = Z/N$ with a 95 % Wilson score interval. For the convergence
shape, fit $\delta(n) = \hat p(n) - 1/q$ against the two candidate families
named in the initialized study — geometric $\delta \sim c\,q^{-\alpha n}$ and
polynomial $\delta \sim c\,n^{-\beta}$ — by weighted least squares with weights
$1/\mathrm{se}(n)^2$, comparing them by AIC and by whether each fit's parameter
interval excludes the other family's behaviour. Report the range of $n$ over
which $|\delta(n)|$ exceeds a few standard errors, since outside it the two
families are not distinguishable and no model preference should be claimed.
Ratios of rates are never formed by averaging per-repetition reciprocals.

### 7.6 Delta against the prior art at $q = 3$

[Scheinerman2024] is the baseline: exact enumeration for $n \le 5$, Monte Carlo
for $6 \le n \le 30$, at $\approx 3.1 \times 10^3$ processor-hours. The right
comparison is **achieved precision**, not trial count — matching his $N$ is not
a goal, resolving $\delta(n) = p(n) - 1/q$ is — so the envelope CSV carries his
per-$n$ standard error and 95 % Wilson half-width beside this campaign's, with
their ratio and a `precision_comparison` classification. Where this campaign
adds, and where it does not:

1. **Greater precision at specific $n$.** Real but uneven, and the envelope
   table in §4 names the cells rather than the prose asserting a blanket claim.
   A cell classified `exceeds_prior_precision` narrows the published interval;
   one classified `below_prior_precision` does not, and is reported as a
   reproduction.
2. **Extension beyond $n = 30$: not available here.** His table already reaches
   $n = 30$; §4's measured rates put $n = 30$ far outside a 12 h budget on this
   host for any backend. This campaign does **not** extend the $q = 3$ curve in
   $n$, and no wording in the final report should imply otherwise.
3. **Intervals where the source publishes none.** Table 4 gives point estimates
   without uncertainty. Supplying a Wilson interval for every cell — including,
   by reanalysis, for his own published counts — is a contribution independent
   of how many samples this campaign draws.
4. **Independently reproducible artifacts and protocol.** A committed dataset
   with root seed, per-shard stream indices, git SHA, toolchain, and hardware
   makes every cell regenerable. The prior work reports counts, not a
   reproduction path.
5. **Independent reproduction has value on its own.** Agreement from a different
   implementation (packed bipedal kernels versus the paper's Julia), a different
   RNG (ChaCha20 versus whatever the paper used), and a different backend
   (AVX2/rayon/HIP) is meaningful evidence for both pipelines. Cells that merely
   reproduce are labelled as reproductions and reported as such.
6. **A preserved discrepancy would be the most valuable outcome.** Any $n$ where
   the campaign's interval excludes his point estimate is recorded together with
   the contradiction and investigated rather than reconciled away, per
   `@/inv/falsification-preserved`.

The $q \in \{5, 7\}$ arms have no published baseline known to us, so every
measured cell there is new; that is a statement about the literature as we found
it, not a claim of priority.

## References

- [GGK2025] Ghasemi, Gross, Kopparty — Permanental Rank versus Determinantal Rank
  of Random Matrices over Finite Fields. APPROX/RANDOM 2025. arXiv:2512.03221;
  ECCC TR25-206.
- [HKS2026] Hunter, Kwan, Sauermann — Permanents of Random Matrices over Finite
  Fields. arXiv:2603.15856.
- [Scheinerman2024] Scheinerman — Fast Computation of Permanents over F_3 via F_2
  Arithmetic. arXiv:2407.20205.
