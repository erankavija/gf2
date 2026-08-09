# Feasibility study: permanent-zero-fraction sampling campaign

> **Status:** complete. Every number in §4 traces to a committed CSV in this
> directory. One artifact is not evidence for anything and is committed as
> preserved history: `gpu-hang-2026-08-07.log`, whose single unreproduced device
> fault supports no claim here (§4.5). The receipt files keep the `2026-08-07`
> names they were given when
> the set was first produced; the measurements committed here were re-taken on
> **2026-08-08 UTC**, and each receipt's `timestamp_utc` header gives its own
> time. Where the two disagree, the header is authoritative — the filename is a
> stable identifier, not a date of measurement.

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
against it cell by cell, on achieved precision rather than raw trial count. For
$q \in \{5, 7\}$ a documented search found no published numerics; the search
and its limits are recorded in `literature-search-2026-08-08.md` and the claim
is stated at that strength in §7.6.

## 2. Capability inventory (verified in-tree)

Every claim in this section was checked against the source, and each citation
re-verified against it on 2026-08-08. Line
references are to the state of `crates/` recorded as `deps_source_sha` in every
receipt (`195f8254`), alongside the harness commit `0e0b0aec` that produced the
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
- **Generic**: `permanent_ryser<F: FiniteField>` (`permanent/ryser.rs:89`) is a
  complete Ryser evaluation over unpacked field elements, asserting only
  $n \le 63$. It is field-agnostic, so it applies to $q \in \{3,5,7\}$ at every
  $n$ the study measures — including the sizes where the packed $\mathbb{F}_7$
  kernel does not exist. Its rustdoc describes it as a driver for cross-checks;
  that is a statement of intent, and §4.1 confirms it returns the same value as
  every packed kernel at every comparable cell.
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
3. `permanent_bipedal7`'s $n \le 16$ bound means **$q = 7$ has no *packed* CPU
   kernel for $n > 16$**. An earlier revision of this study concluded from that
   that $q = 7$ has no CPU path at all above $n = 16$, and that the $n \in
   \{24, 28\}$ cells therefore carry no rate from any backend. **Both were
   wrong**, and the error was in this document rather than in the code: the
   generic `permanent_ryser` has always applied there, and §4.4 now measures it
   at 0.798 matrices/s at $n = 24$ and 0.0426 at $n = 28$. The rates are far too
   slow for the campaign, so no frontier moves, but "no path exists" and "too
   slow to use" are different findings and only the second is true.

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

All measurements were taken on the project benchmark host, which was otherwise
idle, and the committed set was produced on 2026-08-08 UTC between 05:45 and
07:24 — equivalence at 05:45, the grid at 05:47, the sustained runs at 06:48,
and the two derived stages at 07:23, each stamped in its own `timestamp_utc`.
The filenames retain the `2026-08-07` stem of the original set and are not
evidence of when a
measurement ran; §4's provenance list records why the set was regenerated. The
one artifact whose date is its content is `gpu-hang-2026-08-07.log`, which
records a fault that genuinely happened on 2026-08-07. One known limitation of
the CSVs: preamble note text is emitted unquoted, so a note containing a comma
splits across fields for a naive reader — the `#` prefix marks those lines as
preamble, and fixing the emitter would mean regenerating the set, which DEC-01
forecloses. Artifacts in this
directory, each carrying a preamble with
git SHA, rustc and ROCm versions, CPU and GPU model, governor, thread count, and
the exact invocation:

| Artifact | Contents |
|---|---|
| `equivalence-2026-08-07.csv` | cross-backend per-matrix agreement (newer pin; §4) |
| `throughput-2026-08-07.csv` | the `(q, n, backend)` grid, 120 cells |
| `sustained-2026-08-07.csv` | minutes-scale streaming runs per backend |
| `envelope-2026-08-07.csv` | REQ-02 envelope, incl. the prior-art comparison |
| `zero-fraction-2026-08-07.csv` | pooled zero fractions with Wilson intervals |
| `cargo-tree-2026-08-07.txt` | resolved dependency graph (the harness gitignores `Cargo.lock`) |
| `gpu-hang-2026-08-07.log` | preserved history of one device fault; supports no claim (§4.5) |
| `literature-search-2026-08-08.md` | recorded search behind §7.6's novelty claim |
| `determinant-anchor-check.py` | executable check of §6's exact singular probability |
| `determinant-anchor-2026-08-08.txt` | that check's committed output |
| `order3-anchor-check.py` | executable receipt for §4.7's order-3 sampling anchor |
| `anchor-report/` | standalone crate printing that anchor's observed draw |
| `order3-anchor-2026-08-08.txt` | that check's committed output |

Every CSV preamble records `harness_source_sha` and `harness_source_dirty`
alongside the repository SHA. The harness SHA is the one that matters: the
repository carries `.jit/` workflow state that other agents commit
independently, so a whole-repo dirty flag says nothing about whether the
measured code was committed. **The four measurement receipts were produced by
one executable, `binary_sha256`
`b1fe566fe1d133f2cbd298d35ca5290707830b71e3670f891b9e86aa8978698c`, built at
harness source commit `0e0b0aec` with
`harness_source_dirty: false`, `deps_source_sha: 195f8254` and
`deps_source_dirty: false`.** Two harness commits post-date them, and the next
paragraph bounds what they change; the first, `b96f9550`, rewords a rustdoc
paragraph and nothing else, and a forced recompile at it produces the
byte-identical executable, checked with `cmp` against the binary that wrote
these receipts.

**The equivalence receipt carries a newer pin than the other four, and the
difference is bounded.** It was regenerated at `2bea03a4`, binary
`77b52ddb43e59c722adf2c311b79336e78ce7407a3f261fcfa79cf87350dad02`, to add the
$q = 7$, $n = 20$ cell described in §4.1; the four measurement receipts stay at
`0e0b0aec` and `b1fe566f` under **DEC-01**, which fixes that set. Two commits
separate the two binaries. `b96f9550` changes only comments — its `protocol.rs`
diff contains no non-comment line — and `2bea03a4` touches `equivalence.rs` and
the two preamble strings and removed size skip inside `cmd_equivalence`.
`backend.rs`, `sampler.rs`, `protocol.rs`'s code, `stats.rs`, `prior.rs` and
`env.rs` are untouched across both, so every path the grid, sustained, envelope
and zero-fraction stages execute is identical in the two builds. The kernels,
the sampler and the timing protocol that produced the measurements are the same
code either binary would run.

`binary_sha256` is what makes the reproduction claim checkable, and it is
load-bearing rather than decorative: a source SHA describes the checkout, while
the hash is taken over the executable that actually ran. The two can disagree —
a checkout advances the moment a commit lands, an executable only when it is
rebuilt — and in this study's own history they did.

Four earlier receipt sets were discarded rather than reinterpreted. The third
is the reason `binary_sha256` is quoted here, and the fourth is the reason the
preamble figures are now formatted from the constants they describe:

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
4. One announced `streams from 1_000_001` in the sustained preamble while every
   row it described began at `1_000_000_001`. The note predated the rise of
   `SUSTAINED_STREAM_BASE` to $10^9$ and was never updated, so the receipt
   contradicted itself on the disjointness that makes its samples poolable
   (§4.7). The emitted note is now formatted from the constants themselves
   (`12f6e81a`), and the receipts below it were regenerated.
5. One asserted in its own preamble that the GPU batch ceiling "is a watchdog
   limit rather than a memory or occupancy one" — a cause this study had already
   retracted, since the fault occurred once and nothing captured identifies its
   mechanism (§4.5). The emitted note now records the observation and names the
   log as its only receipt (`7189d66f`). That sweep also restated four other
   emitted claims that outran their evidence: drift is measured rather than
   attributed to boost decay, the projection's low bias is marked as an
   extrapolation from the $q = 3$ GPU chain to fields it was never checked on,
   the censoring contract no longer promises numbers a current grid does not
   carry, and `no_prior` is described as this harness's own missing baseline
   rather than as a statement about the literature.
6. One measured six backends where seven applied. The grid enumerated its CPU
   paths from a hand-written list beside `Backend::ALL`, so the generic
   `permanent_ryser` — applicable to every field at every $n \le 63$ — was never
   scheduled, and REQ-01's "every applicable in-tree path" went unmet with no
   receipt showing a gap. `0e0b0aec` adds the backend and drives the grid from
   `Backend::ALL`, so a path added to the enum cannot miss the schedule again.
   It also fixes the sustained quarter statistic, which split by shard count
   while the receipt called it a split by elapsed time.

