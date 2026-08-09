# Investigation — epic b8206228 (empirical permanent statistics)

Read-only verification of the current tree against the claims carried into
planning from `dev/studies/b488f02c/feasibility-study.md` (GO verdict,
2026-08-08). Every finding below cites a file and line checked in this session;
where a claim could not be settled from code that is said explicitly.

Working tree at time of investigation: branch `main`, HEAD `f5c01332`, dirty
only in `.jit/` (events log and the b8206228 issue file).

## 1. Claim classification

### C1 — sampler prototyped in `dev/research/permanent-sampling-feas/`; no production uniform F_q sampler

**Valid, with one correction that matters for the plan.**

Prototype confirmed. `MatrixSampler` draws by exact rejection over ChaCha20
(`dev/research/permanent-sampling-feas/src/sampler.rs:107`, RNG constructed at
`:147`), with `accept_bound` at `:87` and the 32-byte `(root, q, n, stream)`
little-endian seed block assembled in `derive_seed`
(`dev/research/permanent-sampling-feas/src/sampler.rs:97-104`). The crate pins
`rand_chacha = "0.9"` and `rand_core = "0.9"`
(`dev/research/permanent-sampling-feas/Cargo.toml:49-51`). Entry-uniformity and
stream-separation tests exist: `entries_are_uniform_within_sampling_error`
(`sampler.rs:311`, six binomial standard errors over 3e5 draws per field for
q ∈ {3,5,7}) and `streams_are_reproducible_and_domain_separated`
(`sampler.rs:295`). Eight `#[test]` functions in that file in total.

`gf2_algebra::testutil::random_matrix` uses `Lcg::next_u64() % P` exactly as
claimed (`crates/gf2-algebra/src/testutil.rs:38-43`), and `Lcg` is the MMIX LCG
in `crates/gf2-core/src/rng.rs`.

**Correction:** the tree does contain a second, production, default-enabled
uniform F_q matrix generator that the study's G1 section does not mention:
`FieldMatrix::random` (`crates/gf2-core/src/field/matrix.rs:596`) and
`FieldMatrix::random_seeded` (`:617`, `rand::rngs::StdRng`). Its element
distribution is `Fp::<P>::new(rng.gen::<u64>())`, i.e. `u64 % P`
(`crates/gf2-core/src/field/matrix.rs:566-568`), so its modulo bias is the
negligible `q·2^-64`, and it is behind the `rand` feature which is **default-on**
(`crates/gf2-core/Cargo.toml:41`). So the accurate statement is not "no
production uniform sampler exists" but "no production sampler with
domain-separated, reproducible stream addressing exists": `random_seeded` takes
only a `u64` seed, offers no `(root, q, n, stream)` partition, produces
`FieldMatrix<Fp<P>>` rather than the packed matrix types the kernels consume,
and `StdRng` carries no cross-version reproducibility guarantee. The plan should
state G1 that way; a reviewer who greps `random` in `gf2-core` will otherwise
read the study's phrasing as falsified.

### C2 — Wilson intervals in `stats.rs`; no checkpointable accumulator; no Clopper–Pearson in-tree

**Valid.** `wilson_interval` at
`dev/research/permanent-sampling-feas/src/stats.rs:38`, `Z_95` at `:52`. The
module has no accumulator type and no serde: `EnvelopeRow` derives only
`Clone, Debug` (`stats.rs:69-70`) and leaves the crate through a
`to_csv_row() -> String` method (`stats.rs:108`). A repo-wide search for
Clopper–Pearson returns nothing outside prose; every in-tree interval is Wilson.
Test coverage in `stats.rs` is 6 tests (`:186`–`:232`), none about restart.

### C3 — no campaign runner for permanent sampling; the FEC campaign runner is coding-domain

**Valid as to the gap, but the study names the wrong crate and misses the closer
precedent.**

There are **two** campaign runners in-tree, and the study's G3 discusses only the
first:

1. `crates/gf2-coding/src/bin/sim_runner.rs` — the TOML-driven FEC campaign
   runner. `CampaignConfig` at `:132`, `CurveConfig` at `:159`, `SnrRange` at
   `:204`, `TurboConfig` at `:235`. Schema is thoroughly coding-domain (codes,
   decoders, SNR sweeps). It reads campaign TOMLs given as a positional argument
   (`:307`, parse at `:383`). This is the runner the study calls "the `gf2-sim`
   FEC campaign runner" — it lives in `gf2-coding`, not `gf2-sim`. Fix the
   attribution in any text that repeats it.
2. `crates/gf2-sim/src/bin/dvb_t2_awgn_campaign.rs` — the DVB-T2 BICM campaign
   binary from issue `152388f4` (done). CLI-arg driven, not TOML: `--rate`,
   `--modulation`, `--esn0-range`, `--target-errors`, `--max-frames`, `--seed`,
   `--output-dir`, `--resume`, `--heartbeat-frames` (`:226`-`:244`). It writes a
   per-curve output directory containing the curve CSV, `tracing.jsonl`,
   a generated `README.md`, and `checkpoints/` (`:154`), and drives
   `Scheduler::run_sweep_checkpointed`.

**This second runner, not the first, is the structural precedent for a
permanent-statistics driver**, and it is the one the convention-convergence
question in §6 turns on.

### C4 — no versioned dataset format; `dev/campaigns` is a permanent documentation path

**Valid on the gap; the proposed location conflicts with the area's declared
purpose.**

