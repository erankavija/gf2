# Investigation: 0de41c82 — wave-parallel GPU kernels for finite-field permanents

Read-only verification of the current system, performed 2026-08-09 against the
working tree at `main` (`f5c01332`). Every finding below cites `file:line` in
the tree as read. Nothing was edited, committed, or transitioned.

Scope: classify the planning brief's input claims, sweep prior art and
consumers, verify the primitives the breakdown will assert, and record the
architectural constraints the breakdown must preserve.

---

## 1. Claim classification

### Claim 1 — one matrix per one-thread block, sequential Gray walk

**Valid as stated.** All three HIP permanent kernels launch
`grid(m, 1, 1)`, `block(1, 1, 1)` and return immediately from every thread but
thread 0:

- F_3: `permanent_bipedal3.hip:174` (`if (threadIdx.x != 0) return;`), launch
  config at `permanent_bipedal3.hip:332-343`.
- F_5: `permanent_bipedal5.hip:99`, launch config at
  `permanent_bipedal5.hip:251-262`.
- F_7: `permanent_bipedal7.hip:99`, launch config at
  `permanent_bipedal7.hip:348-359`.

The design is stated as intentional in each file header (e.g.
`permanent_bipedal3.hip:5-8`: "only thread 0 of each block executes the Gray
walk (sequential by design) … GPU parallelism comes from having many matrices
in flight simultaneously"), and repeated at the batch entry point
(`permanent_bipedal3.hip:346-347`: "all other threads return immediately
(idle)").

Note on wording: with `blockDim.x = 1` the hardware allocates a wavefront per
block and only lane 0 is active, so the correct statement is that 31 of 32
RDNA2 lanes are inactive. "Most wavefront lanes idle" is therefore right, but
the mechanism is a one-thread block, not a partially filled wave.

### Claim 2 — F_3 keeps a bit-sliced word pair; F_5/F_7 keep byte arrays

**Valid as stated.**

- F_3 keeps `uint64_t sum_m, sum_s` (`permanent_bipedal3.hip:247-248`), updated
  by six-to-eight bitwise ops per Gray step (`permanent_bipedal3.hip:272-282`,
  helpers at `:47-82`), with an O(1) six-stage halving fold
  (`permanent_bipedal3.hip:102-133`, fold steps at `:112-123`).
- F_5 keeps `uint8_t columns[63][63]` and `uint8_t col_sum[63]`
  (`permanent_bipedal5.hip:132`, `:143`) with an explicit `for (i = 0; i < n)`
  update (`permanent_bipedal5.hip:183-191`) and an O(n) product loop with a
  zero short-circuit (`permanent_bipedal5.hip:197-202`).
- F_7 keeps the same byte arrays (`permanent_bipedal7.hip:132`, `:148`) with
  LUT-driven O(n) update (`permanent_bipedal7.hip:190-202`) and an O(n) LUT
  product loop (`permanent_bipedal7.hip:212-218`). The three LUTs are 64 KiB
  each: `d_MUL_LUT` in `__constant__` (`permanent_bipedal7.hip:74`),
  `d_ADD_LUT`/`d_SUB_LUT` in `__device__` global (`:77`, `:80`).

All three file paths in the claim are correct.

### Claim 3 — per-call allocate / H2D / launch / sync / D2H / free, no persistence, no streams

**Valid as stated.** `permanent_gf3_batch_dispatch` at
`crates/gf2-kernels-hip/src/permanent/mod.rs:936` allocates two
`DecoderDeviceBuffer`s (`:953`, `:955`), copies H2D (`:958`), launches
(`:965`), calls `hip_device_synchronize` (`:979`), copies D2H (`:991`), and
drops the buffers at end of scope. `permanent_gf5_batch_dispatch` (`:1034`)
and `permanent_gf7_batch_dispatch` (`:1138`) follow the same shape (syncs at
`:1075`, `:1182`). The contract is stated in the module comment at
`mod.rs:882-891`. No stream is ever created; every launch uses stream 0
(`permanent_bipedal3.hip:339`, `permanent_bipedal5.hip:258`,
`permanent_bipedal7.hip:355`).

The `gf2_algebra::gpu` line numbers in the claim are correct:
`permanent_batch_bipedal3` at `gpu.rs:264`, `permanent_batch_bipedal5` at
`gpu.rs:361`, `permanent_batch_bipedal7` at `gpu.rs:467`. Each serialises the
whole batch on the host into a flat `Vec<u8>` first (`gpu.rs:295`,
`serialise_bipedal3` at `gpu.rs:140`, `serialise_packed5` at `:160`,
`serialise_packed7` at `:180`) — an `O(M · n^2)` per-call host pass, lane by
lane.

**Material addition the claim omits.** The persistent-buffer and stream
infrastructure the study would need for a production design *already exists*
and is public and unconditionally compiled: `crates/gf2-kernels-hip/src/host/`
exports `DeviceBuffer<T>`, `PinnedHostBuffer<T>`, `HipStream`,
`HipStreamPool`, and `LaunchDims` (`host/mod.rs:28-31`; `pub mod host;` at
`lib.rs:33`, outside the `hip` feature gate). `DeviceBuffer` has async
pinned-copy methods (`host/alloc.rs:535`, `:593`) and `HipStreamPool` supports
fixed-index, round-robin and oldest-idle acquisition (`host/streams.rs:335`,
`:373`, `:409`). The permanent path bypasses all of it in favour of
`DecoderDeviceBuffer`, which is `pub(crate)` (`lib.rs:279`) and documented as
"a byte-oriented adapter over the canonical `host::DeviceBuffer<u8>`"
(`mod.rs:873-875`). A go decision's production design should converge on the
canonical `host` API rather than extend the adapter — see §6.

### Claim 4 — `Packed7::LANES = 16` limits F_7 to n≤16; `permanent_bipedal5` "scalar-only" asserts n≤63

**Split verdict.**

*Valid:* `pub const LANES: usize = 16;` is a module-level const at
`crates/gf2-algebra/src/packed/packed7.rs:216` (the trait associated const
mirrors it at `:593`). `permanent_bipedal7` (`permanent/bipedal7.rs:110`)
asserts `n <= LANES` at `:119-122`; the inner
`permanent_bipedal7_singleword` (`:163`) repeats the assert at `:172-175`.
The n≤63 assert on `permanent_bipedal5` is real, at
`permanent/bipedal5.rs:117-123` (function signature at `:108`), with the same
assert repeated in `permanent_bipedal5_singleword` at `:182-187`.

*Invalid as stated:* **"scalar-only" is wrong if read as "scalar arithmetic".**
`permanent_bipedal5` runs a packed three-plane path: one O(1)
`Packed5::add`/`sub` per Gray step (`bipedal5.rs:228`, `:232`) followed by
`Packed5::fold_mul_first_n` (`:236`). The phrase comes from
`dev/studies/b488f02c/feasibility-study.md:82-85`, where "scalar only" means
*no SIMD or rayon companion exists for that field* — which is true (see claim
10). The distinction is load-bearing for REQ-08/REQ-09: the F_5 baseline the
study must beat is already a packed bit-plane accumulator, not byte
arithmetic, on the CPU side; only the **GPU** F_5 path is byte arithmetic.

*Also worth recording:* the horizontal folds on **both** fields are serial
per-lane scalar multiplies, not packed multiplies and not LUT lookups:

- `Packed5::fold_mul_first_n` (`packed5.rs:1333`) decodes lane by lane and
  multiplies `Fp<5>` values in a loop (`:1345-1349`), with an explicit comment
  at `:1338-1341` that no bit-sliced halving fold exists for F_5.
- `Packed7::fold_mul_first_n` (`packed7.rs:469`) extracts each nibble and
  multiplies `Fp<7>` values in a loop (`:474-480`).

The rustdoc on the F_7 one claims "`O(n)` LUT lookups" (`packed7.rs:468`),
which the body contradicts — it performs `Fp<7>` multiplications. That is a
stale doc comment and a candidate cleanup, not a correctness bug.

### Claim 5 — `Packed5` is a three-plane representation with a Boolean add/sub circuit

**Valid as stated.** `pub struct Packed5 { b0: u64, b1: u64, b2: u64 }` at
`packed5.rs:208-212`, canonical 3-bit encoding table at `packed5.rs:9-20`,
64 lanes per triple (`packed5.rs:3-5`; `LANES = 64` at `packed5.rs:419`).
Add/sub/mul/neg are pure Boolean circuits over the planes: 5-way decode
(`decode5` at `packed5.rs:70`), 5×5 cross-product, encode
(`packed5.rs:24-38`).

**Cost the plan must carry.** The module docstring states 60 bitwise ops per
`u64`-triple for add/sub and 52 for mul (`packed5.rs:33`). That is the price
of "the production `Packed5` Boolean add/subtract circuit" the representation
study proposes as the F_5 candidate 2 — roughly twice the 31 ops of the F_7
Mersenne-fold add (`dev/research/f7_packing/src/cand_d.rs:27-28`), because
`5 = 2^2 + 1` is not Mersenne. The study's exploratory candidate 3 (ripple-add
plus conditional mod-5 reduction) exists precisely to attack that number.

### Claim 6 — `dev/research/f7_packing/src/cand_d.rs` exists, three-plane Mersenne-add F_7

**Valid, with one correction to the description.** The file exists at 417 lines
and implements the three-plane Mersenne-fold candidate: layout at
`cand_d.rs:3-9`, the `7 = 2^3 − 1` argument at `:11-21`, op counts at `:25-31`,
`struct VecD { b0, b1, b2, len }` at `:39-44`, `decode7` at `:59`.

**It is a candidate, not the oracle.** The correctness oracle in that crate is
`dev/research/f7_packing/src/common.rs` (`ref_mul`, `ref_div`, `F7Encoding`),
which `cand_d.rs` imports at `:33`. The plan should say "Candidate D's
implementation is the starting point; `common.rs` supplies the reference" —
`cand_d.rs` alone cannot validate itself.

### Claim 7 — the measurement harness exists with the named modules and receipts

**Valid as stated, in full.** `dev/research/permanent-sampling-feas/` contains
`backend.rs` (321), `env.rs` (297), `equivalence.rs` (375), `lib.rs` (29),
`main.rs` (790), `prior.rs` (325), `protocol.rs` (885), `sampler.rs` (334),
`stats.rs` (233), all git-tracked.

- ChaCha20 rejection sampling: `sampler.rs`, motivated at
  `feasibility-study.md:1039-1049` (256 mod 7 = 4 ⇒ 1.56 % worst-class relative
  bias against a 0.07 % target).
- Preregistered protocol: `protocol.rs:89` `WARMUP_SECONDS = 3.0`, `:91`
  `MIN_REPS = 5`, `:93` `MIN_TIMED_SECONDS = 5.0`, `:95`
  `MAX_CELL_SECONDS = 120.0`, `:97` `TARGET_REP_SECONDS = 2.0`, `:99`
  `MAX_BATCH = 65_536`. Warm-up / repetition / censoring policy documented at
  `protocol.rs:20-33`; the normative censoring contract at `protocol.rs:34-50`.
- Provenance preamble: `env.rs:47` `struct HostInfo` with `binary_sha256`
  (`:73`), `cpu_model` (`:76`), `gpu_model` (`:82`), `rocm_version` (`:83`);
  emitted by `csv_preamble` (`env.rs:130`). A live example is the 24-line
  preamble of `dev/studies/b488f02c/throughput-2026-08-07.csv`.
- Committed receipts under `dev/studies/b488f02c/`: `throughput-2026-08-07.csv`,
  `sustained-2026-08-07.csv`, `envelope-2026-08-07.csv`,
  `zero-fraction-2026-08-07.csv`, `equivalence-2026-08-07.csv`,
  `gpu-hang-2026-08-07.log`, `cargo-tree-2026-08-07.txt`, plus the two anchor
  scripts and outputs — all git-tracked.

The harness CLI has five subcommands (`main.rs:51-55`): `equivalence`, `grid`,
`sustained`, `envelope`, `zerofrac`, documented in `src/usage.txt`. The GPU
backend degrades to "unsupported" without `--features hip`
(`backend.rs:169-171`; `Cargo.toml` feature `hip = ["gf2-algebra/hip"]`).

### Claim 8 — gfx1030 / RX 6950 XT 80 CUs / ROCm 7.2 / wave32; what the build actually compiles for

**Host facts valid; the "supported architecture scope" is narrower than the
target list suggests.**

- GPU model and CU count confirmed: `feasibility-study.md:294-295` (RX 6950 XT,
  gfx1030, 80 compute units); the CSV preamble records the device string and
  `# rocm: 7.2.4` (`throughput-2026-08-07.csv:16-17`), and
  `dev/research/permanent_gpu_speedup/src/main.rs:18` and `:65` independently
  state 80 CUs.
- Wavefront 32 on RDNA2 is stated in-tree at
  `crates/gf2-kernels-hip/hip/bcjr_kernel.hip:26` ("wavefront=32 on RDNA2,
  wavefront=64 on CDNA").
- **The permanent kernels compile for gfx1030 only.** `build.rs:43` applies a
  single unconditional `--offload-arch=gfx1030` to the static library that
  contains all three `.hip` permanent sources (added under the `hip` feature at
  `build.rs:36-39`). The six-entry `GFX_TARGETS` list (`build.rs:9-11`:
  gfx1030, gfx1100, gfx1200, gfx90a, gfx940, gfx942) drives a *separate*
  per-arch `.co` pipeline (`compile_arch_blobs`, `build.rs:113`) whose only
  sources today are auto-generated no-op probes (`ensure_probe_source`,
  `build.rs:192`, body at `:204-208`). `kernels/` is entirely gitignored
  (`crates/gf2-kernels-hip/.gitignore`), and `git ls-files
  crates/gf2-kernels-hip/kernels` returns nothing.

**Consequence for REQ-04.** "States the supported GPU architecture scope and
records compile evidence for every architecture claimed to be supported" today
has exactly one defensible answer — gfx1030 — unless the study adds
`--offload-arch` flags. The `GfxTarget` enum (`host/arch.rs:67`) and the
`GF2_HIP_COMPILED_ARCHS` build manifest (`build.rs:70-73`, consumed at
`host/arch.rs:34`) already provide the mechanism to record what compiled; the
permanent kernels are simply not wired into it.

### Claim 9 — `permanent_ryser<F: FiniteField>` at `ryser.rs:89`, n≤63, CPU oracle

**Valid as stated.** `pub fn permanent_ryser<F: FiniteField>(matrix: &[F], n: usize) -> F`
is at `crates/gf2-algebra/src/permanent/ryser.rs:89`, asserting `n <= 63`
(`:90-95`) and `matrix.len() == n * n` (`:96-103`). It is field-agnostic and so
applies to all three fields at every n the study measures, including where the
packed F_7 kernel does not exist. Its use as the oracle above the F_7 lane
limit is already exercised: `equivalence.rs:107-126` falls back to it when the
scalar reference is unsupported, and `crates/gf2-algebra/tests/gpu_dispatcher.rs:36-40`
documents it as the n=24 F_7 reference.

Its own rustdoc calls it a driver for `n ≤ 16` cross-checks
(`ryser.rs:87-88`); `feasibility-study.md:92-94` records that this is a
statement of intent rather than a bound, and that §4.1 confirms it agrees with
every packed kernel at every comparable cell.

### Claim 10 — intra-matrix rayon for F_3 only; `parallel.rs:5-7` says F_5/F_7 remain a follow-up

**Valid as stated, and it is the single most reusable piece of prior art in the
tree for REQ-01.** `permanent_bipedal3_parallel` is at
`permanent/parallel_bipedal3.rs:102`, delegating to
`permanent_bipedal3_parallel_with_chunk` (`:148`).
`crates/gf2-algebra/src/parallel.rs:5-7` reads verbatim: "F_5 / F_7 single-word
analogues (`permanent_bipedal5` / `permanent_bipedal7`) landed in W4-T18/T20;
parallel companions for F_5 / F_7 remain a follow-up."

The chunked design already implements every structural element REQ-01 demands
of the GPU decomposition, on the CPU:

- Gray-range partition: `CHUNK_SUBSETS = 1 << 16` (`parallel_bipedal3.rs:49`),
  chunk starts at `:199-201`.
- Gray-range initialization from a range start:
  `process_chunk` (`:237`) reconstructs `col_sum` from
  `gray_code_index_to_subset(start)` (`:240-247`), using
  `crates/gf2-algebra/src/gray.rs:48`.
- Order-independent partial-sum reduction: `.reduce()` over F_3 addition
  (`:210`), with the determinism argument at `:196-198`.
- Outer `(-1)^n` factor applied once after the reduce (`:212-217`).

A wave-cooperative GPU kernel is the same decomposition with lanes instead of
rayon workers. The plan should require the prototype to reuse
`gray_code_index_to_subset`'s semantics rather than re-derive them, and should
cite this as the CPU cross-check for the GPU Gray-range initialization.

### Claim 11 — existing Lean verification of bipedal F_3; obligations for new representations

**Understated.** Lean proofs exist for **all three** packed representations,
not only F_3:

- `proofs/Gf2Algebra/Proofs/Bipedal3Correctness.lean` (358 lines) — lane truth
  tables (`:93`, `:104`, `:117`, `:132`) lifted to word level (`:149`, `:159`,
  `:175`, `:190`).
- `proofs/Gf2Algebra/Proofs/Packed5Correctness.lean` (543 lines) — decoder
  table (`:64`-`:81`), per-op lane tables (`:172`, `:195`, `:215`, `:232`),
  word lifts (`:269`, `:303`, `:337`, `:372`), `*_correct` against the
  Aeneas-extracted functions (`:402` onward).
- `proofs/Gf2Algebra/Proofs/Packed7Correctness.lean` (785 lines) — decoder
  contracts (`:107`, `:112`, `:117`, `:122`), three LUT-characterisation axioms
  (`:145` section header), `binary_op_word_spec` (`:356`).
- `proofs/Gf2Algebra/Proofs/RyserBounded.lean` (1060 lines) — Gray-code purity
  (`:222`), single-bit-flip identity (`:407`), `flipBit` bound (`:494`), subset
  bijection (`:531`), `gray_injective` (`:571`); the header at `:81-105` records
  that it is `sorry`-free and declares no new axiom.

The proof targets are pinned to a fixed inherent surface for the
Charon/Aeneas pipeline (`packed7.rs:485-497`, naming
`dev/plans/30e98ef1/d6_lean_packed7_sketch.md` §4 and the `packed5.rs:326-401`
pattern it adapts). **Consequence:** a new *public* packed representation
attracts a proof obligation by convention, and `AGENTS.md:114-116` requires an
approved sketch (lemma statements, strategy, exact production path) *before*
proof code. A permanent-internal, non-public kernel state does not obviously
attract one — which is another reason the public/internal boundary of REQ-11 is
the load-bearing design decision, not a formality.

---

## 2. Prior-art sweep

Documents that record facts a GPU permanent feasibility study must not
re-derive. Paths are repo-relative; each was opened.

**GPU permanent measurements and crossover**

- `dev/studies/b488f02c/feasibility-study.md` — the baseline. §2 capability
  inventory (`:69-135`), §4.1 equivalence precondition (`:297-327`), §4.2
  protocol (`:329-383`), §4.3 cell outcomes with the eight censored GPU cells
  (`:384-433`), §4.4 measured throughput incl. the GPU-vs-CPU table
  (`:563-668`), §4.5 sustained, §5 gap analysis G1–G8 (`:1022-1178`).
- `dev/studies/b488f02c/throughput-2026-08-07.csv` — 120-cell grid, seven
  backends, full provenance preamble. The per-cell receipt format this study
  should extend rather than replace.
- `dev/studies/b488f02c/sustained-2026-08-07.csv` — steady-state re-measurement
  of nine configurations; the source of the 1.80 % worst cross-run disagreement
  quoted at `feasibility-study.md:621-625`.
- `dev/studies/b488f02c/equivalence-2026-08-07.csv` — 72 rows, `reference`
  column recording scalar vs generic oracle per row (66 / 6 split,
  `feasibility-study.md:319-321`).
- `dev/benchmarks/gf2_algebra_permanent/s5_gpu_crossover-2026-05-15.csv` — the
  2026-05-15 receipt reporting 28.65× / 30.32× at n=24/28, q=3, M=256. Superseded
  in interpretation: `feasibility-study.md:647-659` shows the baseline was the
  *slower* AVX2 single-thread path and restates the same configuration as
  0.46× / 0.44× against the best CPU path.
- `dev/archive/ae82bd73-gf2-algebra-permanent/plans/a9e461de/s5_gpu_crossover.md`
  — the design behind that receipt.
- `dev/archive/ae82bd73-gf2-algebra-permanent/plans/9480f8a6/s1g_gpu_speedup_results.md`
  — the M=80-per-round batching rationale (`permanent_gpu_speedup/src/main.rs:18`)
  and a 7176.9 s single-matrix GPU determinism run.
- `dev/archive/ae82bd73-gf2-algebra-permanent/active/ae82bd73-w5-gpu-verification.md`
  and `.../ae82bd73-w5-gpu-dispatcher-verification.md` — the on-device evidence
  format accepted at review for the three kernels and the dispatcher, including
  the canonical `--manifest-path` build invocation (`:19-25`).

**Watchdog / launch duration — the most directly relevant prior art**

- `dev/archive/ae82bd73-gf2-algebra-permanent/plans/b293af5a/r4_gpu_uniformity_resample.md`
  §2.5 (`:167-185`) — the only *calibrated* watchdog record in the tree: the
  hang boundary is placed at ≈190–200 s per launch; bounded sub-batches keep
  every launch at ≈10–117 s; q-aware work budgets on `sub_batch · 2^n` of
  4.0e9 (q=3), 1.3e9 (q=5), 3.5e8 (q=7); 400 ms host cooldown; and the F_3
  Bipedal3 kernel never tripped it "even on a 2300 s single launch at n=32".
- `dev/research/perm_uniformity_gpu/src/main.rs:100-121` — the executable form
  of that mitigation, with env-overridable budget and cooldown.
- `dev/studies/b488f02c/gpu-hang-2026-08-07.log` — a single `HW Exception by
  GPU node-1 … reason :GPU Hang` at q=3, n=24, M=4096 (`:52-53`), with an
  explicit retraction of the watchdog attribution (`:36-43`: "nothing here
  attributes the hang to a watchdog timeout … no diagnostic was captured that
  would separate them") and a note that the file supports no claim in the study
  (`:59-67`).

  **These two disagree and the plan must not paper over it.** The r4 doc
  asserts a watchdog mechanism and a calibrated boundary; b488f02c retracted the
  mechanism attribution for its own observation and recorded the retraction per
  `@/inv/falsification-preserved` (`feasibility-study.md:264-268`). REQ-04's
  "watchdog-safe work bounds" should be re-derived by measurement in this study,
  citing r4's numbers as a prior calibration rather than as an established
  device property.

**F_5 / F_7 representation decisions (the record this study re-opens)**

- `dev/archive/ae82bd73-gf2-algebra-permanent/plans/f10152f6/r2_f7_encoding_decision.md`
  — **F_7 Candidate D was already measured and rejected.** Summary at `:7-22`;
  the workload model that rejected it at `:145-147` ("each Gray-code transition
  does 1 packed add/sub … and ~`n−1` packed muls"); the numbers at `:164-184`
  (D is 9.5× faster on add, ~1.7× slower on mul, Ryser-weighted 0.62×, so A
  wins by 1.61× at n=36). §7 (`:205-211`) explicitly permits a SIMD re-bench of
  A vs D but states it is "**not** a re-decision authority for T19".
- `dev/archive/ae82bd73-gf2-algebra-permanent/plans/6b3f6054/r1_f5_encoding_decision.md`
  — F_5 chose Candidate D (three-plane) under the revised §8 rule (`:7-19`);
  this is why `Packed5` is already bit-sliced and `Packed7` is not.
- `dev/archive/ae82bd73-gf2-algebra-permanent/plans/r2_packed_encoding_generalizations.md`
  — the cross-prime "two algebraic gifts" map (`:11-24`): Fermat-like
  `p − 1 = 2^m` for {3,5,17,257,65537} vs Mersenne-like. The algebraic backing
  for the representation study's "why F_3 is exceptional" section.
- `dev/archive/ae82bd73-gf2-algebra-permanent/plans/r2_f3_f5_f7_cross_prime_comparison.md`
  — F_3 bipedal measured 13–22× faster than F_3 LUT-A on the same harness;
  records that F_5/F_7 have no candidate with comparable headroom.
- `dev/archive/ae82bd73-gf2-algebra-permanent/plans/60c30e2d/r3_multi_word_streaming.md`
  — the multi-word streaming design for n > 64 on F_3 (`N_MAX_MULTIWORD = 255`),
  the precedent for any multi-word F_7 accumulator alternative.

**Lean / proof-obligation precedent**

- `dev/archive/ae82bd73-gf2-algebra-permanent/plans/30e98ef1/d5_lean_packed5_sketch.md`
  and `.../d6_lean_packed7_sketch.md` — the approved sketch format a new public
  representation would have to match.
- `dev/archive/ae82bd73-gf2-algebra-permanent/plans/d3_lean_ryser_sketch.md`,
  `.../a0c0a45f/d2_lean_bipedal3_sketch.md` — same, for the algorithm layer.

**Process / session context**

- `dev/sessions/2026-08-07-research-frontier-handoff.md` — resume contract for
  b488f02c, standing decisions (`:47-58`) including the research-review gate
  model pinned to `gpt-5.6-sol` at xhigh.
- `dev/sessions/2026-08-08-b488f02c-review-rca.md` — the RCA for that review
  cycle; the failure modes a study of this shape hits at gate time.
- `dev/studies/b488f02c/literature-search-2026-08-08.md` — external-baseline
  search record; relevant only to the sampling epic, not to kernel design.
- `dev/plans/806eb14e-hip-gpu-prototype/hip_gpu_prototype_wave.md` — the wave
  plan that established the crate's evidence protocol (`## Evidence protocol`,
  `:51`) and the multi-arch `.co` seam (design doc §6, referenced from
  `build.rs:50` and `host/arch.rs:4`).
- `dev/plans/16283d6f-fieldmatrix-gpu/gpu_fieldmatrix_sketch.md` — an unrelated
  GPU epic sketch; relevant only for its storage/transfer-model section
  (`:33-52`) as a precedent for how this repo frames device residency.

**Not found.** No document records a wave-cooperative or intra-matrix GPU
decomposition for any workload in this repository. No occupancy, register-count,
spill, or `rocprof`/`rocprofv3` measurement of any permanent kernel exists in
the tree. REQ-04's occupancy/private-memory/spill quantification is entirely
new work with no in-tree tooling precedent.

---

## 3. Consumer sweep

Complete list of files referencing the surfaces a go decision might change.
Excludes `target/` and `.claude/worktrees/`.

### `permanent_batch_bipedal{3,5,7}` (the GPU dispatcher API)

Production:
- `crates/gf2-algebra/src/gpu.rs` — definitions at `:264`, `:361`, `:467`;
  module docs `:3-4`, `:36`, `:45-47`, `:99`, `:112`.
- `crates/gf2-kernels-hip/src/permanent/mod.rs:1098` — rustdoc back-reference.

Tests:
- `crates/gf2-algebra/tests/gpu_dispatcher.rs` — six tests: `:99`, `:144`,
  `:190` (n=24 criterion), `:240`, `:271`, `:305` (n=16 smoke). Whole file is
  `#![cfg(feature = "hip")]` and every test is `#[ignore]`.

Docs:
- `crates/gf2-algebra/README.md:131`.

dev/research (all four crates call the API directly):
- `dev/research/permanent-sampling-feas/src/backend.rs:294`, `:299`, `:304`.
- `dev/research/perm_uniformity_gpu/src/main.rs:62`, `:562`, `:569`, `:589`,
  `:596`, `:617`, `:636`, `:656`, `:937`, `:950`, `:963`; README at `:20`.
- `dev/research/permanent_gpu_crossover/src/main.rs:31`, `:97`;
  `tests/smoke.rs:13`, `:48`; `README.md:9`.
- `dev/research/permanent_gpu_speedup/src/main.rs:47`, `:201`;
  `src/det_check.rs:3`, `:33`, `:75`, `:77`; `tests/smoke.rs:13`, `:39`, `:68`,
  `:97`.

Archived docs (no code impact, but any API rename makes them stale):
`dev/archive/ae82bd73-gf2-algebra-permanent/active/ae82bd73-handoff-13.md:11`,
`.../active/ae82bd73-w5-gpu-dispatcher-verification.md:31-69`,
`.../plans/9480f8a6/s1g_gpu_speedup_results.md:13,17,19,147,168,175`,
`.../plans/a9e461de/s5_gpu_crossover.md:18`,
`.../plans/b293af5a/r4_gpu_uniformity_resample.md:30,52,170`.
Live docs: `dev/studies/b488f02c/feasibility-study.md:86,377,1184`.

### `Packed7` / `Packed7Matrix`

Production code:
- `crates/gf2-algebra/src/packed/packed7.rs` (definition, 2488 lines)
- `crates/gf2-algebra/src/packed/mod.rs` (re-export)
- `crates/gf2-algebra/src/packed/scalar.rs` (the `PackedField` scalar reference)
- `crates/gf2-algebra/src/permanent/bipedal7.rs`
- `crates/gf2-algebra/src/permanent/mod.rs`
- `crates/gf2-algebra/src/gpu.rs`
- `crates/gf2-algebra/src/lib.rs`
- `crates/gf2-algebra/Cargo.toml` (the `f7` feature)
- **`crates/gf2-kernels-simd/src/bipedal/packed7.rs`** — an independent AVX2
  implementation of the same LUT encoding (`:1-11`), with its own
  compile-time LUTs "mirrored from gf2-algebra::packed::packed7"
  (`:36-40`). Entry points live in
  `crates/gf2-kernels-simd/src/x86/bipedal_avx2_packed7.rs`.
- `crates/gf2-kernels-simd/Cargo.toml`
- `crates/gf2-kernels-hip/hip/permanent/permanent_bipedal7.hip` (LUT layout
  SSOT reference at `:19-34`)

Tests: `crates/gf2-algebra/tests/cas_cross_validation.rs`,
`crates/gf2-algebra/tests/gpu_dispatcher.rs`,
`crates/gf2-kernels-hip/tests/permanent_f7.rs`.

Proofs: `proofs/Gf2Algebra/Proofs/Packed7Correctness.lean`.

dev/research: `permanent-sampling-feas/src/{backend,equivalence,main,protocol}.rs`,
`perm_uniformity_gpu/src/main.rs`, `perm_uniformity/src/main.rs`,
`perm_uniformity/tests/smoke.rs`, `dev/studies/b488f02c/anchor-report/src/main.rs`.

Docs: `crates/gf2-algebra/README.md`, `ROADMAP.md`,
`dev/active/150d7d79/150d7d79-toolchain-upgrade.md`,
`dev/active/b4b4b9ee-tech-debt-2026-06-30/b4b4b9ee-assessment-report.md`,
`dev/studies/b488f02c/feasibility-study.md`, plus the archived handoffs and
plan docs listed by the sweep.

### `Packed5` / `Packed5Matrix`

Identical shape. Production: `packed/packed5.rs`, `packed/mod.rs`,
`packed/scalar.rs`, `permanent/bipedal5.rs`, `permanent/mod.rs`, `gpu.rs`,
`lib.rs`, `Cargo.toml` (`f5`),
**`crates/gf2-kernels-simd/src/bipedal/packed5.rs`** (AVX2 three-plane
implementation, 6-in/3-out stream API, `:17-25`; entry points in
`crates/gf2-kernels-simd/src/x86/bipedal_avx2_packed5.rs`),
`crates/gf2-kernels-simd/Cargo.toml`,
`crates/gf2-kernels-hip/hip/permanent/permanent_bipedal5.hip`.
Tests: `cas_cross_validation.rs`, `gpu_dispatcher.rs`,
`crates/gf2-kernels-hip/tests/permanent_f5.rs`.
Proofs: `proofs/Gf2Algebra/Proofs/Packed5Correctness.lean`.
Same dev/research and doc consumers as `Packed7`.

### `permanent_bipedal5` / `permanent_bipedal7`

- Definitions: `permanent/bipedal5.rs:108` / `:173`, `permanent/bipedal7.rs:110`
  / `:163`; re-exported via `permanent/mod.rs`.
- `dev/research/permanent-sampling-feas/src/backend.rs:238-243` (Scalar and
  Rayon backends) and `equivalence.rs:95-96` (support bound in prose).
- `crates/gf2-kernels-hip/tests/permanent_f5.rs`,
  `crates/gf2-kernels-hip/tests/permanent_f7.rs` (both import the
  `*_singleword` variants as GPU references).
- `crates/gf2-algebra/tests/gpu_dispatcher.rs`.
- `dev/research/perm_uniformity/src/main.rs`, `perm_uniformity_gpu/src/main.rs`,
  `dev/studies/b488f02c/anchor-report/src/main.rs`.
- Docs: `crates/gf2-algebra/README.md`, `feasibility-study.md:82-85`,
  `dev/active/0de41c82/bipedal-f5-f7-representation-study.md:66,88-89`.

**Two stale doc references found in production rustdoc.** `packed5.rs:45`
cites `dev/plans/6b3f6054/r1_f5_encoding_decision.md` and `packed7.rs:34`
cites `dev/plans/f10152f6/r2_f7_encoding_decision.md`; both moved to
`dev/archive/ae82bd73-gf2-algebra-permanent/plans/…`. The same two paths appear
in `crates/gf2-algebra/Cargo.toml`'s `f5` / `f7` feature comments and in
`crates/gf2-kernels-simd/src/bipedal/packed5.rs:3` / `packed7.rs:3`. These are
`@/inv/single-source-prose` staleness defects that any task touching those
files will be expected to fix in the same change.

---

## 4. Primitive verification

### (a) The harness's protocol and receipts can be reused/extended — **confirmed, with one gap**

The protocol module is normative and self-describing (`protocol.rs:34-50`
declares itself "the single normative statement of what a censored row
means"), the CSV headers are constants (`CELL_CSV_HEADER` at `protocol.rs:196`,
`SUSTAINED_CSV_HEADER` at `:730`), and the provenance preamble is generated
from probed host state (`env.rs:130`). Adding a backend is a one-line enum
change: the grid is driven from `Backend::ALL` (`backend.rs:60-68`) precisely
so a new path cannot miss the schedule — that fix is recorded at
`feasibility-study.md:275-282` as the correction for a superseded receipt set
that measured six backends where seven applied.

**Gap.** The harness measures the *composite campaign hot path* — generate,
evaluate, reduce to a histogram, append-and-flush a shard
(`protocol.rs:5-19`). REQ-03 asks for "kernel-only and end-to-end throughput"
and REQ-09 asks for *isolated* Gray-update, horizontal-product and end-to-end
receipts. The current cell reports four component times (generate / evaluate /
reduce / store) but has no sub-evaluate decomposition and no kernel-only timer
(no HIP event instrumentation exists anywhere in the crate). The three nested
measurement shapes REQ-09 requires are new harness work, not a configuration
of the existing one.

### (b) Equivalence checking vs a CPU oracle, per matrix — **confirmed**

`equivalence::check` (`equivalence.rs:104`) builds one shared batch from a
reserved seed stream (`shared_batch`, `:68`; stream index 0 is reserved,
`:69-71`) plus a matching unpacked batch for the generic path
(`shared_raw_batch`, `:57`), then compares **element-wise per matrix**:
`row.mismatches = reference.iter().zip(got.iter()).filter(|(a,b)| a != b).count()`
(`:178-182`). Unsupported backends are recorded with a reason rather than
dropped (`:158-162`). Reference selection is scalar-where-supported, generic
otherwise (`:107-126`), and the chosen reference is recorded per row.

There is also an oracle *outside* the kernel family: `exact_zero_count_order3`
(`equivalence.rs:210`) enumerates all `q^9` order-3 matrices and compares the
production kernel against an independent six-term permanent expansion
(rationale at `:201-207`), with a hard-coded Scheinerman2024 anchor
`z(3) = 8_163` asserted at `:345`.

### (c) Seeded suites with Gray-range boundaries — **contradicted; must be built**

The harness samples uniform random matrices only (`MatrixSampler`,
`sampler.rs`; call sites `equivalence.rs:58-63`, `:71-87`). Nothing in
`dev/research/permanent-sampling-feas/` constructs matrices or Gray indices
targeting a chunk boundary, an add-vs-sub transition, an exponent class, or a
zero-containing product. There is no seeded fixture suite for those cases
anywhere in `crates/` either — `crates/gf2-algebra/src/permanent/parallel_bipedal3.rs`
tests exercise chunk sizes (`:396` `test_parallel_with_chunk_matches_default_wrapper`,
`:412`, `:420`, `:428`) but by whole-result agreement, not by boundary
construction.

REQ-02 (empty/singleton, Gray-range boundaries, add and sub transitions) and
REQ-10 (active-lane masking, zero-containing products, every nonzero exponent
class, n ∈ {16, 20, 24}, observed zero-fast-path frequency against
`((q−1)/q)^n`) therefore require a **new deterministic fixture suite**. This
should be sized as its own worker task; it is a prerequisite of every
candidate-measurement task, not a rider on one.

The exact marginal expectations REQ-10 wants compared against are already
tabulated in `dev/active/0de41c82/bipedal-f5-f7-representation-study.md:160-166`
(F_5 n=20: 1.153 %; F_5 n=24: 0.472 %; F_7 n=16: 8.489 %; F_7 n=20: 4.582 %;
F_7 n=24: 2.473 % nonzero slow path).

### (d) HIP kernels are testable in isolation — **confirmed; the pattern is fixed**

Three test files exercise the `.hip` entry points through the raw FFI, not
through `gf2-algebra`:

- `crates/gf2-kernels-hip/tests/permanent_f3.rs` — n ∈ {16, 24, 32, 40, 63},
  contractual per-n matrix counts at `:19-36`.
- `crates/gf2-kernels-hip/tests/permanent_f5.rs` — n ∈ {8, 12};
  `#![cfg(feature = "hip")]` at `:19`; calls
  `gf2_kernels_hip::permanent::compute_permanent_gf5_batch` (`:28`) against
  `permanent_bipedal5_singleword` (`:26`).
- `crates/gf2-kernels-hip/tests/permanent_f7.rs` — n ∈ {8, 12} plus the
  `__constant__` MUL_LUT checksum test; explains at `:30-31` why n is capped
  at the CPU reference's bound.

Conventions, all three files:

- Whole file gated `#![cfg(feature = "hip")]`.
- Every test `#[ignore = "external: gfx1030 device required"]`.
- Device buffers acquired via a shared helper, `common::run_with_device_buffers`
  (`crates/gf2-kernels-hip/tests/common/mod.rs`), with a deterministic
  `xorshift64` generator.
- Canonical invocation is `--manifest-path crates/gf2-kernels-hip/Cargo.toml`
  because the crate is workspace-excluded (`permanent_f7.rs:19-22`; also
  `AGENTS.md:30-32`).

The `hip` feature is defined at `crates/gf2-kernels-hip/Cargo.toml`
(`hip = []`) and chained from `gf2-algebra`
(`hip = ["dep:gf2-kernels-hip", "gf2-kernels-hip/hip"]`). `gf2-algebra`'s
`gpu` module is gated at `crates/gf2-algebra/src/lib.rs:84-85`; the HIP crate's
`permanent` module at `crates/gf2-kernels-hip/src/lib.rs:73-74`.

---

## 5. Architecture fit

### Where the prototype kernels belong

**Recommendation: a new `dev/research/` crate, not an experimental module in
`crates/gf2-kernels-hip`.** Reasons, all evidenced:

1. Every prior kernel-representation study in this repo lived in
   `dev/research/`: `f5_packing`, `f7_packing`, `f3_bipedal`, `f17_packing`,
   `simd_batching_bench`, `intrinsic_feasibility_stub`. The F_5 and F_7
   production types were then transliterated from those prototypes
   (`packed5.rs:44-45`, `packed7.rs:34-35`).
2. `dev/research` is a `permanent_paths` entry in `.jit/config.toml:164`, so
   the prototypes survive archival of the issue's `dev/active` directory.
3. `crates/gf2-kernels-hip` is workspace-excluded and ROCm-only
   (`AGENTS.md:30-32`); adding experimental modules there puts unvalidated
   kernels behind the same feature flag as production ones and widens the
   `unsafe` surface subject to `@/inv/unsafe-kernel-isolation`.
4. Retained-but-rejected candidates (REQ-08's explicit requirement) belong
   somewhere permanent that is not production code. `dev/research/f7_packing`
   already holds four candidates including the rejected D, and
   `r2_f7_encoding_decision.md:229-235` states that keeping them there was
   deliberate, "for future re-evaluation".

If a `.hip` source must live next to the Rust harness, the crate can carry its
own `build.rs` invoking `hipcc` — `dev/research/permanent-sampling-feas`
already demonstrates the standalone non-workspace crate pattern
(`Cargo.toml:10-20`, `[workspace]` stanza at the end).

**Mandatory `.gitignore`.** Every existing `dev/research/` crate carries a
two-line `.gitignore` with exactly `target/` and `Cargo.lock`, verified in
`f7_packing`, `f5_packing`, `permanent-sampling-feas`, `permanent_gpu_common`,
`permanent_gpu_crossover`, `permanent_gpu_speedup`, `perm_uniformity_gpu`. Any
new stub must add it before the first commit. (Related: `jit archive --execute`
enforces a 512 MB cap over `dev/`, so `target/` must never be tracked.)

### Feature-gating and CPU-fallback conventions

- HIP code gates on the `hip` Cargo feature at module granularity
  (`gf2-kernels-hip/src/lib.rs:62-74`, `gf2-algebra/src/lib.rs:84-85`), and
  the GPU-facing test files gate at file level (`#![cfg(feature = "hip")]`).
- Every GPU rustdoc entry point states the CPU fallback *as runnable code*:
  `gpu.rs:217-224` (F_3), `:313-322` (F_5), and the equivalent for F_7. The
  study's production design (REQ-06 "tested CPU fallback") should follow that
  shape.
- `@/inv/accelerator-safe-fallback` requires unsupported capabilities and
  recoverable resource failures to select a *tested* fallback while fatal
  kernel/driver failures stay explicit. The current permanent dispatchers do
  neither — they `assert_eq!` on every HIP return code and panic
  (`mod.rs:973-976`, `:980-983`), with the panic contract documented at
  `mod.rs:913-917`. A go decision has to close that gap; a no-go decision
  should record it as a finding.
- Launch geometry must be a pure function of problem size, never of a runtime
  occupancy query (`host/launch.rs:3-19`, `:39-41`), because the pipeline's
  determinism contract depends on it. This directly constrains REQ-01: the
  wave-cooperative decomposition may not size its grid from
  `hipOccupancyMaxPotentialBlockSize` or any runtime probe. `LaunchDims::explicit`
  (`host/launch.rs:124`) is the sanctioned escape for bespoke 1-D mappings and
  records the choice without enforcing purity.

### Where this issue's receipts belong

`jit doc dir 0de41c82 dev/active` resolves to `dev/active/0de41c82/`, which
already holds the representation study attached as a `design` doc. The issue
Notes say prototype artifacts and final findings belong in "this issue resolved
study directory". Two constraints interact:

- `dev/active` and `dev/studies` are both `managed_paths`
  (`.jit/config.toml:163`) — content there is issue-scoped and archived with the
  issue. `dev/research` and `dev/archive` are `permanent_paths` (`:164`).
- The b488f02c precedent put the study document and all CSV receipts under
  `dev/studies/b488f02c/` while its *code* prototype went to
  `dev/research/permanent-sampling-feas/`.

**Recommendation:** findings document and all receipts under the issue's
resolved doc directory (matching what `jit doc dir` returns at authoring time,
which is `dev/active/0de41c82/` today), executable prototypes under
`dev/research/<new-crate>/`, and every doc attached with `jit doc add` rather
than referenced by an inline `dev/...` path.

---

## 6. Architectural-invariant check

Invariant texts resolved via `jit item show`; the repository's projected list is
at `AGENTS.md:128-151`.

### `@/inv/convention-convergence` — the binding constraint on REQ-11

> "A shared convention or abstraction has one form: work that finds it harmful
> or ill-fitting changes it at its source, or reports the mismatch as a blocking
> concern before proceeding. A local parallel variant, a private helper
> duplicating a shared mechanism, or a bypass around an abstraction is a defect
> unless it is a named, cited exception with a tracked convergence condition."

Three surfaces are exposed:

1. **A second F_7 representation.** REQ-11 requires the design to distinguish a
   public packed-field representation from a permanent-specialized internal
   state. Option 2 in the representation study
   (`bipedal-f5-f7-representation-study.md:257-260`) — "keep the LUT
   representation public and define the three-plane state as an internal
   permanent-kernel representation with explicit scope, shared behavioral tests,
   and a tracked convergence condition" — is written to satisfy this invariant
   literally, and the breakdown must carry the *named exception plus tracked
   convergence condition* as a deliverable, not as a note. Option 1 (change
   `Packed7` at its source) is the other compliant path and is far more
   expensive: it touches the SIMD mirror (`gf2-kernels-simd/src/bipedal/packed7.rs`
   and `x86/bipedal_avx2_packed7.rs`), the HIP LUT upload path
   (`permanent_bipedal7.hip:275`), and `proofs/Gf2Algebra/Proofs/Packed7Correctness.lean`.
2. **Re-opening a ratified decision.** `r2_f7_encoding_decision.md:205-211`
   states the D-vs-A re-bench is "not a re-decision authority". If this study
   concludes D is right for the permanent, the decision document must be amended
   at its source with the new workload evidence — not silently contradicted by a
   new document. Note the archived R2 doc lives under `dev/archive`, a
   `permanent_path`; amending it is a deliberate act requiring approval.
3. **A parallel GPU dispatch mechanism.** Building persistent buffers or streams
   for the permanent path while `host::{DeviceBuffer, HipStreamPool,
   LaunchDims}` exist would create exactly the "private helper duplicating a
   shared mechanism" the invariant forbids. The production design in REQ-06
   should name those types.

### `@/inv/falsification-preserved`

> "Data that contradicts a criterion, hypothesis, or cited claim is recorded
> together with the contradiction; silent rework of the falsified statement is
> a defect."

Four existing falsifications the breakdown inherits and must not overwrite:

- The `W/probe` upper bound on batched GPU rate, disproved by a measurement
  beside it (`feasibility-study.md:441-462`).
- The retracted watchdog attribution for the single GPU hang
  (`gpu-hang-2026-08-07.log:36-43`, `feasibility-study.md:264-268`), which sits
  in tension with `r4_gpu_uniformity_resample.md` §2.5's calibrated boundary.
- The superseded n=28 ordering claim (2.3× for intra-matrix rayon), corrected
  to an unresolved 1.6 % gap (`feasibility-study.md:636-645`).
- The 28.65×/30.32× GPU crossover headline, restated as 0.46×/0.44× against the
  best CPU path (`feasibility-study.md:647-659`).

REQ-02 and REQ-08 add forward obligations of the same kind: a rejected
candidate must retain its falsifying correctness, compile, or resource
evidence. That means the prototype crate must keep rejected candidates
*compiling and tested*, as `dev/research/f7_packing` does today for its four.

### `@/inv/backend-behavioral-equivalence`

> "Scalar, SIMD, parallel CPU, and GPU implementations expose equivalent
> observable results within their declared numerical contract."

Satisfied today for the paths the campaign uses
(`feasibility-study.md:326-327`). Every new prototype backend joins that
obligation, and `@/inv/shared-test-contracts` means it must run the *same*
behavioral suite rather than a bespoke one. Concretely: a new backend added to
`Backend::ALL` (`backend.rs:60-68`) is automatically scheduled by both the
equivalence check and the grid, which is the cheapest compliant path.

### Others that bear on this work

- `@/inv/benchmark-backed-performance` — every crossover claim needs a
  reproducible protocol and a committed receipt from an uncontended host.
  `AGENTS.md:96-98` adds that `GF2_BENCH=1` is only for a prepared benchmark
  host and requires the `dev/scripts/` lock wrapper.
- `@/inv/deterministic-seeded-execution` — a fixed seed must give identical
  results across worker counts and accelerator fallbacks. For a
  wave-cooperative kernel this constrains the partial-sum reduction: the F_3
  CPU path relies on F_3 addition being commutative and associative
  (`parallel_bipedal3.rs:196-198`), which holds equally in F_5 and F_7, so a
  tree reduction across lanes is sound — but the reduction order still must not
  depend on runtime scheduling.
- `@/inv/canonical-cutover` — if the study proposes replacing `Packed7`'s
  representation, the superseded one is removed after cutover; compatibility
  code needs a named migration boundary with a tracked removal condition.
- `@/inv/unsafe-kernel-isolation` — only `gf2-kernels-simd` and
  `gf2-kernels-hip` may hold production `unsafe`, each boundary carrying an
  explicit safety contract. A `dev/research` prototype is not production, but
  anything promoted must land in the HIP crate.
- `@/inv/single-source-prose` — see the stale `dev/plans/...` citations in §3.

### Gate implications for the breakdown

From `.jit/gates.toml`: `asm-artefact-present` (`:101`) fires only on changes
under `crates/gf2-kernels-simd/src/x86/*.rs` (`scripts/asm-artefact-present.sh:28-45`,
excluding `mod.rs`). HIP work does not trip it; changing the AVX2 `Packed5`/`Packed7`
entry points in `x86/bipedal_avx2_packed{5,7}.rs` **does**, and would require
regenerating the sibling `crates/gf2-kernels-simd/src/x86/asm/*.asm.txt`.
`cargo-ci` (`:113`) runs `./scripts/cargo-ci.sh` over the default workspace,
which excludes `gf2-kernels-hip` and every `dev/research` crate — so a
prototype-only change passes `cargo-ci` without ever compiling; the breakdown
must name the explicit `--manifest-path` build and test invocations per
`AGENTS.md:30-32`. This issue's own gate set is `doc-review`,
`research-review`, `repo-validate`, `holistic-review` — no `cargo-ci`, no
`code-review` — consistent with a study container whose product is a document
plus prototypes.

---

## 7. Findings the breakdown should treat as facts

1. Wave-cooperative GPU permanent work has **no in-tree precedent**; the CPU
   chunked Gray-range split (`parallel_bipedal3.rs:237`) is the only structural
   prior art and is directly transferable.
2. The permanent kernels compile for **gfx1030 only** (`build.rs:43`); the
   six-arch `.co` pipeline compiles no-op probes. REQ-04's architecture-scope
   claim is currently a one-element set unless the study extends the build.
3. The F_7 three-plane Mersenne candidate was **already rejected once**
   (`r2_f7_encoding_decision.md:16-19`) under a Ryser workload model
   (`:145-147`) that does not match the shipped row-packed kernel. That is the
   study's central re-opening, and it needs the archived decision amended, not
   bypassed.
4. Both CPU horizontal folds are **serial per-lane scalar multiplies**
   (`packed5.rs:1345-1349`, `packed7.rs:474-480`), so the zero-mask /
   log-popcount fold of REQ-08 competes against a serial loop, not against a
   packed multiply. The F_7 rustdoc at `packed7.rs:468` misdescribes this.
5. Persistent buffers and streams **already exist and are public**
   (`host/mod.rs:28-31`); the permanent dispatcher simply does not use them.
6. Deterministic boundary fixtures for REQ-02/REQ-10 **do not exist** and are
   prerequisite work for every candidate-measurement task.
7. Kernel-only and sub-evaluate timing **do not exist** in the harness; REQ-03
   and REQ-09 need new instrumentation (no HIP event usage anywhere in the crate).
8. The watchdog record is **contested**: a calibrated ≈190–200 s boundary with
   q-aware work budgets in `r4_gpu_uniformity_resample.md` §2.5, against an
   explicit retraction of the mechanism attribution in
   `gpu-hang-2026-08-07.log:36-43`.
9. GPU F_5/F_7 losing to CPU is **measured, not assumed**: batch rayon wins by
   roughly 3× at every shared n for both fields
   (`feasibility-study.md:606-610`, table at `:575-584`), and all eight censored
   cells are GPU cells at (q, n) ∈ {5,7} × {24,28} (`:392-404`).
10. Minor defects found in passing, each fixable in whichever task touches the
    file: `gpu_dispatcher.rs:24` states "gfx1030's 36 compute units" against the
    receipted 80; `packed7.rs:468` claims LUT lookups the body does not perform;
    `packed5.rs:45`, `packed7.rs:34`, `gf2-algebra/Cargo.toml`, and both
    `gf2-kernels-simd/src/bipedal/packed{5,7}.rs:3` cite `dev/plans/…` paths that
    now live under `dev/archive/ae82bd73-gf2-algebra-permanent/plans/…`.