That set is superseded by the present one, whose four measurement receipts were
regenerated end to end — grid, sustained, envelope, zero fractions — from the
single binary named above, with each derived stage reading the throughput and
sustained files committed here; equivalence was regenerated separately and later
at the pin the next paragraph gives. The harness stamps provenance faithfully;
nothing in it checks that the executable it is running was built from the source
SHA it reports, so
the guard against that failure is regenerating a receipt set from one build,
which is what the four measurement receipts are.

Host: AMD Ryzen 9 5900X (12 cores / 24 threads, `powersave` governor), AMD
Radeon RX 6950 XT (gfx1030, 80 compute units), rustc 1.97.0, ROCm/HIP 7.2.

### 4.1 Backend equivalence (precondition)

Equivalence ran twice, and the order matters. The original schedule ran **before
any timing counted as evidence**, across all seven backends, and the timing
measurements were taken only after it passed. The schedule-extended re-run
**postdates those measurements**: it re-verified the whole suite and added the
$q = 7$, $n = 20$ cell, under the user-approved extension recorded as
**DEC-02**, which also fixes the resulting two-binary provenance as final. So
the precondition was met in the original order, and the extra cell is a later
addition to the same check rather than something the timing rested on. Both runs
compared backends per matrix on shared inputs drawn from a reserved seed
stream. **Every backend
returned byte-identical permanents for every matrix** — 46 comparisons agreeing
and 26 recorded unsupported with a reason — for $q \in \{3,5,7\}$ at
$n \in \{8, 12, 16, 20\}$, 512 matrices per cell.

**The $q = 7$, $n = 20$ cell is checked, and that matters more than the rest of
the table.** An earlier schedule skipped $q = 7$ above $n = 16$ because the
reference kernel stops there, which left the one $(q, n)$ carrying this study's
most extreme zero-fraction reading (§4.7, $z = -2.30$) with no cross-backend
check at all — the samples came from the GPU, and nothing independent had ever
reproduced them. The check now falls back to the generic `permanent_ryser` as
its reference wherever the packed kernel is absent, and the CSV's `reference`
column records which kernel each row was compared against (66 rows against the
scalar kernel, 6 against the generic one). At $q = 7$, $n = 20$ the GPU returns
**byte-identical permanents to the generic path over all 512 matrices**, both
counting 69 zeros. The backend that produced the anomalous cell agrees with an
independent implementation, so §4.7's reading is a statement about the sample
rather than about the kernel. Backends without a kernel for a given field are
recorded with the reason rather than dropped. This satisfies
`@/inv/backend-behavioral-equivalence` for the paths the campaign would use.

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
  mitigated rather than eliminated, and the residue is a stated limitation of
  this protocol: the machine is warmed to steady state before cell 0 and
  within-stratum order is randomised, but **$n$ remains confounded with elapsed
  time across the grid**. §4.5's sustained runs bound drift *within* a 180 s
  window below 1.2 % on every path; they say nothing about drift across the
  multi-hour span of the whole grid, so a slow monotone trend over that span
  would be absorbed into the $n$ trend rather than detected. Settling it needs a
  reference cell re-measured at intervals through the run, which this study did
  not do. The conclusions drawn here are robust to it only because the rates
  span six orders of magnitude across the grid while any plausible thermal or
  clock drift is a few per cent.
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

Of 120 cells: **76 measured, 36 unsupported, 8 censored.** No cell was silently
skipped. The grid gained 15 cells this round, one per $(q, n)$, when the generic
`permanent_ryser` path was added as a measured backend (§2); every one is
measured, including the two at $q = 7$, $n \in \{24, 28\}$ that earlier
revisions recorded as having no path at all.

All eight censored cells are GPU cells at $(q, n) \in \{5, 7\} \times \{24, 28\}$,
each projected from its own measured rate at $n = 20$:

| cell | projected rate | implied repetition | verdict |
|---|---|---|---|
| $q{=}5$, $n{=}24$, $M{=}256$ | 1.687 | 152 s | over the 120 s cap |
| $q{=}5$, $n{=}24$, $M{=}1024$ | 3.330 | 308 s | over |
| $q{=}5$, $n{=}28$, $M{=}256$ | 0.0904 | 2833 s | far over |
| $q{=}5$, $n{=}28$, $M{=}1024$ | 0.1784 | 5741 s | far over |
| $q{=}7$, $n{=}24$, $M{=}256$ | 1.457 | 176 s | over |
| $q{=}7$, $n{=}24$, $M{=}1024$ | 3.017 | 339 s | over |
| $q{=}7$, $n{=}28$, $M{=}256$ | 0.0781 | 3279 s | far over |
| $q{=}7$, $n{=}28$, $M{=}1024$ | 0.1616 | 6335 s | far over |

Rates are matrices/second and are **estimates** whose bias magnitude is
unvalidated at these field orders (below). None is rescued by any plausible
correction: the closest, $q{=}5$ at $n{=}24$ and $M{=}256$, would need its true
rate to beat the projection by 27 % to fit the 120 s cap. The $n = 24$ cells are
marginal and the $n = 28$ cells miss by one to two orders of magnitude.

**What censoring costs differs sharply between the two fields.** At $q = 5$ it
costs the envelope nothing: CPU batch rayon is measured at every one of those
sizes and is the faster path anyway, so the GPU cell was never going to be
selected. At $q = 7$ it costs everything. `permanent_bipedal7` stops at
$n \le 16$, so $q{=}7$ at $n \in \{24, 28\}$ has **no supported packed CPU
kernel**, and both GPU batch sizes are censored there. What remains is the
generic `permanent_ryser`, which measures those two cells at 0.798 and 0.0426
matrices/s. The $n = 28$ cell is among the slowest in the whole grid — the
generic path at $n = 28$ measures 0.0417 to 0.0426 matrices/s across all three
fields, and nothing else runs slower — and it is why the $q = 7$ arm's frontier
still stops at $n = 20$. Earlier
revisions of this study called them unmeasurable; they are merely unaffordable,
and §4.6 now carries their rates instead of a dash.

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
$(q, \text{backend}, M)$ chain, 60 of the 120 cells — and the large-$n$ GPU
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
| 256 | $n = 20 \rightarrow 24$ | 111.26 | 136.41 | $-18.4\%$ |
| 256 | $n = 24 \rightarrow 28$ | 7.308 | 8.533 | $-14.4\%$ |
| 1024 | $n = 20 \rightarrow 24$ | 253.26 | 310.38 | $-18.4\%$ |
| 1024 | $n = 24 \rightarrow 28$ | 16.627 | 19.268 | $-13.7\%$ |

The projection is consistently **low**, by 14–18 % across both batch sizes and
both steps, because longer kernels amortise per-launch overhead better than the
reference does. This is the figure the CSV preamble defers to rather than
quoting, so a re-measurement cannot leave a stale percentage behind.

**A $q = 3$ chain cannot validate a $q \in \{5,7\}$ projection, so each field
validates its own.** The kernels do not share a cost structure:
`permanent_bipedal3.hip` keeps its column sums bit-sliced in a single
`(sum_m, sum_s)` word pair and updates them in **O(1)** work per Gray step,
while `permanent_bipedal5.hip` and `permanent_bipedal7.hip` run an explicit
`for (i = 0; i < n; i++)` over the column-sum array at every step, costing
**O(n)**. Ryser's $n \cdot 2^n$ model therefore *overstates* how the $q = 3$
kernel grows — one reason its projections read low — and *matches* the
$q \in \{5,7\}$ kernels far better, so a 14–18 % correction carried across
would be a number from the field where the model fits worst applied where it
fits best.

The grid supports the check per field without any extra measurement: within each
$(q, \text{backend})$ chain, project every measured $n$ from the preceding
measured $n$ and compare against what was measured. The censored cells are all
GPU cells at $q \in \{5,7\}$, so that is the chain that governs them:

| chain | step | projection error |
|---|---|---|
| $q{=}5$ GPU $M{=}256$ | $12 \rightarrow 16$ | $-13.2\%$ |
| $q{=}5$ GPU $M{=}256$ | $16 \rightarrow 20$ | $-4.7\%$ |
| $q{=}5$ GPU $M{=}1024$ | $12 \rightarrow 16$ | $-11.1\%$ |
| $q{=}5$ GPU $M{=}1024$ | $16 \rightarrow 20$ | $-4.3\%$ |
| $q{=}7$ GPU $M{=}256$ | $12 \rightarrow 16$ | $-12.4\%$ |
| $q{=}7$ GPU $M{=}256$ | $16 \rightarrow 20$ | $-4.2\%$ |
| $q{=}7$ GPU $M{=}1024$ | $12 \rightarrow 16$ | $-8.7\%$ |
| $q{=}7$ GPU $M{=}1024$ | $16 \rightarrow 20$ | $-4.3\%$ |