`dev/campaigns` is in `[documentation] permanent_paths`
(`.jit/config.toml`, `permanent_paths` list) — the study's citation is correct.
But `dev/index.md:42` classifies it as "Simulation campaign **definitions**",
while `dev/index.md:46` classifies `simulation_results/` as "Campaign
**outputs**", also permanent. Today `dev/campaigns/` holds 50 `*.toml` campaign
*configs* and nothing else; the only active consumer is `sim_runner` taking the
path as an argument, plus a surface test asserting which TOMLs exist
(`crates/gf2-coding/tests/grand_phase1_smoke.rs:409-413`). So §7.4's proposal to
publish the *dataset* under `dev/campaigns/permanent-zero-fraction/<id>/`
(`dev/studies/b488f02c/feasibility-study.md:1464`) puts an output into the
definitions area. Either the plan justifies the departure explicitly, or it
places the dataset under `dev/simulation_results/` (equally permanent, and the
index's declared home for campaign outputs). This is worth deciding before
breakdown — it is the kind of detail doc-review surfaces late.

**No manifest/checksum convention exists anywhere in-tree.** A search for
`*.sha256`, `checksums*` and `manifest*` under `dev/` and `docs/` returns
nothing. The closest precedent is the DVB-T2 per-curve `README.md`
(`dev/benchmarks/dvb_t2_awgn/curve_1_2_16qam/README.md`), generated at
`crates/gf2-sim/src/bin/dvb_t2_awgn_campaign.rs:692`, which records invocation,
configuration, host (`whoami` + `uname -a`, `:661`) and wall-clock — and
**records no git revision and no toolchain version**. Since
`@/inv/claims-trace-to-artifacts` demands seeds, git revision, hardware and
toolchain, the campaign's manifest is genuinely new work rather than an
adaptation, and it is also a chance to fix that gap at its source.

### C5 — no permanental-rank-deficiency predicate in-tree; decomposes into k×k square permanents

**Valid; nothing has appeared since.** No `per_rank`, `permanental_rank`, or
equivalent exists in `crates/`. The public permanent surface is exactly seven
functions (`crates/gf2-algebra/src/permanent/mod.rs:31-60`): `permanent_ryser`,
`permanent_mod3_reference`, `permanent_bipedal3`, `permanent_bipedal3_singleword`,
`permanent_bipedal3_multiword`, `permanent_bipedal3_parallel` (feature
`parallel`), `permanent_bipedal5` (feature `f5`), `permanent_bipedal7` (feature
`f7`), plus the three GPU batch entry points in `crates/gf2-algebra/src/gpu.rs`.
All are square-only. The decomposition argument in the study
(`feasibility-study.md:1096-1111`) needs no new kernel and is unaffected by
anything in the current tree.

**Closed by jit issue `175972df`.** The finding above records the tree as it
stood at investigation time and is kept as written. Since then
`permanent::permanental_rank_status` and its brute-force oracle
`testutil::permanental_rank_bruteforce` landed in `gf2-algebra`, confirming the
decomposition argument: the predicate adds no numeric kernel and calls
`permanent_ryser` on each $k \times k$ row submatrix.

### C6 — scalar single-matrix selection and separate AVX2 APIs

**Resolved in the current tree, with the original measurement preserved.**

The public `permanent_bipedal3` dispatcher now calls
`permanent_bipedal3_singleword` directly for `n <= 63`, so enabling the default
`simd` feature does not change the single-matrix selection. The AVX2 paths are
separate APIs: `permanent_bipedal3_singleword_simd` remains directly available
for kernel conformance, while `permanent_bipedal3_batch` evaluates one to four
matrices together through `Bipedal3x4` when AVX2 is available and otherwise
falls back safely to scalar evaluation.

The current selection rests on the evidence that motivated the original
finding:

- `crates/gf2-algebra/src/permanent/bipedal3.rs` documents and implements the
  scalar public single-matrix route and the separate direct and batched AVX2
  entry points.
- `crates/gf2-algebra/examples/s3_scalar_vs_avx2_sanity.rs` measures the direct
  kernels and records that the single-matrix AVX2 kernel is about three times
  slower than scalar for `n in {16, 20, 24}`.
- `dev/benchmarks/gf2_algebra_permanent/csvs/s3_cross_cpu.csv` preserves the
  measured scalar-to-AVX2 ratios of 0.317–0.319.

The plan absorbed both parts of the finding: the public dispatcher was moved to
the faster scalar kernel, and the four-matrix SIMD entry point was built as the
throughput-oriented user of the existing `Bipedal3x4` arithmetic type. This
keeps the measured selection, direct conformance API, and batched SIMD API
distinct in both code and prose, as required by `@/inv/single-source-prose`.

### C7 — no batch-parallel path for any field; intra-matrix rayon only for F_3

**Valid.** `permanent_bipedal3_parallel` at
`crates/gf2-algebra/src/permanent/parallel_bipedal3.rs:102` splits one matrix's
Gray-code walk. The follow-up note is at
`crates/gf2-algebra/src/parallel.rs:5-7` — note the path: the file is
`src/parallel.rs`, **not** `src/permanent/parallel.rs` as the study's citation
implies; `crates/gf2-algebra/src/permanent/parallel.rs` does not exist. The
module is described at `:11-14` as "a top-level parallel scaffold for future
parallel algorithms". `permanent/mod.rs:42-46` gates only `parallel_bipedal3`;
no F_5/F_7 parallel module is declared.

### C8 — `Packed7::LANES = 16`; generic `permanent_ryser` the only F_7 CPU path above n=16

**Valid, and the "CPU" qualifier is load-bearing.** `pub const LANES: usize = 16`
at `crates/gf2-algebra/src/packed/packed7.rs:216` (re-exported as the trait
constant at `:593`). `permanent_bipedal7` asserts against it at
`crates/gf2-algebra/src/permanent/bipedal7.rs:110`. `permanent_ryser` is at
`crates/gf2-algebra/src/permanent/ryser.rs:89`. The GPU batch path
`permanent_batch_bipedal7` (`crates/gf2-algebra/src/gpu.rs:467`) also supports
F_7 to n ≤ 63, so generic Ryser is the only *CPU* path above n=16, not the only
path. Drop the qualifier and the claim is false.

### C9 — composite hot path exists in the harness and is liftable

**Valid.** `one_rep` in
`dev/research/permanent-sampling-feas/src/protocol.rs:315` times the four phases
in order: `generate` → `evaluate` → `histogram` (`:293`) + `count_zeros` →
shard line `writeln!` + `flush` (`:335-343`). The shard record is
`q,n,m,stream,zeros,<q histogram bins>`. The module docs state the same
contract at `protocol.rs:11-14`.

**Caveat for the lift:** `protocol.rs` carries **zero** `#[test]` functions, as do
`backend.rs`, `env.rs` and `main.rs`. Tested modules are `sampler.rs` (8),
`prior.rs` (6), `stats.rs` (6), `equivalence.rs` (4). So the two modules the plan
most wants to productionise — sampler and stats — are the tested ones, and the
composite loop is not. Budget test-first work for the driver rather than
treating it as a transcription.

### C10 — exact-enumeration anchors exist under `dev/studies/b488f02c/`

**Valid.** `order3-anchor-check.py` and its committed output
`order3-anchor-2026-08-08.txt`; `determinant-anchor-check.py` and
`determinant-anchor-2026-08-08.txt`; and the standalone `anchor-report/` crate
whose `main.rs:15-27` links the harness library and pins `SEED_ROOT =
0xB488_F02C`, `STREAM = 12_345`, `DRAWS = 400_000`, `N = 3`. The exact-count
function it calls is `permanent_sampling_feas::equivalence::exact_zero_count_order3`.

**Placement problem the plan must solve.** `dev/studies` is in
`issue_scoped_areas` (`.jit/config.toml`), so these anchors are attached to the
closed study `b488f02c` and are archival candidates. The campaign's REQ-04
("validated by exact enumeration and cross-implementation agreement") needs
enumeration anchors that live where the campaign can re-run them — a permanent
area (`dev/scripts`, `dev/research`) or in-crate tests. Re-siting or
re-implementing them is real work that the study's G-table does not price.

### C11 — in-tree F_q determinant usable for the companion curve

**Confirmed present; the study's §6 open question can be closed by code read.**

`FieldMatrix::det` at `crates/gf2-core/src/field/inverse.rs:494`, with the
free-function alias `det<F: FiniteField>` at `:586`. Implementation is
Dumas–Pernet via PLE with an explicit permutation sign (`:517-533`), returning
field zero at rank < n; documented complexity `O(n³)` (`:478`). It is generic
over `FiniteField`, and the rustdoc example is over `Fp<7>` (`:486-492`), so
q ∈ {3,5,7} are covered by the same `Fp<P>` used by the samplers. Supporting
`ple` / `rref` / `rank` live in `crates/gf2-core/src/field/ple.rs`.

**Measured performance:** Criterion benches exist but not at the campaign's
parameters — `inverse/det` covers `Fp<MERSENNE_31>` and `Gf2m8` at n ∈ {64, 256,
1024} (`crates/gf2-core/benches/inverse.rs:109-120`), i.e. a different field and
much larger n than the campaign's n ≤ 28. No committed receipt times `det` over
`Fp<3>`/`Fp<5>`/`Fp<7>` at campaign sizes, which matches the study's own
statement that the O(n³)-vs-Θ(n2ⁿ) ratio is "arithmetic on the complexity
expressions, not a measurement" (`feasibility-study.md:1207-1213`). The
determinant cell must be timed before the companion is assumed free, per
`@/inv/benchmark-backed-performance`.

**Layering note:** `det` is in `gf2-core`, i.e. *below* `gf2-algebra`, so a
determinant companion introduces no reverse dependency.

### C12 — issue 76dfd2ff (S3/S5 receipts non-authoritative) still open

**Valid.** `76dfd2ff` is `ready`, priority low, labels `type:task`,
`epic:permanent-statistics`, `component:gf2-algebra`, one gate (`doc-review`),
no dependencies. Receipts live in `dev/benchmarks/gf2_algebra_permanent/`:
`s3_cross_cpu-2026-05-12.csv` and `s5_gpu_crossover-2026-05-15.csv` (plus
snapshots under `csvs/`). That directory has **no README in the live tree** — the
provenance README that REQ-01 would amend is in the archive at
`dev/archive/ae82bd73-gf2-algebra-permanent/benchmarks/gf2_algebra_permanent/README.md`,
so REQ-01 has no live host document today and the task will have to create one.

**The live prose citing them as authoritative is `ROADMAP.md`:**

- `ROADMAP.md:86` — "**GPU batch ~28-30× CPU-SIMD** at M=256 (n=24: 28.65×,
  n=28: 30.32×) … source: `…/s5_gpu_crossover-2026-05-15.csv`". The feasibility
  study restates the same configuration against the best measured CPU path as
  **0.46× at n=24 and 0.44× at n=28** (`feasibility-study.md:648-659`). This is
  a live, unqualified headline contradicted by a newer measurement — squarely
  `@/inv/falsification-preserved` territory, and the single highest-value item in
  76dfd2ff.
