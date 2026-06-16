# GPU batch BCH syndrome evaluation — design plan

JIT issue: `9012f8a0` (HIP/ROCm GPU-accelerated batch BCH syndrome evaluation)
Epic: `806eb14e` (`epic:hip-gpu-prototype`) · Story: `aeab2ee4` (`story:gpu-bch-syndrome`)
Status: design (pre-implementation). Author: agent:claude, 2026-06-16.

> Supersedes the 2026-04 breakdown stub at this path. The stub's performance
> section gated GPU against a *serial* CPU loop with guessed speedups; this
> revision gates decode-vs-decode against the best production CPU path (rayon),
> per the `a930be7f` precedent, and grounds every formula in the CPU source.

## 1. Goal and scope

Offload the **syndrome-evaluation** sub-step of BCH decoding to the GPU as a
batched kernel, proving exact CPU/GPU equivalence and measuring throughput
against the best production CPU path. Berlekamp-Massey and Chien search remain
on the CPU (per issue background: "keeps Berlekamp-Massey and Chien search on
CPU unless evidence shows they dominate").

**Integration shape (decided 2026-06-16):** an *evaluator*, not a pipeline
`Stage`. BCH syndrome eval maps received-bits -> `2t` syndromes, an intermediate
decode sub-step, so it is not a natural full-decode `Stage<In,Out>` like
`GpuLdpcBp` (`LlrBatch -> HardDecisionBatch`). We deliver:

1. A HIP kernel `crates/gf2-kernels-hip/hip/bch_syndrome.hip`.
2. A safe host wrapper `crates/gf2-kernels-hip/src/launch_bch_syndrome.rs`
   (`GpuBchSyndrome` evaluator), mirroring `launch_ldpc_bp.rs` conventions
   (`HipError` boundary, lazy device build, default-stream path with a
   stream-ordered seam).
3. A `gf2-coding` hook `BchDecoder::compute_syndromes_batch_gpu(...)` under
   `--features hip` that calls the wrapper; the existing CPU
   `compute_syndromes` path is untouched (no auto-threshold inside
   `decode_batch`).

No `gf2-sim` pipeline `Stage`. No production multi-backend selection (epic
non-goal).

## 2. Success criteria (verbatim from issue `9012f8a0`)

- [hard] GPU GF($2^m$) multiplication support matches CPU `Gf2mField`
  arithmetic exactly for the required table-backed fields, including
  exhaustive GF($2^4$) checks.
- [hard] HIP Horner syndrome evaluation matches CPU polynomial evaluation
  exactly on small BCH fixtures.
- [hard] DVB-T2 short and normal BCH syndrome fixtures match CPU exactly for
  valid codewords and injected-error codewords.
- [hard] Batch correctness covers the selected design workload and at least one
  smaller fixture; GPU batch syndromes match the CPU comparator exactly.
- [hard] GPU syndromes fed into the CPU Berlekamp-Massey and Chien pipeline
  decode the same received words as the CPU-only pipeline.
- [hard] Safe Rust wrappers in `gf2-kernels-hip` and feature-gated `gf2-coding`
  integration compile only behind the `hip` feature; default workspace builds
  succeed without ROCm installed.
- [hard] Performance evidence records hardware metadata, ROCm version,
  benchmark command, raw result paths, batch-size sweep, and phase timing where
  practical.
- [hard] GPU BCH syndrome evaluation is at least 5x faster than the best
  existing production CPU path at the selected design workload.

## 3. CPU reference (the SSOT we must reproduce bit-for-bit)

### 3.1 Syndrome construction — `BchDecoder::compute_syndromes` (`crates/gf2-coding/src/bch/core.rs:568`)

`S_i = r(α^i)` for `i = 1..=2t`, where `r(x)` is the received polynomial. The
coefficient vector passed to the evaluator is built as (core.rs:576-605):

- Parity bits first: `for i in (k..n).rev()` push `received[i]` -> `coeffs[0..r]`
  (so `coeffs[0] = received[n-1]`), `r = n - k`.
- Then message bits: `for i in (0..k).rev()` push `received[i]` ->
  `coeffs[r..n]` (so `coeffs[n-1] = received[0]`).
- Each pushed coefficient is `field.one()` (bit set) or `field.zero()` (clear).

Evaluation points: `alpha = field.primitive_element()`, then
`alpha_power = alpha; for _ in 0..2t { push alpha_power; alpha_power *= alpha }`,
giving `[α^1, α^2, ..., α^(2t)]`.

### 3.2 Horner recurrence — `FieldPoly::eval` (`crates/gf2-core/src/field/poly.rs:1130`)

`coeffs` is little-endian (`coeffs[0]` is the constant term). The recurrence is:

```rust
let mut result = self.coeffs.last().unwrap().clone(); // highest-index coeff
for i in (0..self.coeffs.len() - 1).rev() {
    result = result * x.clone() + self.coeffs[i].clone();
}
```

On the all-zero polynomial every result is the field zero (total contract; the
all-zero received vector needs no special case).

### 3.3 GF(2^m) multiplication — `Gf2mField` mul (`crates/gf2-core/src/gf2m/field.rs:1033`)

Table-backed path (the one we replicate; both DVB-T2 fields carry tables):

```text
mul(a, b):
  if a == 0 || b == 0: return 0
  order = (1 << m) - 1                 // 2^m - 1
  log_result = (log_table[a] + log_table[b]) % order
  return exp_table[log_result]
```

`log_table` has `2^m` u16 entries; `exp_table` has `order = 2^m - 1` u16 entries.
Addition in GF(2^m) is XOR.

## 4. Design workload and fields (decided 2026-06-16)

Parameters are read from `DvbBchParams::for_code` (`bch/dvb_t2/params.rs:54`) —
the SSOT; do not hardcode `n`/`k`/`t`/primitive-poly here (the breakdown stub
mis-transcribed the Short poly).

| Role | Config | n | k | t | 2t | field |
|------|--------|---|---|---|----|-------|
| Primary design workload | DVB-T2 Normal Rate 1/2 | 32400 | 32208 | 12 | 24 | GF(2^16) |
| Secondary (smaller field) | DVB-T2 Short Rate 1/2 | 7200 | 7032 | 12 | 24 | GF(2^14) |
| Small exhaustive fixture | textbook BCH over GF(2^4) | 15 | per t | small | — | GF(2^4) |

Table sizes: GF(2^16) -> 128 KB `exp` + 128 KB `log`; GF(2^14) -> 32 KB each.

## 5. GPU GF(2^m) arithmetic (criterion 1)

Upload the **exact CPU `exp`/`log` tables** to device global memory; the kernel
multiply is byte-identical to §3.3 by construction:

```c
// d_log: 2^m u16,  d_exp: (2^m - 1) u16,  order = (2^m - 1)
__device__ uint16_t gf_mul(uint16_t a, uint16_t b,
                           const uint16_t* d_log, const uint16_t* d_exp,
                           uint32_t order) {
    if (a == 0 || b == 0) return 0;
    uint32_t s = (uint32_t)d_log[a] + (uint32_t)d_log[b];
    if (s >= order) s -= order;          // (log_a + log_b) % order, single subtract
    return d_exp[s];
}
```

`s < 2*order` always, so one conditional subtract realises the modulus.
Tables live in global memory (L1/L2 + Infinity Cache absorb the hot `exp`
lookups; 256 KB for m=16 is negligible in 16 GB VRAM). Addition is `^`.

The host hands the tables to the wrapper from the live `Gf2mField`, so there is
a single source of truth and no on-device table regeneration.

## 6. Coefficient layout and kernel (criteria 2-4)

**Host-side reorder (keeps the kernel trivial and exactness in tested Rust).**
The `gf2-coding` hook reorders each received `BitVec` into the §3.1 `coeffs`
order (parity-reversed ++ message-reversed) and uploads it as a packed
little-endian bit stream of `n` bits per frame (`ceil(n/64)` u64 words). The
kernel runs a pure Horner pass with no knowledge of the parity/message split.

> Packed-bit upload (not bit-to-u32 expansion): the design workload is
> `batch * ceil(32400/64)` words ~= `batch * 507` u64, i.e. ~4 MB at batch 1024,
> vs ~133 MB if expanded to u32. The stub's u32 expansion is dropped.

**Thread mapping (decided 2026-06-16): one thread per `(frame, point)`.**
A flat grid of `batch * 2t` threads. Thread `(f, i)` evaluates `S_{i+1}` for
frame `f`:

```c
uint16_t alpha_i = d_points[i];          // α^(i+1), uploaded as u16
uint16_t acc = coeff_bit(f, n-1);        // coeffs[n-1] (0 or 1)
for (int d = n - 2; d >= 0; --d) {
    acc = gf_mul(acc, alpha_i, d_log, d_exp, order) ^ coeff_bit(f, d);
}
d_syndromes[f * (2*t) + i] = acc;        // u16 field element
```

`coeff_bit(f, d)` is one word load + shift from frame `f`'s packed stream.
Adjacent threads share the coeff stream (broadcast-friendly) and keep `d_exp`
cache-hot. Determinism is automatic: the recurrence is strictly sequential per
`(f,i)`, identical order to the CPU, so equality is exact and total.

Occupancy: the design workload is `batch * 24` threads (batch 1024 -> 24576
threads); the `n`-length sequential chain is the latency the batch hides. The
batch sweep (§11) finds the working point — no occupancy claims are asserted
without measurement.

## 7. Host wrapper API — `launch_bch_syndrome.rs`

Mirrors `launch_ldpc_bp.rs`. All FFI stays in `gf2-kernels-hip` behind
`// SAFETY:` comments; errors map to the existing `HipError`
(`OutOfMemory`/`UnsupportedArch` recoverable, surfaced not dispatched).

```rust
pub struct BchFieldTables { /* m, order, exp: Vec<u16>, log: Vec<u16> */ }

pub struct GpuBchSyndrome { /* device id, n, t, device tables + points, max_batch */ }

impl GpuBchSyndrome {
    /// Lazy builder — no device allocation until first evaluate.
    pub fn new(tables: &BchFieldTables, eval_points: &[u16],
               n: usize, t: usize, max_batch: usize, device_id: i32)
        -> Result<Self, HipError>;

    /// Default-stream batch: upload packed coeff streams, launch, sync, read back.
    /// `coeff_streams`: batch * ceil(n/64) u64 words in §3.1 order.
    /// Returns batch * 2t u16 syndromes (row-major per frame).
    pub fn evaluate_batch(&mut self, coeff_streams: &[u64], batch: usize)
        -> Result<Vec<u16>, HipError>;
}

// Stream-ordered seam (structured now, implemented later if needed):
// pub fn evaluate_batch_on_stream(&mut self, ..., stream: &HipStream,
//                                 scratch: &mut BchStreamScratch) -> Result<...>;
```

Stream scope (decided 2026-06-16): default-stream now, stream-ordered seam left
in place for a follow-up, no rework required.

## 8. gf2-coding hook

```rust
#[cfg(feature = "hip")]
impl BchDecoder {
    /// GPU batch of `compute_syndromes`; bit-identical to the per-frame CPU path.
    pub fn compute_syndromes_batch_gpu(&self, received: &[BitVec])
        -> Result<Vec<Vec<Gf2mElement>>, HipError>;
}
```

It extracts `exp`/`log` tables + `α^1..α^2t` from `self.code.field`, reorders each
`received` into the packed coeff stream, calls `GpuBchSyndrome::evaluate_batch`,
and rehydrates `u16` -> `Gf2mElement`. The CPU `compute_syndromes` is the oracle.
`gf2-coding`'s `hip` feature forwards to `gf2-kernels-hip`.

## 9. Build / multi-arch wiring

- Add `hip/bch_syndrome.hip` to the static-lib source list in
  `crates/gf2-kernels-hip/build.rs` (alongside `ldpc_bp`) and to its
  rerun-if-changed triggers.
- Per-arch blob (`kernels/<gfx>/bch_syndrome.cpp`) optional; gfx1030 mandatory,
  others best-effort, matching the existing `GFX_TARGETS` pattern.
- `pub mod launch_bch_syndrome;` + re-exports in `crates/gf2-kernels-hip/src/lib.rs`.

## 10. Correctness ladder (full — decided 2026-06-16)

1. **Exhaustive GF(2^4) mult**: all 256 `(a,b)` products, GPU `gf_mul` vs CPU
   `Gf2mField` — bit-identical.
2. **Uploaded-table equality**: device `exp`/`log` for GF(2^14) and GF(2^16)
   equal the CPU tables element-for-element.
3. **Small BCH(15)/GF(2^4) Horner fixture**: hand-derived expected syndromes for
   a known received word; GPU == hand value == CPU.
4. **DVB-T2 Short + Normal syndrome byte-identity**, 200 frames per config at a
   fixed seed, **mixed**: valid codewords (all-zero syndromes), `<=t`
   correctable errors, and `>t` uncorrectable errors. All `2t` `u16` syndromes
   equal CPU with **zero tolerance** (exact integer GF arithmetic — no ULP
   drift, unlike LDPC).
5. **Decode-equivalence** (criterion 5): feed GPU syndromes into CPU
   Berlekamp-Massey + Chien and assert the decoded output equals the CPU-only
   pipeline on the same valid + injected-error frames.

Tests are `#[cfg(feature = "hip")]`, carry `#[ignore = "sim: ..."]` where they
need the GPU, and skip cleanly when `device_mem_info().is_err()` (mirrors
`gpu_ldpc_byte_identity.rs`). Field-level checks (1-3) live in
`crates/gf2-kernels-hip/tests/`; chain checks (4-5) in
`crates/gf2-sim/tests/gpu_bch_syndrome_byte_identity.rs`.

## 11. Performance evidence (criteria 7-8)

Bench bin `crates/gf2-sim/src/bin/gpu_bch_syndrome_throughput.rs`, patterned on
`gpu_ldpc_throughput.rs`:

- **Apples-to-apples divisor (decided 2026-06-16):** CPU `compute_syndromes`
  measured **in isolation** (no BM/Chien), at single-thread **and** rayon-24T,
  same DVB-T2 Normal Rate 1/2 config and operating point.
- **`[hard]` gate:** GPU syndrome throughput >= **5x the best production CPU
  path** = rayon-24T `compute_syndromes` (the issue's "including rayon where
  production uses rayon"). The single-thread number is reported for context, not
  the gate divisor. This follows the `a930be7f` decode-vs-decode precedent
  (avoid GPU-vs-serial category confusion — the defect in the stub's perf table).
- **Batch-size sweep** (e.g. 64 / 256 / 1024 / 4096) and **phase timing** where
  practical: H2D (coeff streams), kernel, D2H (syndromes). Tables uploaded once,
  excluded from per-batch timing (amortised).
- Record hardware metadata, ROCm version, exact command, and raw result paths.
- Receipt: `dev/benchmarks/gf2-sim/gpu-bch-syndrome-receipt.md`, attestation
  table matching `gpu-stages-receipts.md`.

No speedup numbers are asserted in this plan. The 5x target is `[hard]` and must
be backed by measurement; if the data falsifies it, amend only with the observed
number + user approval (project "measurements, not guesses" rule).

## 12. Risks / open items

- **5x vs 24-thread CPU is the binding constraint.** Syndrome eval is
  mul-heavy and memory-light (coeffs are bits), favourable for GPU, but `t=12`
  -> only 24 points/frame; throughput rests on batch parallelism. The batch
  sweep finds the crossover; report honestly.
- **exp-table cache behaviour** for m=16 (128 KB) under `batch*24` divergent
  `log` values — measure L2/IC hit rate; acceptable for a prototype either way.
- **Coeff bit-unpacking cost** on device — one word load + shift per Horner
  step; cheap relative to a GF mul, but verify it does not dominate.

## 13. Criteria -> evidence traceability

| Criterion | Evidence |
|-----------|----------|
| GF(2^m) mult exact, exhaustive GF(2^4) | ladder 1-2 |
| Horner exact on small fixtures | ladder 3 |
| DVB-T2 short/normal, valid + error | ladder 4 |
| Batch + smaller fixture exact | ladder 4 (Normal + Short) |
| GPU syndromes -> CPU BM/Chien equal | ladder 5 |
| hip-gated; default build green w/o ROCm | feature gating §1,§8; CI `check` step |
| perf metadata + sweep + phase timing | §11 receipt |
| >= 5x vs best production CPU path | §11 gate |

## 14. Implementation manifest

| File | Change |
|------|--------|
| `crates/gf2-kernels-hip/hip/bch_syndrome.hip` | new — `gf_mul` + Horner kernel |
| `crates/gf2-kernels-hip/src/launch_bch_syndrome.rs` | new — `GpuBchSyndrome`, `BchFieldTables` |
| `crates/gf2-kernels-hip/src/lib.rs` | `pub mod` + re-exports |
| `crates/gf2-kernels-hip/build.rs` | add source + rerun trigger |
| `crates/gf2-coding/src/bch/core.rs` | `compute_syndromes_batch_gpu` under `--features hip` |
| `crates/gf2-coding/Cargo.toml` | `hip` feature -> `gf2-kernels-hip` |
| `crates/gf2-sim/tests/gpu_bch_syndrome_byte_identity.rs` | new — ladder 4-5 |
| `crates/gf2-kernels-hip/tests/...` | new — ladder 1-3 (field-level) |
| `crates/gf2-sim/src/bin/gpu_bch_syndrome_throughput.rs` | new — perf bin |
| `dev/benchmarks/gf2-sim/gpu-bch-syndrome-receipt.md` | new — attestation |