**All eight steps run low, in the fields the censored cells belong to.** The
bias direction is therefore empirically validated where it is applied, not
merely argued: a projection understates the rate, so a censored cell's true rate
is at least as high as its projection. The magnitude is 4–13 % and **shrinks as
$n$ grows** — every $16 \rightarrow 20$ step is near $-4.3\%$ against $-9$ to
$-13\%$ one step earlier — which is what the per-launch-amortisation mechanism
predicts, since a longer kernel has proportionally less launch overhead left to
absorb. The censored cells project from $n = 20$, one step beyond the last
validated step, where the trend points to a bias smaller still.

Two limits on that validation are worth stating. It is bounded by the largest
$n$ where both ends are measured, so nothing here validates the *size* of the
correction at $n \ge 24$ on these kernels, only its direction and its downward
trend. And the direction is not universal across backends: the generic Ryser
path, whose per-step cost genuinely is $O(n)$ and so matches the model, projects
**high on 10 of its 12 steps**, from $+8.6\%$ at $12 \rightarrow 16$ down to
$+0.4\%$ at $24 \rightarrow 28$. The two exceptions are the $24 \rightarrow 28$
steps at $q = 3$ and $q = 5$, at $-0.68\%$ and $-0.21\%$ — both within a percent
of zero, which is where the whole chain is heading: the bias is largest at small
$n$ and decays toward zero as $n$ grows, because the model's $n \cdot 2^n$ is
this kernel's true cost and only the small-$n$ overheads it omits keep the two
apart. A projection's bias follows the kernel's cost structure, which is exactly
why it is validated per chain rather than once.

**The throughput receipt's own prose is more conservative than this, and that is
recorded rather than reconciled.** Its `projection accuracy` preamble and its
censored-row notes say the check is possible on the $q = 3$ GPU chain and call
the step to $q \in \{5,7\}$ an extrapolation. That wording was written before
the same-field validation above, which is derived from rows in that very file
and needed no new measurement. Per **DEC-01** the derivation is authoritative
where a receipt's rows support a stronger result than its emitted text, and the
emitted text stands as a conservative point-in-time statement: a supersession on
the record, not a contradiction between two claims. The receipt set is final at
`243886e2`; nothing above re-measures anything.

None of that changes a verdict, because the censoring margins are not close. To
rescue the nearest censored cell — $q{=}5$, $n{=}24$ at $M{=}256$ — the true
rate would have to exceed its projection by **26 %**, twice the largest bias
measured on its own field's chain and six times the bias at the step nearest it;
the other $n = 24$ cells need 46 % to 183 %, and the $n = 28$ cells need factors
of 24 to 53. The conclusions rest on those margins
rather than on a transferred correction. The projection is used the same way
throughout: a cell whose projected rate misses the budget by an order of
magnitude is confidently infeasible, while one that misses by 20 % is not, and
is attempted rather than censored.

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

| $q$ | $n$ | scalar | AVX2 | rayon batch | rayon intra | Ryser generic | GPU $M{=}256$ | GPU $M{=}1024$ |
|---|---|---|---|---|---|---|---|---|
| 3 | 12 | 50 846 | 18 182 | **280 056** | 38 462 | 6 902 | 218 275 | 247 646 |
| 3 | 16 | 3 638 | 1 196 | 36 311 | 4 439 | 307.8 | 30 210 | **61 306** |
| 3 | 20 | 229.9 | 74.82 | 2 500 | 2 982 | 15.18 | 2 136 | **4 863** |
| 3 | 24 | 14.35 | 4.650 | 155.4 | 296.6 | 0.777 | 136.4 | **310.4** |
| 3 | 28 | 0.903 | 0.289 | 9.986 | **19.58** | 0.0419 | 8.532 | 19.27 |
| 5 | 12 | 6 787 | — | **72 323** | — | 6 906 | 11 440 | 23 210 |
| 5 | 16 | 327.0 | — | **4 164** | — | 306.9 | 617.5 | 1 224 |
| 5 | 20 | 16.42 | — | **213.7** | — | 15.08 | 32.39 | 63.92 |
| 5 | 24 | 0.855 | — | **11.83** | — | 0.777 | censored | censored |
| 5 | 28 | 0.0461 | — | **0.648** | — | 0.0417 | censored | censored |
| 7 | 12 | 6 496 | — | **72 556** | — | 7 285 | 10 016 | 21 596 |
| 7 | 16 | 313.9 | — | **3 741** | — | 314.6 | 536.2 | 1 108 |
| 7 | 20 | unsupported | — | unsupported | — | 15.42 | 27.98 | **57.93** |
| 7 | 24 | unsupported | — | unsupported | — | **0.798** | censored | censored |
| 7 | 28 | unsupported | — | unsupported | — | **0.0426** | censored | censored |

Four results carry consequences beyond the envelope.

**The public F_3 dispatcher selects the slower path (gap G6).** The scalar
single-word kernel beats the AVX2 single-word kernel by **2.80x-3.12x** at every
$n$ measured — the ratio is smallest at $n=12$ and rises monotonically with $n$,
and the two rates are in the table above — yet `permanent_bipedal3` prefers AVX2
whenever the CPU supports it. The cause is documented in the kernel itself: the
SIMD path zero-pads a single Bipedal3 word into a 4-element AVX2 lane, so three
of four lanes carry no data. This reproduces the ratio already visible in
`dev/benchmarks/gf2_algebra_permanent/s3_cross_cpu-2026-05-12.csv` (scalar at
0.317-0.319 of the AVX2 time), now on an independent harness and RNG.

**At $q = 3$ the GPU leads only in the middle of the range, and loses at
$q \in \{5,7\}$ throughout.** The $q = 3$ crossover has a shape rather than a
threshold: batch rayon takes $n = 12$, the GPU at $M = 1024$ takes $n = 16$
through $n = 24$, and intra-matrix rayon takes $n = 28$. At the bottom the
composite is dominated by generating and packing a 65 536-matrix batch rather
than by the kernel, which is what costs the GPU that cell; at the top its margin
over intra-matrix rayon decays — 1.63x at $n = 20$, 1.05x at $n = 24$, and
**0.984x at $n = 28$**, where intra-matrix rayon is ahead. For
$q = 5$ and $q = 7$ the CPU batch-rayon path wins wherever both are supported,
by roughly a factor of three at every shared $n$ in both fields. The F_7 GPU
kernel is the weakest of the three: its LUT-based arithmetic makes it the
slowest GPU path at every shared $n$, and like the $\mathbb{F}_5$ kernel it is
censored above $n = 20$.

**Whether the GPU or intra-matrix rayon leads at $n = 28$ is not resolved by
these measurements, and the receipts bound that quantitatively.** Each rate is
one cell execution, so the two are point comparisons rather than replicated
ones, and the gap between them is **1.6 %**, intra-matrix rayon ahead. Two
things in the receipts bound what a gap that size means:

- *Within* a single cell execution the repetitions are very stable: `rep_sd_s`
  is 0.12 % of a repetition for intra-matrix rayon and 0.16 % for the GPU, an
  order of magnitude under the gap.
- *Across* independent executions of the same configuration the spread is
  wider. The sustained runs re-measure nine grid configurations end to end; at
  identical batch size the three GPU pairs disagree with their grid cells by
  0.57 %, 0.02 % and 0.02 %, while over all nine pairs the disagreement reaches
  1.80 % with a median of 0.84 %.

So the gap is larger than anything a re-measured GPU configuration moved, and
comparable to the widest disagreement across all nine pairs. The evidence leans
toward intra-matrix rayon at $n = 28$, and **no ordering is asserted**: one
execution of each cannot settle 1.6 % when independent re-measurement of a
single configuration has moved 1.8 %, and those nine pairs are a loose bound
rather than a variance estimate, four of the nine differing in batch size as
well as in run. The table reports both rates; a campaign choosing that cell's
backend should re-measure.

An earlier revision of this study reported the opposite at $n = 28$ — that
intra-matrix rayon beat the GPU by 2.3x. **That was an artifact of the
superseded censoring rule**, which declined the $q{=}3$, $n{=}28$, $M{=}1024$
cell and left only $M{=}256$ to compare against. With the corrected rule that
cell is measured, and the two rates come within 1.6 % of each other — a gap the
preceding paragraph shows one execution of each cannot settle, rather than a
2.3x lead for either. Recorded per
`@/inv/falsification-preserved`; it is the clearest illustration of why a
censoring rule that hides affordable cells is a correctness problem and not a
scheduling convenience.