- `ROADMAP.md:84-85` cite `s1_speedup-2026-05-11.csv` for the 10.6× figure; same
  provenance class, though not contradicted.

The epic's plan should sequence 76dfd2ff early (it is cheap, it is already
`ready`, and it clears a contradiction the campaign's own report would otherwise
have to explain).

### C13 — REQ-01 creditable to b488f02c via a `satisfies:REQ-01` label

**Invalid as stated. Two separate blockers.**

`b488f02c` labels are exactly `["type:task", "epic:permanent-statistics",
"component:gf2-algebra", "cites:GGK2025", "cites:Scheinerman2024"]` — **no
`satisfies:` label**, and (incidentally) no `cites:HKS2026` despite the study
resting on HKS Theorem 1.3.

Worse for coverage: **`b488f02c` is not a dependency of `b8206228` at all.** The
epic's only dependency is the breakdown node `f3dc1bb1`; `b488f02c` has no
dependencies and its sole reverse dependency is the planning node `912f1008`.

The `hard-criteria-covered` rule (`.jit/rules.toml:77-84`) asserts
`label-coverage` with `child-link = "dependencies"`, `child-state = "done"`,
`child-type-exclude = ["planning", "breakdown"]`, `satisfies-namespace =
"satisfies"`, firing `when = { state = "done", type = "epic" }`. The
`coverage-preview` rule (`:86-93`) is the same shape via
`container-from-label = "brackets"`, without the `child-state` filter, firing on
the breakdown node in `in_progress`/`gated`/`done`.

So for REQ-01 to be credited, **both** of these must hold and neither does today:
a dependency edge `b8206228 → b488f02c`, and a `satisfies:REQ-01` label on
`b488f02c`. `coverage-preview` runs on `f3dc1bb1` as soon as breakdown starts, so
this must be fixed *before* the breakdown node is worked, not at epic close.
Both changes touch a `done` issue's labels and the epic's dependency set; per
the standing no-autonomous-amendments rule they need explicit user approval
before anyone applies them.