**This qualifies the 2026-05-15 GPU crossover receipt.** That receipt reports
the GPU beating "CPU SIMD" by 28.65x at $n=24$ and 30.32x at $n=28$ for $q=3$,
both at $M=256$. Those ratios reproduce here — GPU $M{=}256$ over `cpu_avx2`
gives 29.3x at $n=24$ and 29.5x at $n=28$ — but the baseline is the AVX2
single-thread path, which §4.4 has just shown to be the *slower* of the two
single-thread CPU paths, and unparallelised besides. Restated against the best
CPU path measured here, the same $M{=}256$ configuration is **0.46x at $n=24$**
and **0.44x at $n=28$**, and the honest headline is the $M{=}1024$ comparison
above: a 1.05x edge at $n=24$ and an unresolved ordering at $n=28$, not 30x.
This study's
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
| 3 | 24 | scalar | 8 | 14.508 | 14.346 | 1.011 | 14.500 → 14.511 |
| 3 | 24 | AVX2 | 4 | 4.595 | 4.650 | 0.988 | 4.603 → 4.589 |
| 3 | 24 | rayon batch | 96 | 157.837 | 155.359 | 1.016 | 158.233 → 157.607 |
| 3 | 24 | rayon intra | 24 | 299.146 | 296.643 | 1.008 | 299.207 → 299.118 |
| 3 | 24 | GPU | 1024 | 312.135 | 310.378 | 1.006 | 312.440 → 311.602 |
| 3 | 24 | GPU | 2048 | 209.756 | — | — | 209.853 → 209.718 |
| 5 | 20 | rayon batch | 96 | 214.603 | 213.712 | 1.004 | 213.136 → 215.656 |
| 5 | 20 | GPU | 1024 | 63.935 | 63.919 | 1.000 | 63.949 → 63.924 |
| 7 | 16 | rayon batch | 512 | 3673.828 | 3741.104 | 0.982 | 3674.235 → 3676.730 |
| 7 | 20 | GPU | 1024 | 57.944 | 57.934 | 1.000 | 57.954 → 57.938 |

Rates are matrices/second; "grid cell" is the same $(q, n, \text{backend})$ cell
from §4.4. The batch sizes coincide only on the GPU rows, where $M$ is fixed by
the schedule; the CPU rows let the grid calibrate its own $M$ against a 2 s
repetition, so the comparison is between a short cell and a long one at the same
backend rather than at an identical batch size. Each run draws from its own
reserved stream range, recorded in the CSV's `stream_first` column, so no two
runs share matrices. Two conclusions.

**The short-cell protocol holds.** Every run lands within 2 % of its grid cell,
and boost decay across a 180 s window stays under 1.2 % (largest
first-to-last-quarter drift: 1.18 % on batch rayon at $q{=}5$). Those quarters
are quarters of *elapsed time*, not of the shard count: the harness split them
by count until `0e0b0aec`, which is a different statistic whenever shard
durations vary. The five-second cells are
not riding a boost window, so the envelope built on them is sound.

**$M = 1024$ is the best batch size tested, and the question is open above
2048.** At $q{=}3$, $n{=}24$ doubling the batch to $M = 2048$ costs a **33 %
loss** in sustained rate, so within the tested set $\{256, 1024, 2048\}$ the
optimum is interior rather than at the top. Only four batch sizes were ever
tried, one of them once and fatally, so "optimum" here means best-of-tested at
one $(q, n)$ on one device — not a characterised curve, and not a claim about
sizes between or beyond those tried.

**Sizes above 2048 were not scheduled, and that is a scheduling decision rather
than a measured ceiling.** The task names $M \in \{256, 1024\}$ as starting
points; $M = 2048$ was added to test whether they are the top, and it is slower,
so the measured set $\{256, 1024, 2048\}$ brackets an interior optimum. Nothing
above 2048 was measured, and this study makes **no claim about what happens
there** — not that it is unsafe, not that it is slower, not that a ceiling
exists.

The decision not to schedule larger sizes was informed by an incident:
`gpu-hang-2026-08-07.log` records a device fault during a single $M = 4096$
attempt on 2026-08-07. That record is committed as **preserved history, and no
claim in this study rests on it**. It cannot bear evidentiary weight: it happened
once, was never retried, produced no CSV row, and predates the harness's first
commit — so the code that ran has no SHA, and the log itself separates what was
captured from what is reconstructed and what is unrecoverable. A single
unreproduced event with no attributable cause supports a precaution, not a
conclusion.

Two consequences for the campaign, both derived from the **cost model and the
measured rates** rather than from that incident:

1. **Per-launch wall-clock is the quantity worth managing.**
   $M \times n \times n$ bytes is trivial at these sizes, so device memory is
   not the binding resource at any $M$ in reach; time-in-kernel is what a batch
   size actually buys or spends, and §4.5's own $M = 2048$ row shows it can be
   spent for nothing.
2. **Per-launch time grows steeply with $n$.** It scales as
   $M \cdot n \cdot 2^n / W$, so holding $M$ fixed from $n = 20$ to $n = 24$
   multiplies per-launch time by $24 \cdot 2^{24} / (20 \cdot 2^{20})$, which is
   **19.2x**, and each further step of 4 in $n$ costs about another 19x. A batch
   size that occupies the device for a comfortable fraction of a second at
   $n = 20$ therefore occupies it for seconds at $n = 24$. A campaign should
   pick $M$ per $(q, n)$ from the measured per-matrix cost and cap per-launch
   time rather than fixing a batch size across the grid — which is what the
   storage layout in §7.4 assumes, and which is prudent whether or not any
   ceiling exists.

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
| 3 | 12 | rayon batch | 280 056 | 222 223 | 0.00 | 22 222 223 | 0.02 |
| 3 | 16 | GPU $M{=}1024$ | 61 306 | 222 223 | 0.00 | 22 222 223 | 0.10 |
| 3 | 20 | GPU $M{=}1024$ | 4 863 | 222 223 | 0.01 | 22 222 223 | 1.27 |
| 3 | 24 | GPU $M{=}1024$ | 310.4 | 222 223 | 0.20 | 22 222 223 | 19.89 (x) |
| 3 | 28 | rayon intra | 19.58 | 222 223 | 3.15 | 22 222 223 | 315.3 (x) |
| 5 | 12 | rayon batch | 72 323 | 160 000 | 0.00 | 16 000 000 | 0.06 |
| 5 | 16 | rayon batch | 4 164 | 160 000 | 0.01 | 16 000 000 | 1.07 |
| 5 | 20 | rayon batch | 213.7 | 160 000 | 0.21 | 16 000 000 | 20.80 (x) |
| 5 | 24 | rayon batch | 11.83 | 160 000 | 3.76 | 16 000 000 | 375.7 (x) |
| 5 | 28 | rayon batch | 0.648 | 160 000 | 68.60 (x) | 16 000 000 | 6 860 (x) |
| 7 | 12 | rayon batch | 72 556 | 122 449 | 0.00 | 12 244 898 | 0.05 |
| 7 | 16 | rayon batch | 3 741 | 122 449 | 0.01 | 12 244 898 | 0.91 |
| 7 | 20 | GPU $M{=}1024$ | 57.93 | 122 449 | 0.59 | 12 244 898 | 58.71 (x) |
| 7 | 24 | Ryser generic | 0.798 | 122 449 | 42.61 (x) | 12 244 898 | 4 261 (x) |
| 7 | 28 | Ryser generic | 0.0426 | 122 449 | 798.4 (x) | 12 244 898 | 79 844 (x) |

"(x)" marks a cell that does not fit the 12 h budget. **Feasible frontier at
SE $= 10^{-3}$: $q{=}3$ to $n = 28$, $q{=}5$ to $n = 24$, $q{=}7$ to $n = 20$.
At SE $= 10^{-4}$: $q{=}3$ to $n = 20$, $q{=}5$ to $n = 16$, $q{=}7$ to
$n = 16$.** The $q = 7$ cells at $n \in \{24, 28\}$ now carry a measured rate
rather than none — the generic Ryser path reaches them where the packed
$\mathbb{F}_7$ kernel does not (§2) — but at 42.6 h and 798 h for a single cell
at SE $= 10^{-3}$ they are four to eighty times the whole budget, so the $q = 7$
frontier is unchanged at $n = 20$. The difference this makes is to the record,
not the plan: an earlier revision reported those cells as having no path at all,
which was a fact about this study's backend list rather than about the tree.

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
| 12 | $4.714\times10^{-6}$ | $10^{10}$ | $4.649\times10^{-6}$ | 1.01 | matches |
| 16 | $1.491\times10^{-5}$ | $10^{9}$ | $9.936\times10^{-6}$ | 1.50 | **exceeds** |
| 20 | $4.714\times10^{-5}$ | $10^{8}$ | $3.528\times10^{-5}$ | 1.34 | **exceeds** |
| 24 | $1.491\times10^{-4}$ | $10^{7}$ | $1.396\times10^{-4}$ | 1.07 | matches |
| 28 | $4.715\times10^{-4}$ | $10^{6}$ | $5.560\times10^{-4}$ | 0.85 | below |

A 12 h budget on this host beats the published precision at $n = 16$ and
$n = 20$, matches it at $n = 12$ and $n = 24$, and falls short at $n = 28$.

**No efficiency claim is drawn from that.** It is tempting to note that the
prior work spent about $3.1 \times 10^3$ processor-hours (one day on 128
processors) against this budget's $\approx 288$ thread-hours, and to attribute
the difference to the kernels. That attribution is not available: the paper
reports a processor *count* and no hardware — no model, clock, or year — so the
per-processor throughput is unknown, and the comparison would in any case pit a
CPU-only run against a budget whose best $q=3$ path is a GPU at $n = 16$
through $24$ but a CPU at $n = 12$ and $n = 28$. Two
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
| 3 | 12 | 5 102 485 | 0.33337 | [0.33296, 0.33378] | $+0.18$ |
| 3 | 16 | 813 201 | 0.33272 | [0.33170, 0.33374] | $-1.18$ |
| 3 | 20 | 74 090 | 0.33266 | [0.32928, 0.33606] | $-0.39$ |
| 3 | 24 | 191 567 | 0.33468 | [0.33257, 0.33679] | $+1.25$ |
| 3 | 28 | 5 812 | 0.34308 | [0.33098, 0.35539] | $+1.58$ |
| 5 | 12 | 649 559 | 0.20018 | [0.19921, 0.20116] | $+0.37$ |
| 5 | 16 | 36 572 | 0.19840 | [0.19435, 0.20252] | $-0.76$ |
| 5 | 20 | 58 848 | 0.20016 | [0.19695, 0.20341] | $+0.10$ |
| 5 | 24 | 500 | 0.17000 | [0.13962, 0.20541] | $-1.68$ |
| 5 | 28 | 106 | 0.24528 | [0.17324, 0.33515] | $+1.17$ |
| 7 | 12 | 646 742 | 0.14252 | [0.14167, 0.14337] | $-0.78$ |
| 7 | 16 | 695 955 | 0.14328 | [0.14246, 0.14411] | $+1.01$ |
| 7 | 20 | 17 819 | 0.13682 | [0.13185, 0.14194] | $-2.30$ |
| 7 | 24 | 10 | 0.10000 | [0.01788, 0.40415] | $-0.39$ |
| 7 | 28 | 5 | 0.00000 | [0.00000, 0.43448] | $-0.91$ |
Grid and sustained samples are both pooled. Sustained runs reserve disjoint
stream ranges — recorded per run in the CSV's `stream_first` column — so their
zero counts are independent of one another and poolable, which was *not* true of
the superseded harness (see the retraction below). Disjointness from the *grid*
streams now holds by construction rather than by audit: the grid hands cell $i$
the range $1 + i \times 10^5$ and so cannot reach past $1.2 \times 10^7$ across
its 120 cells, while `SUSTAINED_STREAM_BASE` places the sustained runs at
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

**One of the fifteen intervals now lies strictly below $1/q$, and it is worth
being precise about what that does and does not mean.** At $q{=}7$, $n{=}20$ the
95 % Wilson interval is $[0.13185, 0.14194]$, entirely under $1/7 = 0.142857$,
at $z = -2.30$. Read against a nominal 95 % interval per cell — the rule an
earlier revision of this study proposed — that is a halt-the-campaign event: an
interval below a proved lower bound.

**Read against the rule §7.2 actually adopts, it is not.** That rule controls the
family-wise error rate across the preregistered cell set, and its per-cell
critical value is $z = 3.16$, which $-2.30$ does not reach. Nor should it: these
fifteen cells are read together, and under a correct pipeline the expected number
falling beyond $2.30\sigma$ on the low side is $15 \times 0.0107 = 0.16$, so one
is unremarkable. **This is the first cell in this study to trip the unadjusted
rule, and it is a live demonstration of why §7.2 does not use it** — the
multiplicity correction was derived before this receipt existed, and the first
sample drawn after it produced exactly the false alarm it predicts.

No cell trips the adjusted rule, in either direction. This cell is the largest
deviation in the table either way; the largest positive is $q{=}3$, $n{=}28$ at
$z = +1.58$, on a sample of 5 812 matrices.

**The upper bound is numeric, and two earlier revisions of this study got it
wrong in opposite directions.** [HKS2026] eq. 1.4 reads
$\Pr[\mathrm{per}(A) = 0] \le 1/q + C/q^3$ for $n \ge 3$. The theorem statement
says "for some absolute constant $C$", but the proof evaluates it: the proof of
Theorem 1.3 derives
$\Pr[\mathrm{per}(A) \ne 0] \ge (1 - q^{-1})(1 - 11q^{-3})$, i.e.
$\Pr \le 1/q + 11/q^3$ — so **$C = 11$** — and Theorem 1.4's proof reaches $11$
independently for its eq. 1.6. So the ceiling is a number at every field order:

| $q$ | ceiling $1/q + 11/q^3$ | allowed excess over $1/q$ | measured $\hat p$ |
|---|---|---|---|
| 3 | $20/27 = 0.7407$ | $0.4074$ | $\approx 0.333$ |
| 5 | $36/125 = 0.2880$ | $0.0880$ | $\approx 0.200$ |
| 7 | $60/343 = 0.1749$ | $0.0321$ | $\approx 0.143$ |

**Every measured cell satisfies it, and none of them tests it.** Among the
well-sampled cells the closest approach to a ceiling is $q{=}7$, $n{=}12$, which
sits **74 standard errors** below its own, and the $q = 3$ cells sit 64 to
1 952 standard errors below theirs. Exactly three cells come within two standard
errors of a ceiling, and all three are tiny:

| cell | matrices | $\hat p$ vs $1/q$ | distance below ceiling |
|---|---|---|---|
| $q{=}7$, $n{=}24$ | 10 | $0.100$ vs $0.143$ | $0.7\sigma$ |
| $q{=}5$, $n{=}28$ | 106 | $0.245$ vs $0.200$ | $1.1\sigma$ |
| $q{=}7$, $n{=}28$ | 5 | $0.000$ vs $0.143$ | $1.1\sigma$ |

Their proximity is a statement about their standard errors, not their values.
Two of the three have point estimates *below* $1/q$, so they sit nowhere near
the ceiling in absolute terms and are near it only because a 5- or 10-matrix
sample admits almost anything. The third, $q{=}5$ at $n = 28$, is the only cell
whose estimate sits meaningfully above its field's $1/q$ — by $0.045$ on 106
matrices, against an allowed excess of $0.088$ — and even it is $1.1\sigma$
short of the ceiling, on a sample too small to carry an inference (§4.7's
by-product caveat applies to all three). At
$q = 3$ the bound is not merely slack but weaker than eq. 1.3, since $0.7407$ is
far above $\alpha_3 \approx 0.4399$; it binds most tightly at $q = 7$ and even
there leaves 23 % headroom in relative terms. **No campaign at any feasible $N$
could contradict eq. 1.4 at these field orders** — doing so at $q = 7$ would
require the true value to exceed $1/q$ by $0.032$, two orders of magnitude
larger than the deviations the campaign is designed to resolve.

Both earlier readings are recorded rather than quietly replaced, per
`@/inv/falsification-preserved`. The first reported the largest observed excess
as an upper bound on $C$, which inverts the inequality: an observed excess
$\delta$ is consistent with the theorem whenever $C \ge \delta q^3$, so a
measurement constrains $C$ from *below* if at all. The second, correcting that,
overshot by calling $C$ unevaluated and concluding the ceiling pins no number —
true of the theorem's statement, false of its proof. What the data support is
that **every measured cell is consistent with [HKS2026] Theorem 1.3 in both
directions**, against a floor of $1/q$ and a ceiling that is numeric, satisfied,
and far too loose here to be informative.