## 2. Prior-art sweep

**`dev/studies/b488f02c/feasibility-study.md`** (1585 lines) is the design SSOT.
The parts the plan must carry verbatim rather than paraphrase: §4.6 envelope
table (`:761-777`) and the frontier sentence at `:779-782`; §7.2 sampling plan
including the Bonferroni rule (`:1403-1410`, K = 63, per-cell one-sided level
7.9e-4, critical z = 3.16, family-wise ≤ 4.9 %); §7.3 seeding scheme
(`:1434-1459`); §7.4 storage layout (`:1461-1490`); §7.5 analysis method
(`:1492-1502`); §7.6 delta against prior art (`:1504-1575`). The determinant
companion's acceptance test is two-sided at α/2K, critical z = **3.36**
(`:1249-1257`).

Five standing facts from the study that constrain the plan:

1. **Resample q=7, n=20 first** (`:964-966`) — the study's own recommendation,
   arising from the one by-product cell at z = −2.30.
2. **The q=3 arm is reproduction, not new ground** (`:1288-1297`); no wording may
   imply the campaign extends the q=3 curve in n (`:1519-1540`).
3. **No finite grid tests the conjecture** (`:1327-1333`); the measurable target
   is δ(n) = Pr − 1/q over the reachable range.
4. **Backend choice at q=3, n=28 is unresolved** (`:612-634`) — GPU vs
   intra-matrix rayon differ by 1.6 % against a 1.8 % re-measurement spread. "A
   campaign choosing that cell's backend should re-measure."
5. **DEC-01/DEC-02 fix the receipt set**; no regeneration.

**`dev/sessions/2026-08-08-b488f02c-review-rca.md`** — the RCA for the 11
research-review / 14 doc-review round loop. Its five root causes translate into
concrete plan obligations:

- RC1 (generated receipts carried interpretive prose, forcing regeneration
  cycles): the campaign driver's CSV/manifest emitters must emit **data and
  mechanical provenance only**. Every interpretive sentence belongs in the
  analysis report. Make this an explicit criterion on the driver issue, not an
  aspiration — it is the single largest churn source recorded.
- RC2 (`binary_sha256` self-hash with no build-state guard): embed the source
  SHA at build time and stamp/refuse on mismatch. This is cheap in a new
  binary and expensive to retrofit.
- RC4 (post-decision narrative residue swept one document per round): one
  grep-driven sweep per decision, all linked docs in scope, same commit.
- RC5 (hard-criterion literalism): audit each REQ's literal nouns against the
  artifact plan *before* dispatch.
- Open item: `jit validate` reports `contrib/gates/doc-review-prompt.md` as
  `stale` and the fix is blocked on a profile-package conflict. Expect that
  finding to persist through the epic; it is not caused by this work.

**`dev/sessions/2026-08-07-research-frontier-handoff.md`** — standing decisions
not to re-litigate (`:53-66`): citation registry and `cites:` labels;
research-review pinned to `gpt-5.6-sol` at xhigh with the other AI gates on
`gpt-5.6-terra` at high; 12 h campaign wall-clock budget; harness location. Its
`:48-51` already anticipates this breakdown and names G5 and G6 as the items to
file, with G6 "as a gf2-algebra bug". Its GPU note (`:105-113`) records that
M = 4096 faulted once, cause never established — avoid 4096 on cost-of-fault
grounds, not on a known threshold.

**`dev/studies/b488f02c/literature-search-2026-08-08.md`** (105 lines) is the
recorded search behind the q ∈ {5,7} novelty claim, including its limits (one
general web index, three unread candidate works). The plan's novelty wording must
stay conditional on that search (`feasibility-study.md:1571-1575`).

**`dev/campaigns/`** — 47 campaign definition TOMLs, all coding-domain. No
permanent-statistics precedent.

**Archive.** `dev/archive/ae82bd73-gf2-algebra-permanent/` holds the epic that
built the kernels: `plans/gf2_algebra_permanent.md` (design), `plans/a9e461de/`
(S5 GPU crossover writeup), `plans/363556e6/` (S3 cross-CPU), `plans/8e4e19a0/`
and `plans/.../r3_perm_uniformity_empirical.md` (the perm-vs-det uniformity
work — relevant to REQ-05 and explicitly distinguished from it at
`feasibility-study.md:1483-1490`). `dev/archive/6efb756b-grand/` holds the FEC
campaign-runner history including
`active/a4d86b3d-configurable-simulation-campaign-runner/a4d86b3d-campaign-runner-plan.md`,
the plan behind `sim_runner`.

## 3. Consumer sweep

### 3a. Callers of `permanent_bipedal3` (the G6 fix's blast radius)

Production code: **none**. No `crates/` source file outside `bipedal3.rs` calls
the dispatcher in a non-test path. The consumers are tests, benches, examples,
doc text, and research prototypes:

| Site | Kind |
|---|---|
| `crates/gf2-algebra/src/permanent/bipedal3.rs:145,150` | rustdoc doctest on the dispatcher |
| `crates/gf2-algebra/src/permanent/bipedal3.rs:661,674,691,709,727,742,793,813,875` | unit tests |
| `crates/gf2-algebra/src/permanent/parallel_bipedal3.rs:291,452,472,528` | parallel-vs-serial oracle |
| `crates/gf2-algebra/tests/gpu_dispatcher.rs:51,108,248` | GPU-vs-CPU equivalence |
| `crates/gf2-algebra/tests/permanent_vectors.rs:54,313,338,365,397,440` | CAS vector conformance |
| `crates/gf2-algebra/benches/s1_n36_speedup.rs:54,120,184` | S1 speedup bench |
| `crates/gf2-algebra/benches/permanent.rs:84` | Criterion suite |
| `crates/gf2-algebra/examples/permanent_demo.rs:152,254` | headline demo |
| `crates/gf2-algebra/src/gpu.rs:21,45,223` | rustdoc naming it the CPU equivalent |
| `crates/gf2-algebra/README.md:58` | crate README |
| `crates/gf2-algebra/src/lib.rs:41` | crate-level status prose |
| `crates/gf2-algebra/src/permanent/ryser.rs:10` | module prose |
| `crates/gf2-algebra/examples/s3_scalar_vs_avx2_sanity.rs:1,12-17` | the AVX2-slower sanity sweep |
| `dev/research/perm_uniformity/src/main.rs:147`, `tests/smoke.rs:41` | prototype |
| `dev/research/perm_uniformity_gpu/src/main.rs:619` | prototype |
| `dev/research/permanent_gpu_crossover/src/main.rs:85`, `tests/smoke.rs:94` | prototype |
| `dev/research/permanent_gpu_speedup/{tests/smoke.rs:36,simd_check.rs:41,det_check.rs:68}` | prototype |