The multiplicity arithmetic above is a **post-hoc calculation on samples that
were never a designed experiment**, and is reported as an order-of-magnitude
sanity check rather than as a test the pipeline passed: the fifteen cells are
timing by-products with no pre-registered $N$, and the threshold compared
against is the one the extreme cell happened to sit at. What it is good for is
sizing the false-alarm rate that §7.2's rule has to survive, which is the use
made of it there.

**The kernel behind that cell is not in question.** §4.1's extended equivalence
check compares the GPU — the backend that drew most of this cell's matrices —
against the generic `permanent_ryser` at exactly $q = 7$, $n = 20$, and they
return byte-identical permanents over 512 matrices. Whatever the cell is, it is
not a wrong kernel.

Two things keep it from being read as a finding. It is a by-product sample of
17 819 matrices with no pre-registered $N$, drawn as the timing
protocol happened to need that many; and the $2.30\sigma$ it exceeds is a
threshold the data chose rather than one fixed in advance. **The campaign should
resample $q{=}7$, $n{=}20$ first**, under a pre-registered $N$ and the adjusted
rule, which is the only way to learn whether anything is there. The pipeline
passes its acceptance test as §7.2 states it, at every measured size.

The two smallest cells deserve the opposite caution: $q{=}7$ at $n = 24$ and
$n = 28$ carry 10 and 5 matrices, the whole output of the generic Ryser path
inside its 120 s cap. Their intervals span most of the unit interval, and they
are listed for completeness rather than for reading.

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
re-measurement puts $q{=}5$, $n{=}16$ at $0.19840$ ($z = -0.76$) where it had
read $0.19310$ ($z = -3.08$). **The apparent signal was measurement artefact and
sampling fluctuation, not structure.** Recorded rather than quietly dropped, per
`@/inv/falsification-preserved`: the earlier reading is what the study said, and
this is what independent re-measurement returned.

**What still holds.** These remain by-product samples with no pre-registered $N$
or stopping rule, so no inference rests on them either way, and the smallest
cells are very small indeed: $q{=}7$ at $n = 24$ and $n = 28$ carry 10 and 5
matrices, $q{=}5$ at $n \ge 24$ a few hundred, and $q{=}3$ at $n = 28$ a few
thousand.
Four correctness anchors continue to pass and are the reason the table can be
read at all: the $q = 3$ arm agrees with [Scheinerman2024] at every $n$; the
kernels match an independent six-term permanent over all $q^9$ order-3 matrices,
with F_3 reproducing $z(3) = 8163$; sampler and kernel together recover the
exact order-3 zero fraction within $4\sigma$ over $4 \times 10^5$ draws per
field, receipted in `order3-anchor-2026-08-08.txt` from
`order3-anchor-check.py`, which enumerates the exact fractions independently in
Python — $907/2187$, $17581/78125$ and $126295/823543$, reproducing
[Scheinerman2024]'s $z(3) = 8163$ — and then reproduces the anchor's draw
through the harness's own sampler and kernels, at seed root `0xB488_F02C`,
stream 12 345. The observed estimates are $0.413127$, $0.224820$ and $0.153390$
against exact values of $0.414723$, $0.225037$ and $0.153356$, at
$z = -2.05, -0.33, +0.06$; and no deviation localises to a single backend.

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
| G1 | Uniform $\mathbb{F}_q$ matrix sampler | Landed as [`gf2_stats::sampler`](../../../crates/gf2-stats/src/sampler.rs); prototype retained as historical context | 0.5 d (historical) |
| G2 | Streaming zero-fraction statistics with CIs | Prototyped, needs checkpointing | 1.0 d |
| G3 | Campaign runner | Missing; design decided below | 2.0 d |
| G4 | Versioned dataset format | Missing | 0.5 d |
| G5 | Permanental-rank predicate for the rectangular check | Landed as [`permanent::permanental_rank_status`](../../../crates/gf2-algebra/src/permanent/rank.rs); **no new kernel needed** | 1.0 d (historical) |
| G6 | `permanent_bipedal3` selects the slower path | Defect, confirmed by measurement | 0.5 d |
| G7 | No batch-parallel path for any field | Missing | 0.5 d |
| G8 | $q=7$ CPU ceiling at $n = 16$ | Structural limit | 4.0 d (optional) |

**G1 — uniform sampler.** The feasibility harness implemented exact rejection
sampling over ChaCha20 (`sampler.rs`). Its production successor is the
[`gf2_stats::sampler` rustdoc](../../../crates/gf2-stats/src/sampler.rs), which is
the canonical contract for its `MatrixAddress` and `MatrixSampler` surfaces.
Rejection is required because the sampler consumes bytes and
$256 \bmod 7 = 4$: reducing a byte without rejection would give four residues
probability $37/256$ and three of them $36/256$, against a uniform $1/7$. In
relative terms the worst class is off by **1.56 %** (the most and least likely
differ by 2.78 %). The campaign's tightest target, a standard error of $10^{-4}$
on $\Pr[\mathrm{per} = 0] \approx 1/7$, is **0.07 %** of that same reference. The
entry-level bias is therefore about **22 times** the resolution being aimed at —
comfortably decisive, and the reason rejection is not optional.

An earlier revision put that ratio at "three orders of magnitude", which is wrong
by two: it compared a *relative* imbalance of 2.8 % against an *absolute*
standard error of $10^{-4}$, quantities in different units. The conclusion is
unchanged — the bias dwarfs the effect either way — but the figure was load-bearing
for the claim that rejection matters, so the correction is recorded rather than
silently swapped. Both numbers above are now relative to $1/7$.

The production surface carries the entry-uniformity, stream-separation, and
reproducibility tests; the prototype remains the historical feasibility source.

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
pooled summary, and checksums. Because matrices are regenerable from their full
`MatrixAddress` (root, $q$, $n$, purpose, and stream index), shards store the
$q$-bin histogram of permanent values
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
therefore needs **no new kernel** — it enumerates row subsets and calls the
existing square $k \times k$ kernel on each, returning at the first nonzero
permanent (which is the overwhelmingly common case, so the expected work per
matrix is a small constant number of $k \times k$ permanents rather than
$\binom{n}{k}$ of them). *1.0 d.*

This landed as
[`permanent::permanental_rank_status`](../../../crates/gf2-algebra/src/permanent/rank.rs)
in `gf2-algebra`, not in the campaign driver as this section first projected.
The predicate decides one matrix and carries no dependency on sampling or
statistics, so it belongs beside the square permanent kernels it calls; the
driver consumes it. Its cross-check oracle is
`gf2_algebra::testutil::permanental_rank_bruteforce`, which shares no code path
with it.

**The theorem's regime is not reachable by direct sampling, and the study says
so.** [GGK2025] Theorem 2.1 requires $k \le 0.1\sqrt{n}$, so even $k = 3$ needs
$n \ge 900$. The obstacle there is not Ryser cost — the submatrices are $3\times3$
— but the event probability itself: $\Pr \sim k/q^n \approx 3 \cdot 3^{-900}$,
which no Monte Carlo campaign can observe. Epic REQ-04 must therefore be read as
a **pipeline correctness check**, not a test of Theorem 2.1 in its proven range:

1. exact enumeration of the rank predicate on tiny $(n, k, q)$, and
   cross-implementation agreement against an independent brute-force
   permanental-rank routine — **done**, over every matrix at
   $(q,n,k) \in \{(3,3,1),(3,3,2),(3,4,2),(5,3,2),(7,3,2)\}$, in
   `crates/gf2-algebra/tests/permanental_rank.rs`;
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

**G8 — $q = 7$ above $n = 16$.** `Packed7` fits 16 lanes in a `u64`, so the
*packed* CPU kernel stops at $n = 16$. Two paths remain beyond it: the GPU, and
the generic `permanent_ryser`, which §4.4 measures at 0.798 matrices/s at
$n = 24$ and 0.0426 at $n = 28$. Correct, and too slow — but by a factor worth
naming rather than waving at. Against the SE $= 10^{-3}$ target, which needs
$122\,449$ samples inside the 12 h budget's $3.672 \times 10^4$ productive
seconds and so demands **3.34 matrices/s**, the measured rates fall short by
**4.2x at $n = 24$** and **78x at $n = 28$**. Against SE $= 10^{-4}$ the same
comparison is 418x and 7 800x. So $n = 24$ is within an order of magnitude of
the cheaper target and only the harder target is hopeless there. A multi-word
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

**Is a determinantal companion curve worth the marginal cost?** **Yes, on an
estimate that the campaign should confirm before relying on it.** Gaussian
elimination is $O(n^3)$ per matrix against the permanent's $\Theta(n 2^n)$, so
the work ratio goes as $n^2 / 2^n$: roughly $4 \times 10^{-4}$ at $n = 20$ and
$3 \times 10^{-6}$ at $n = 28$. **That is arithmetic on the complexity
expressions, not a measurement** — no determinant path was benchmarked for this
study, and the constant factors, which differ between a bit-sliced Gray-code
walk and an elimination over $\mathbb{F}_q$, are unknown. It is enough to say
the companion is cheap at the sizes in the envelope and not enough to call it
free; the campaign should time one determinant cell against its permanent cell
before adopting the assumption.

**It also supplies a pipeline validation the permanent cannot — but the
comparison must be against the exact value at each $n$, not against the limit.**
The count of invertible $n \times n$ matrices over $\mathbb{F}_q$ is
$\prod_{k=0}^{n-1}(q^n - q^k)$, since row $k$ may be any vector outside the span
of the $k$ already chosen. Dividing by $q^{n^2}$ and substituting $i = n - k$
gives the exact singular probability at every finite $n$:

$$
\Pr[\det(A) = 0] \;=\; 1 - \prod_{i=1}^{n}\left(1 - q^{-i}\right).
$$

This was checked against brute-force enumeration over all $q^{n^2}$ matrices for
$(q,n) \in \{(3,1),(3,2),(3,3),(5,2),(5,3),(7,2)\}$ and agrees exactly as a
rational — for instance $11/27$ at $q{=}3$, $n{=}2$ and $313/729$ at $n{=}3$.
The check is committed as `determinant-anchor-check.py` with its output in
`determinant-anchor-2026-08-08.txt`, so the agreement is re-runnable rather than
asserted here; it enumerates exhaustively and takes no seed, so its output is a
function of the case list alone.
Its $n \to \infty$ limit is $\alpha_q = 1 - \prod_{i \ge 1}(1 - q^{-i})$,
which is [HKS2026] Fact 1.1 and the value §1 quotes.

**An earlier revision of this study proposed testing the measured curve against
that limit. That would have manufactured failures at small $n$**, where the
campaign's samples are largest and its intervals tightest. The limit overstates
the exact value by $3.5 \times 10^{-3}$ at $q{=}3$, $n{=}4$, by
$3.0 \times 10^{-4}$ at $q{=}5$, $n{=}4$, and by $3.8 \times 10^{-4}$ at
$q{=}3$, $n{=}6$ — deviations tens to hundreds of standard errors wide at cells
that cost minutes to sample; the artifact tabulates the full gap. The gap falls
below $10^{-6}$ only from about
$n = 12$ at $q = 3$ and earlier for larger $q$, so a limit-based check is
harmless exactly where it is uninformative and wrong exactly where it would fire.

The acceptance test is therefore stated against the exact finite-$n$ value and
carries the same error control as §7.2's permanent test: a determinant cell
fails when its measured $\hat p_{\det}(n)$ lies more than the Bonferroni-adjusted
critical $z$ from $1 - \prod_{i=1}^{n}(1 - q^{-i})$, with the level split across
the same preregistered cell set. This one is **two-sided** — unlike
$\Pr[\mathrm{per} = 0] \ge 1/q$, which is a one-sided bound, the determinant's
value is pinned exactly, so a deviation in either direction indicts the pipeline
— which puts each cell at $\alpha / 2K$ and a critical $z$ of $3.36$ at
$\alpha = 0.05$, $K = 63$. A determinant curve that misses its own exact values
falsifies the pipeline before any permanent conclusion is drawn, and unlike the
order-3 permanent anchors it does so at every $n$ the campaign measures.

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

1. **Adopt [HKS2026] Theorem 1.3 as a standing acceptance test, at a controlled
   family-wise error rate.** The theorem proves $\Pr[\mathrm{per} = 0] \ge 1/q$
   for every $n$ at odd characteristic, so a cell that sits significantly below
   $1/q$ indicts the pipeline rather than the theorem, and a below-$1/q$ result
   can never be reported as a finding. Run it on every cell for the campaign's
   life: it costs nothing to evaluate and is sharper than the order-3
   enumeration anchors, which say nothing at the sizes the campaign cares about.
   Run it at §7.2's Bonferroni-adjusted level rather than at a nominal 95 %
   interval per cell — unadjusted, the grid's 63 cells would halt a correct
   pipeline about four runs in five. §4.7 is the argument for it —
   an earlier revision of this study spent its top recommendation on chasing an
   apparent below-$1/q$ signal that turned out to be two measurement defects
   plus fluctuation, and this test would have classified it correctly on sight.
2. **The $q = 3$ arm is a reproduction-and-precision target, not new ground.**
   [Scheinerman2024] already publishes this curve to $n = 30$. This budget beats
   his precision at $n = 16$ and $n = 20$, matches at $n = 12$ and $n = 24$, and
   falls short at $n = 28$; extension to $n = 30$ misses the 12 h budget by
   about a third at SE $= 10^{-3}$ and by two orders of magnitude at
   $10^{-4}$, on the projections §7.6 derives and labels. The $q = 3$
   contribution is tighter intervals at two sizes, intervals where the source
   publishes none, an independently reproducible artifact trail, and a
   cross-implementation check of a published result — all worth doing, none of
   them "first numerics".
3. **The novelty is $q \in \{5, 7\}$**, for which a documented search
   (`literature-search-2026-08-08.md`) found no published numerics, and where
   the envelope reaches $n = 24$ and $n = 20$
   respectively at SE $= 10^{-3}$. Note what [HKS2026] leaves open and what it
   does not (§7.1): the floor $\Pr \ge 1/q$ is proved at every $n$, and the
   ceiling $\Pr \le 1/q + 11/q^3$ is proved for $n \ge 3$ — a real number, but
   $0.03$ to $0.41$ above the floor, so *how close* the finite-$n$ value sits to
   $1/q$ at these field orders is exactly what is not settled. That is the
   campaign's quantity: the sign and size of $\delta(n) = \Pr - 1/q$, which no
   proved bound pins to better than two orders of magnitude. Framing a
   $q \in \{5,7\}$ result as "confirming the limit is $1/q$" would claim
   credit for a conjecture nobody has proved.
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
$n \to \infty$. **No finite grid of $n$ can test it**, in either direction, and
this study does not claim otherwise. An asymptotic statement is compatible with
*any* finite prefix of the sequence, so no measurement at $n \le n_{\max}$ can
confirm [GGK2025]'s conjecture and none can contradict it either.

**What the proved results give at finite $n$ is narrower than "the value is near
$1/q$".** [HKS2026] Theorem 1.3 gives two finite-$n$ facts and one asymptotic
one, in their published directions:

- $\Pr[\mathrm{per}(A) = 0] \ge 1/q$ for **all** $n$ (eq. 1.2) — a hard floor,
  and the only one of the three that the campaign can check per cell;
- $\Pr[\mathrm{per}(A) = 0] \le 1/q + C/q^3$ for all $n \ge 3$ (eq. 1.4), with
  **$C = 11$** derived in the proof though the statement says only "some
  absolute constant";
- $\limsup_{n} \Pr[\mathrm{per}(A) = 0] < \alpha_q$ (eq. 1.3), where $\alpha_q$
  is the determinant's limiting zero probability — an asymptotic separation from
  the determinant, not a statement about proximity to $1/q$.

Eq. 1.4 therefore does pin a number — $0.7407$, $0.2880$ and $0.1749$ at
$q = 3, 5, 7$ — but not a useful one at these field orders: each ceiling sits
$0.03$ to $0.41$ above $1/q$, while the deviation the campaign measures is of
order $10^{-3}$ or smaller (§4.7). So it is **not** established that the
finite-$n$ value sits *numerically close* to $1/q$ in any sense the campaign
cares about; what is established is a floor at $1/q$, a ceiling whose allowed
excess over $1/q$ runs from 32 to 4 100 times the campaign's target standard
error — $407\times$ and $4\,074\times$ the
$10^{-3}$ and $10^{-4}$ targets at $q = 3$, $88\times$ and $880\times$ at
$q = 5$, $32\times$ and $321\times$ at $q = 7$ — so between one and a half and
three and a half orders of magnitude too loose to constrain $\delta(n)$, and an
asymptotic gap from $\alpha_q$. Conjecture 1.2 — that the limit is exactly
$1/q$ — is explicitly stated by [HKS2026] as unproved.