Two consequences. First, the ~3× penalty is currently paid only by benches,
examples and prototypes — which means the *bench* numbers move when the fix
lands, and `crates/gf2-algebra/benches/s1_n36_speedup.rs` explicitly labels its
group "SIMD path" (`:99,109`). Fixing G6 makes that label wrong, and a stale
label inside a benchmark that feeds committed receipts is exactly the RC1 failure
mode. Second, the feasibility harness already bypasses the dispatcher
(`feasibility-study.md:371-374`; `dev/research/permanent-sampling-feas/src/backend.rs`
calls `permanent_bipedal3_singleword` and `..._simd` directly), so the campaign
does not depend on the fix — it is a repository-hygiene item that the campaign
surfaced.

`permanent_bipedal5` / `permanent_bipedal7` have the same consumer shape:
in-module tests and doctests, `tests/gpu_dispatcher.rs:152,279,315`,
`tests/cas_cross_validation.rs:144,196`, the `perm_uniformity` prototypes, and
the sampling harness (`backend.rs:239,242,261,265`;
`equivalence.rs:248,255,302,309`).

### 3b. Readers and writers of `dev/campaigns`

Only `sim_runner`, and only by argument: it takes the TOML path positionally
(`crates/gf2-coding/src/bin/sim_runner.rs:307`) and parses it at `:383`; the
directory is not hard-coded. One test asserts the canonical surface of that
directory (`crates/gf2-coding/tests/grand_phase1_smoke.rs:409-413`). Active prose
references are the runner's own usage docs (`sim_runner.rs:11,15,19`) and the
phase reports under `dev/simulation_results/`. Nothing writes into
`dev/campaigns`. A new subtree there would have no existing tooling to integrate
with — which weakens the "reuse the conventions" argument for that location.

### 3c. Plot and analysis scripts

- `scripts/plot_permanent_benchmarks.py` — the existing permanent-domain plot
  generator, and the natural home/precedent for the campaign's figures.
- `scripts/permanent-repro.sh`, `scripts/perm-uniformity-repro.sh`,
  `scripts/perm-uniformity-gpu-repro.sh` — reproduction wrappers for this exact
  problem family.
- `dev/benchmarks/dvb_t2_awgn/plot.py`, `finalize.py`, `run_campaign.sh`,
  `run_extend.sh`, `watch_progress.sh` — the campaign-local script set,
  co-located with the campaign's outputs rather than in `scripts/`.
- `dev/reference_data/scripts/{compare_results.py,parse_pgfplots.py}` —
  reference-data comparison.
- `dev/scripts/` is bench-lock and PPC tooling (`ccx1-bench-flock.sh`,
  `ppc-compare.sh`, `gen_fig7_report.sh`), not analysis.

So there are two live conventions: repo-wide reusable tooling in `scripts/`, and
campaign-local scripts beside the campaign's data. `dev/index.md:48` states the
`scripts/`-not-`benchmarks/` split is deliberate. The plan should pick one and
say why.

## 4. Primitive verification

### 4a. Harness tests, and the lift into a crate

Tests: `sampler.rs` 8, `prior.rs` 6, `stats.rs` 6, `equivalence.rs` 4;
`protocol.rs`, `backend.rs`, `env.rs`, `main.rs` 0. The two modules targeted for
productionising are the well-tested ones.

Dependencies are clean for a lift. `permanent-sampling-feas` depends on
`gf2-algebra` (features `simd,parallel,f5,f7`), `gf2-core`, `gf2-kernels-simd`,
`rayon 1.10`, `rand_chacha 0.9`, `rand_core 0.9`, `libc 0.2`
(`dev/research/permanent-sampling-feas/Cargo.toml:35-53`). `sampler.rs` itself
needs only `gf2_core::gfp::Fp`, `rand_chacha::ChaCha20Rng` and `rand_core`
(`sampler.rs:55-57`); `stats.rs` needs nothing outside `std` plus the crate's own
`prior` module.

**Version convergence is the real friction.** The workspace pins
`rand = "0.8"` (`Cargo.toml:27`) and does **not** declare `rand_chacha` as a
workspace dependency at all. In-tree today: `gf2-sim` pins `rand_chacha 0.9` +
`rand 0.9` (`crates/gf2-sim/Cargo.toml:18-19`); `gf2-kernels-hip` pins the same
pair as dev-dependencies (`crates/gf2-kernels-hip/Cargo.toml:33-34`);
`gf2-coding` pins optional `rand_chacha 0.3` (`crates/gf2-coding/Cargo.toml:23`);
`gf2-core` uses workspace `rand 0.8`. Three `rand_chacha` majors and two `rand`
majors coexist. Adding ChaCha20 to a production crate should promote
`rand_chacha` to `[workspace.dependencies]` at 0.9 in the same change, per
`@/inv/convention-convergence` ("changes it at its source"). Licensing is a
non-issue: `rand_chacha` is MIT/Apache-2.0, matching the workspace's MIT
(`crates/gf2-algebra/Cargo.toml`, `license = "MIT"`).

The reproducibility argument for ChaCha20 over `StdRng` should be stated
explicitly in the plan: `StdRng`'s algorithm is not stable across `rand`
releases, so a published dataset seeded through it is not regenerable after a
dependency bump, whereas `ChaCha20Rng::from_seed` is.

### 4b. Checkpoint/restart machinery worth reusing

`crates/gf2-sim/src/checkpoint/mod.rs` is a mature, tested implementation. What
is reusable is the **mechanism**, not the type:

- `CheckpointWriter::write` (`:346`) / `write_with_fsync_hook` (`:377`) — atomic
  write via PID-tagged temp file, fsync, rename, directory fsync; crash-safe
  under SIGINT during the write (`:399`).
- `config_hash(&PipelineConfig) -> String` (`:248`) — blake3 over the serialised
  config excluding path-dependent fields; a resume against a different config is
  a hard error.
- `CheckpointReader::load` (`:478`) — v2-only, rejects wrong schema version,
  hash mismatch, or unparseable file rather than silently accepting; missing
  file means fresh work (`:482`).
- Per-point files `snr_<NNNN>.json`, pretty-printed JSON, `u128` RNG positions
  serialised as decimal strings (`:101-102`, `:178-190`).
- Deterministic resume by storing each worker's absolute ChaCha20 32-bit word
  position (`WorkerState::rng_word_pos`, `:97-102`) — directly analogous to what
  a `(root, q, n, stream)` shard scheme needs, and the reason the campaign's
  shard tuples make restart easier here than in the FEC case.
- Tests: resume-vs-uninterrupted counter equality
  (`crates/gf2-sim/tests/determinism.rs:476`) and a SIGINT/resume byte-identity
  subprocess test on the campaign binary
  (`crates/gf2-sim/tests/campaign_cli_flags.rs:356-357`, `#[ignore = "sim: …"]`).

The **schema** is unreusable: `CheckpointV2` fields are `esn0_db`,
`frames_target`, `errors_target`, `errors_accumulated`, `total_iterations`,
`total_queries`, `total_bits`, `total_bit_errors` (`:139-176`), and
`CheckpointWriter::write` is typed to `&CheckpointV2` — it is not generic over a
payload. So reuse means one of: generalise the writer/reader over a serialisable
payload in `gf2-sim` (change at source), or accept a second implementation with a
named, tracked exception. See §6.

### 4c. HIP batch entry points return per-matrix results

**Confirmed.** `permanent_batch_bipedal3(&[Bipedal3Matrix]) -> Vec<Fp<3>>`
(`crates/gf2-algebra/src/gpu.rs:264`),
`permanent_batch_bipedal5(&[Packed5Matrix]) -> Vec<Fp<5>>` (`:361`),
`permanent_batch_bipedal7(&[Packed7Matrix]) -> Vec<Fp<7>>` (`:467`). All three
return one value per input matrix in input order; no device-side aggregation.
The study's §4.1 equivalence run independently confirms per-matrix agreement
(`feasibility-study.md:305-327`). Exact zero counting is therefore available on
the GPU path.

### 4d. Where a production sampler / statistics / driver could live

Workspace layout for reference: five members — `gf2-core`, `gf2-coding`,
`gf2-algebra`, `gf2-kernels-simd`, `gf2-sim` (`Cargo.toml:2-8`); `gf2-kernels-hip`
and two research crates excluded (`:11-20`). Layer contract in `AGENTS.md:38-58`:
`gf2-core` owns primitives and has no dependency on another gf2 crate;
`gf2-algebra` owns packed F_3/F_5/F_7 arithmetic and permanent algorithms;
`gf2-sim` owns simulation pipeline and orchestration; dependencies point inward
(`@/inv/crate-dependency-direction`).

Six real options, with precedents:

1. **Core extension — sampler into `gf2-core`, statistics beside it.** Precedent:
   `FieldMatrix::random`/`random_seeded` already live there
   (`crates/gf2-core/src/field/matrix.rs:596,617`) and `gf2_core::rng` is the
   declared workspace RNG SSOT (`crates/gf2-algebra/src/testutil.rs:4-6` calls it
   "the workspace SSOT RNG"). Fits `@/inv/convention-convergence` best: one
   random-field-element mechanism, one home. Cost: adds `rand_chacha` to the
   lowest layer; statistics (Wilson/Clopper–Pearson) have no natural home in a
   finite-field crate.
2. **Module in `gf2-algebra`** — e.g. `gf2_algebra::sampling` producing packed
   matrices directly. Precedent: `testutil.rs` is already the crate's matrix
   generator and `packed::*::from_row_major` is the constructor the harness uses
   (`dev/research/permanent-sampling-feas/src/protocol.rs:267`). Cheapest
   integration with the kernels; but a statistics/campaign layer inside an
   algebra crate strains `AGENTS.md:44-46` ("may dispatch to isolated kernels
   without becoming a kernel layer itself" — by the same logic, not an
   orchestration layer either).
3. **New narrow crate — `gf2-stats`** (or `gf2-sampling`): sampler + streaming
   accumulator + intervals, depending only on `gf2-core`. Precedent for a small
   focused member: `gf2-kernels-simd`. Keeps the campaign driver out of the
   algebra layer and gives the Clopper–Pearson/Wilson pair a real home. Cost: a
   sixth workspace member and its full gate surface.
4. **New broad crate — `gf2-campaign`**: sampler, statistics, dataset format and
   driver binary, depending on `gf2-core`, `gf2-algebra`, optionally
   `gf2-kernels-hip`. Precedent: `gf2-sim` is exactly this shape for the coding
   domain (orchestration + `src/bin/` campaign binaries + `checkpoint/`). Best
   fit for the campaign as a whole; largest new surface, and duplicates
   `gf2-sim`'s checkpoint mechanism unless that is generalised first.
5. **Binary inside `gf2-sim`** — `crates/gf2-sim/src/bin/permanent_zero_fraction.rs`
   beside `dvb_t2_awgn_campaign.rs`. Precedent is exact: `gf2-sim` already hosts
   `dvb_t2_awgn_campaign`, `ldpc_bler_sweep`, `checkpoint_sweep`, and owns the
   checkpoint module the driver wants. Requires a **new** dependency edge:
   `gf2-sim` today depends on `gf2-core`, `gf2-coding`, and optional
   `gf2-kernels-hip` only (`crates/gf2-sim/Cargo.toml:14-16`) — not on
   `gf2-algebra`. That edge is legal under `@/inv/crate-dependency-direction`
   (`gf2-sim` is orchestration, `gf2-algebra` is mathematical, so it points
   inward), but it widens the crate's stated identity: "Research-grade CPU+GPU
   FEC simulation pipeline … built on gf2-coding"
   (`crates/gf2-sim/Cargo.toml:6`). Cheapest path to reusing checkpointing;
   couples a number-theory campaign to the FEC simulation crate.