**The campaign's target is therefore the shape of the finite-$n$ correction**,
$\delta(n) = \Pr[\mathrm{per}(A) = 0] - 1/q$, over the range it can measure: its
sign, its magnitude, and how it varies with $n$ and $q$. That is a real and
unmeasured quantity, and §7.5's model comparison is a statement about it. What
the campaign can falsify is a **model** of $\delta(n)$ — a geometric decay
against a polynomial one, say — and its own pipeline, via the per-cell floor of
eq. 1.2. What it cannot falsify is either the conjecture or the theorem: the
first is asymptotic, and the second is a proved statement that finite data can
only be consistent with. Every conclusion must be phrased at the measured sizes.

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
- **Standing acceptance test, with its false-alarm rate controlled.** Every cell
  is checked against $\Pr[\mathrm{per} = 0] \ge 1/q$, proved for all $n$ at odd
  characteristic by [HKS2026] Theorem 1.3. A cell that fails halts the campaign
  for pipeline investigation rather than being reported.

  The test must be built so that a *correct* pipeline almost never trips it,
  which the naive form does not achieve. A 95 % interval misses its parameter
  5 % of the time, and only the lower excursion can trip this one-sided test, so
  a single cell whose true value sits at $1/q$ — the worst case the theorem
  allows — flags with probability up to 2.5 %. Across the preregistered grid
  above ($n$ from 4 to the per-$q$ frontier: 25 cells at $q=3$, 21 at $q=5$, 17
  at $q=7$, so $K = 63$) that compounds to a **79 % chance of at least one false
  alarm**, which would halt a healthy campaign nearly every time it ran.

  The rule is therefore stated with an explicit family-wise error rate.
  Bonferroni across the $K$ preregistered cells at a family-wise
  $\alpha = 0.05$ puts each cell's one-sided level at $\alpha / K = 7.9 \times
  10^{-4}$, a critical $z$ of **3.16**, and a family-wise false-alarm
  probability under a correct pipeline of **at most 4.9 %**. A cell halts the
  campaign when $\hat p$ lies below $1/q$ by more than $3.16$ standard errors —
  equivalently, when the Bonferroni-adjusted one-sided interval excludes $1/q$ —
  not when the nominal 95 % interval does.

  Three consequences are accepted rather than argued away. $K$ is fixed by
  pre-registration, so cells added later require restating the adjustment rather
  than reusing this one. The adjustment costs sensitivity: a real defect must be
  larger to trip a single cell, which is tolerable because a defect that matters
  is systematic and will show across many cells at once, and the campaign should
  read the *pattern* of deviations, not only the extreme cell. And the
  companion upper bound $\Pr \le 1/q + 11/q^3$ is **not** part of the test. It
  is a real ceiling — $0.1749$ at $q = 7$, the tightest of the three — but the
  campaign's cells sit tens to thousands of standard errors below it, so
  checking it would never fire and would never inform (§4.7).
- **Determinant companion** on the same matrices, per §6, checked against the
  **exact** finite-$n$ singular probability $1 - \prod_{i=1}^{n}(1 - q^{-i})$ —
  never against its $n \to \infty$ limit, which differs from it by far more than
  a campaign standard error at small $n$ — at a two-sided Bonferroni level
  ($\alpha / 2K$, critical $z = 3.36$) matching the permanent test's error
  control.
- **Rectangular validation** (epic REQ-04) on the landed G5 predicate, framed as
  the three-part pipeline check described there rather than as a test of
  [GGK2025] Theorem 2.1 in its proven regime. Part 1 of that check — exhaustive
  enumeration and oracle agreement — is already committed with the predicate;
  parts 2 and 3 remain campaign work.
- **Comparison against [Scheinerman2024] Table 4 at $q = 3$** as a first-class
  output, per §7.6.

### 7.3 Seeding scheme

The production [`gf2_stats::sampler` rustdoc](../../../crates/gf2-stats/src/sampler.rs)
is the source of truth. Its `MatrixAddress` seeds ChaCha20 (`rand_chacha` 0.9)
from four little-endian `u64` words: campaign root, $q$, $n$, and a final word
whose high eight bits are a closed `StreamPurpose` tag and whose low 56 bits are
a validated stream index. A campaign gives each independently generated matrix
a distinct full address; within one purpose, shards use their own validated
indices. Validation is a purpose, not a reservation of stream 0.

**Stream allocation must be structurally disjoint, not coincidentally so.**
This study's own history makes the point: the harness first based its sustained
runs at $10^6 + j \times 10^5$ against a grid reserving $1 + i \times 10^5$ per
cell, which are commensurate — the ranges collided in index space, and the
pooled counts of the day were valid only because no colliding pair happened to
share a $(q, n)$. That was verified, not designed. Raising the sustained base to
$10^9$ (§4.7) fixed those historical receipts by putting the two allocations two
orders of magnitude apart; it is not an allocation rule for the production
sampler. A campaign running for twelve hours per cell cannot audit
its way out of this: the full address — including the closed purpose tag —
separates generator-state domains by construction. This guarantees address and
state separation; it does not make sampled matrix values injective, so distinct
addresses can validly produce equal matrices. Three properties follow, and all
three matter operationally: any shard is reproducible from its full address, a
lost or corrupted shard is redrawn without touching its neighbours, and shards
can be produced in any order or concurrently without coordinating a shared
generator state. The matrix layout
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
2. **Extension beyond $n = 30$: not available here, by a margin that depends on
   the target.** His table already reaches $n = 30$. Projecting this study's
   fastest measured $q = 3$ path at $n = 28$ — intra-matrix rayon, 19.58
   matrices/s — through Ryser's $n \cdot 2^n$ work model gives an **estimated**
   4.57 matrices/s at $n = 30$; the GPU at $M = 1024$ projects to 4.50 from
   19.27. **These are projections, not measurements**, formed by the same
   machinery as §4.3's censored cells and from the reference measurements named
   here. Their bias is only partly validated: §4.3's 14-18 % low reading comes
   from the $q = 3$ GPU chain, so it transfers to the GPU-based estimate here —
   one step further along the same field, kernel and batch size — but not to the
   intra-matrix rayon estimate, whose backend was never checked and which pays
   no per-launch cost for a longer kernel to amortise. Treat the direction as
   argued and the magnitude as loose. Against the 12 h budget's
   $3.672 \times 10^4$ productive seconds, SE $= 10^{-3}$ at $n = 30$ needs an
   estimated 13.5 h — over budget by about a third, a gap a 14-18 % correction
   would not close even if it applied — and SE $= 10^{-4}$ needs an estimated
   1350 h, over by two orders of magnitude. So $n = 30$ is out of reach at both
   targets, but only the $10^{-4}$ figure deserves the phrase "by orders of
   magnitude"; at
   $10^{-3}$ it is a near miss that a larger budget would reach. This campaign
   does **not** extend the $q = 3$ curve in $n$, and no wording in the final
   report should imply otherwise.
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

The $q \in \{5, 7\}$ arms have no published baseline that a documented search
found. The queries, engines, dates, examined works and reading depth per work
are recorded in `literature-search-2026-08-08.md`, together with that search's
limits: it ran one general web index rather than a bibliographic database, and
three of the closest candidate works — `@/citation/Budrevich2018`,
`@/citation/BudrevichGuterman2012` and `@/citation/Bassalygo2013`, all counting
papers of the kind that might carry a small table without advertising it — were
never read. Corroborating it, [HKS2026] §1 reports that its own authors "were
not able to find any study of the asymptotic distribution of
$\mathrm{per}(A)$" in this literature, and cites only [Scheinerman2024]'s
$\mathbb{F}_3$ work as computational evidence.

The claim this supports is **conditional on that search**: a recorded search
found no prior numerics at $q \in \{5, 7\}$, subject to the limits it states.
It is not a claim that none exist, that every measured cell is new, or that this
campaign has priority. Reports should say what was searched and what it
returned, and leave the coverage for a reader to judge.

## References

- [GGK2025] Ghasemi, Gross, Kopparty — Permanental Rank versus Determinantal Rank
  of Random Matrices over Finite Fields. APPROX/RANDOM 2025. arXiv:2512.03221;
  ECCC TR25-206.
- [HKS2026] Hunter, Kwan, Sauermann — Permanents of Random Matrices over Finite
  Fields. arXiv:2603.15856.
- [Scheinerman2024] Scheinerman — Fast Computation of Permanents over F_3 via F_2
  Arithmetic. arXiv:2407.20205.