6. **Research prototype — `dev/research/permanent-campaign/`.** Precedent: the
   feasibility harness itself, plus `perm_uniformity`, `permanent_gpu_crossover`,
   `permanent_gpu_speedup`. Fastest, no workspace gate cost, and `dev/research` is
   a permanent path. But REQ-02/REQ-03 speak of a harness and a published dataset
   with recorded versions; a prototype outside the workspace gets no `cargo-ci`
   coverage, which sits badly with `@/inv/shared-test-contracts`. Reasonable only
   as a staging step with a stated convergence condition. Note the standing rule:
   any new `dev/research/<crate>/` stub needs a `.gitignore` with `target/` and
   `Cargo.lock` before its first commit (both existing prototypes have one:
   `dev/research/permanent-sampling-feas/.gitignore`).

A defensible split, offered as a recommendation rather than a decision: (1) or
(3) for the sampler and statistics, plus (5) for the driver so the checkpoint
mechanism is reused rather than re-implemented.

## 5. Architecture fit

**Reusable primitives.** Wilson interval and sample-size planning
(`dev/research/permanent-sampling-feas/src/stats.rs:11,27,38`); the seed
derivation and rejection sampler (`sampler.rs:87,97,184`); the four-phase
composite loop (`protocol.rs:315`); per-matrix GPU batch results
(`crates/gf2-algebra/src/gpu.rs:264,361,467`); the F_q determinant
(`crates/gf2-core/src/field/inverse.rs:494`); the atomic checkpoint writer and
config-hash gate (`crates/gf2-sim/src/checkpoint/mod.rs:346,248,478`); the
generic Ryser cross-check (`crates/gf2-algebra/src/permanent/ryser.rs:89`).

**Layer boundaries.** Sampling and statistics are primitives (core or a narrow
new crate). Permanent evaluation stays in `gf2-algebra`. Campaign orchestration,
checkpointing and dataset emission are orchestration-layer concerns — the layer
`gf2-sim` already occupies. The GPU path reaches the campaign only through
`gf2_algebra::gpu`, so the driver never touches `gf2-kernels-hip` directly, and
`@/inv/unsafe-kernel-isolation` is unaffected.

**Published-artifact structure, as actually practised.** The DVB-T2 campaign
(issue `152388f4`, done) is the best model:

```
dev/benchmarks/dvb_t2_awgn/
  run_campaign.sh, run_extend.sh, watch_progress.sh   # drivers
  plot.py, finalize.py                                # analysis
  curve_<modcod>.csv, curve_<modcod>.png              # published curve + figure
  curve_<modcod>/
    curve_<modcod>.csv        # per-curve copy
    README.md                 # invocation, config, host, wall-clock, plotting
    checkpoints/snr_NNNN.json # resume state
    tracing.jsonl             # structured progress log
```

Generated by `crates/gf2-sim/src/bin/dvb_t2_awgn_campaign.rs` (layout documented
at `:154`, README writer at `:692`, host capture at `:661`, CSV header
`es_n0_db,fer,ber,frames,errors,mean_iters,wall_seconds` at `:612`, hand-written
`writeln!` rather than the `csv` crate at `:618`). `dev/simulation_results/`
holds the other convention: flat `<name>.csv` + `<name>.json` +
`<name>.progress.jsonl` triples plus comparison reports.

**Neither convention records git revision, toolchain, or checksums**, and neither
has a manifest. The campaign's manifest is therefore new. Two things follow: the
plan should not describe it as "reusing existing conventions" (a reviewer will
check), and the fields `@/inv/claims-trace-to-artifacts` requires — seeds, git
revision, hardware, toolchain — must be enumerated in the criterion text rather
than left to the implementer. The feasibility harness's own CSV preambles
already carry most of them (`harness_source_sha`, `deps_source_sha`,
`binary_sha256`, rustc/ROCm versions, CPU/GPU model, governor, thread count) and
are the better template — with RC1's caveat that they must carry no interpretive
prose.

## 6. Architectural-invariant check

**`@/inv/convention-convergence`** (verified wording via `jit item show`): "A
shared convention or abstraction has one form: work that finds it harmful or
ill-fitting changes it at its source, or reports the mismatch as a blocking
concern before proceeding. A local parallel variant, a private helper duplicating
a shared mechanism, or a bypass around an abstraction is a defect unless it is a
named, cited exception with a tracked convergence condition."

The study's G3 argument (`feasibility-study.md:1068-1078`) reads: the invariant
"is aimed at duplicating a mechanism; the two campaigns share no axis and no
inner loop, so a separate driver is a distinct abstraction rather than a parallel
copy". **That reading holds for the campaign schema and fails for the checkpoint
writer.** The invariant's own text names "a private helper duplicating a shared
mechanism" as a defect, and `CheckpointWriter` (atomic temp+fsync+rename+dir
fsync, blake3 config-hash gate, v2-only rejecting reader) is a shared mechanism
with one form today. `(q, n, shard)` versus `(code, modem, SNR)` is a genuinely
different schema; "write a checkpoint file crash-safely and refuse to resume
against a different config" is not a different mechanism.

Three admissible resolutions, in the invariant's own terms — the plan must pick
one explicitly:

- **Change at source.** Generalise `CheckpointWriter`/`CheckpointReader` over a
  serialisable payload + config-hash provider in `gf2-sim`, and have both
  campaigns use it. Highest cost, cleanest under the invariant, and it touches a
  module with existing byte-identity resume tests that must keep passing
  (`crates/gf2-sim/tests/determinism.rs:476`).
- **Reuse in place.** Host the driver in `gf2-sim` (option 5 above) and use the
  existing writer with a permanent-statistics payload — which still requires
  generalising the payload type, but keeps the change local to one crate.
- **Named exception.** A second implementation with a cited exception and a
  tracked convergence condition. Permitted by the invariant's final clause, but
  it must be *named and tracked*, not merely argued in a study section.

Whichever is chosen, the study's G3 sentence should be narrowed to the campaign
*schema*, because as written it is broader than what the invariant licenses and a
research-review round will say so.

**`@/inv/falsification-preserved`** — "Data that contradicts a criterion,
hypothesis, or cited claim is recorded together with the contradiction; silent
rework of the falsified statement is a defect." Live obligations: (a) the
`ROADMAP.md:86` 28–30× GPU claim contradicted by `feasibility-study.md:648-659`
(0.46×/0.44×) must be recorded with the contradiction, not just edited — this is
76dfd2ff's REQ-02; (b) any campaign cell whose interval excludes
[Scheinerman2024]'s point estimate is recorded and investigated, never reconciled
away (`feasibility-study.md:1554-1557`); (c) the q=7, n=20 by-product cell at
z = −2.30 stays on the record when the campaign resamples it, whatever the
resample returns.

**`@/inv/uncertainty-reported`** — every Monte Carlo estimate states sample count
and CI; plots and tables carry error bars or interval columns. Binds every REQ-03
artifact and the analysis report's figures.

**`@/inv/claims-trace-to-artifacts`** — every published number traces to a
committed artifact recording seeds, git revision, hardware, toolchain. This is
what makes the manifest a hard requirement rather than a nicety, and what the
DVB-T2 README convention does not currently satisfy.

**`@/inv/deterministic-seeded-execution`** — "A fixed seed and configuration
produce identical observable results across supported worker counts, scheduling
paths, checkpoint/resume boundaries, and accelerator fallbacks." The campaign
must carry an explicit test that a shard redrawn from its `(root, q, n, stream)`
tuple reproduces bit-identically across thread counts and across a
checkpoint/resume boundary, and across CPU and GPU backends. The harness's
history is the argument for making this a criterion rather than an assumption:
two of its retracted anomalies were stream-reuse and warm-up-shared-counter
defects (`feasibility-study.md:978-995`).

**`@/inv/backend-behavioral-equivalence`** and **`@/inv/shared-test-contracts`** —
the campaign's backends (scalar, AVX2, batch rayon, intra rayon, generic Ryser,
GPU) must run one shared behavioural suite. §4.1 of the study did this once for
the feasibility harness; the production driver needs it as a standing test, not a
one-off run.

**`@/inv/benchmark-backed-performance`** — the unresolved q=3, n=28 ordering
(`feasibility-study.md:612-634`) and the untimed determinant companion
(`:1207-1213`) are both open measurement obligations. Backend selection per
`(q, n)` must be re-measured at campaign time and receipted, not inherited from
the study's table.

**`@/inv/single-source-prose`** — the G6 fix invalidates
`crates/gf2-algebra/src/permanent/bipedal3.rs:26` and `:349`, the S3 example
header, and the bench group label in `benches/s1_n36_speedup.rs:99,109`. Sweep
them in the same change.

**Preregistration and acceptance-test obligations the plan must carry.** These
are contractual, from §7.2, and should appear as criteria rather than as design
notes:

- Fixed N per cell, chosen from the envelope **before** any campaign data is
  drawn; no stopping rule keyed on the observed statistic
  (`feasibility-study.md:1374-1378`).
- Grid: all n from 4 to the per-q frontier — 25 cells at q=3, 21 at q=5, 17 at
  q=7, **K = 63** (`:1379-1382`, `:1398-1401`).
- Standing per-cell test Pr[per = 0] ≥ 1/q at the Bonferroni one-sided level
  α/K = 7.9e-4, critical **z = 3.16**; a failing cell halts the campaign for
  pipeline investigation and is never reported as a finding (`:1403-1410`).
- Determinant companion checked two-sided against the exact finite-n value
  1 − ∏(1 − q^-i) at α/2K, critical **z = 3.36** — never against the n→∞ limit
  (`:1422-1427`, `:1249-1257`).
- K is fixed by pre-registration; adding cells later requires restating the
  adjustment rather than reusing it (`:1412-1414`).
- Exact anchors before any estimated cell is believed: q=3 to n=4, q=5 and q=7
  to n=3 (`:1383-1388`).
- Resample q=7, n=20 first (`:964-966`).

## 7. Open questions the plan must decide (not answerable from code)

1. Dataset location: `dev/campaigns/` (permanent, but declared for *definitions*)
   versus `dev/simulation_results/` (permanent, declared for *outputs*). §1/C4.
2. Checkpoint mechanism: generalise in `gf2-sim`, host the driver in `gf2-sim`,
   or take a named exception. §6.
3. Crate placement for sampler/statistics/driver. §4d.
4. G6 fix shape: flip the preference, or build the 4-matrix batched path the
   rustdoc already promises. §1/C6.
5. Whether `b8206228 → b488f02c` dependency + `satisfies:REQ-01` label are added
   (needs user approval; blocks `coverage-preview` on `f3dc1bb1`). §1/C13.
6. Where the exact-enumeration anchors live once `dev/studies/b488f02c/` is
   archived. §1/C10.

## 8. Claim classification summary

| Claim | Verdict |
|---|---|
| C1 sampler prototyped; no production sampler | valid-and-open, **with correction** — `FieldMatrix::random` exists; the missing thing is stream addressing + version-stable RNG |
| C2 Wilson only; no checkpointing; no Clopper–Pearson | valid-and-open |
| C3 no permanent campaign runner | valid-and-open; **crate misattributed** (`gf2-coding`, not `gf2-sim`), and the DVB-T2 binary is the closer precedent |
| C4 no dataset format; `dev/campaigns` permanent | valid-and-open; **location conflicts** with `dev/index.md:42` |
| C5 no rank predicate; no new kernel | valid-and-open |
| C6 dispatcher prefers slower AVX2 | valid-and-open, **still present** (`bipedal3.rs:190-193`) |
| C7 no batch-parallel path | valid-and-open; **citation path wrong** (`src/parallel.rs:5-7`) |
| C8 `Packed7::LANES = 16`; Ryser only F_7 path > 16 | valid-and-open **only with the "CPU" qualifier** |
| C9 composite hot path liftable | valid-and-open; **`protocol.rs` has no tests** |
| C10 anchors exist | already-done, but **issue-scoped location** is a live problem |
| C11 determinant path | already-done (`inverse.rs:494`); **unmeasured at campaign sizes** |
| C12 76dfd2ff open; receipts cited as authoritative | valid-and-open; live citer is `ROADMAP.md:84-86`, **no live README** to amend |
| C13 REQ-01 creditable via label | **invalid-as-stated** — no `satisfies:` label *and* no dependency edge |
