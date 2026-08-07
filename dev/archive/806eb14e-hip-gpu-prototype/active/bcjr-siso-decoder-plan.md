# Plan: Trellis-Based BCJR SISO Decoder for dRM(32,21) — JIT 3e136982

## Context

The dRM(32,21) product code BLER at 1.0 dB is **0.667 vs. paper's 0.072** (9.27× gap).
Root cause: dRM(32,21) has 2 million codewords in 4 billion patterns. SOGRAND finds ~24
codewords in 50K queries, but the correct codeword is often indistinguishable in the APP
computation — the "not found" probability dominates and washes out the extrinsic signal.

eBCH(16,11) works because n=16 allows near-exhaustive ORBGRAND coverage (~65K total patterns
vs. 50K budget). dRM(32,21) cannot be fixed by increasing list size or query budget alone.

The fix is the **BCJR forward-backward algorithm** on the code trellis. It computes exact APP
LLRs in O(n × 2^(n-k)) time without enumerating codewords at all.

---

## Algorithm Background

### Trellis for a linear [n,k] block code

Given parity-check matrix H (m×n, m = n-k = 11 for dRM(32,21)):

- **State at boundary i**: s_i ∈ GF(2)^m — the partial syndrome of bits 0..i-1.
  Represented as a `u32` bitmask over m=11 bits. 2^11 = 2048 states total.
- **Precomputed column bitmasks**: `h_col[i]` = i-th column of H as a u32 (11-bit integer).
  Extracted from `BitMatrix::col_as_bitvec()`.
- **Transitions**:
  - `c_i = 0`: state s → s (no change)
  - `c_i = 1`: state s → s XOR h_col[i]
- **Start/end state**: must be 0 (zero syndrome = valid codeword).

### Log-domain BCJR (Log-MAP)

All messages in log-probability domain (f32). Uses the **Jacobian logarithm** (max-star
operator) for exact log-sum-exp rather than the max-log approximation, which is critical
at our operating SNR (0.75-1.0 dB) per literature recommendations (McEliece 1996, Springer
reduced-complexity Log-MAP analysis).

**Jacobian logarithm** (max-star):
```
max*(a, b) = max(a, b) + log(1 + exp(-|a - b|))
           = max(a, b) + J(|a - b|)
```
The correction term `J(Δ) = log(1 + exp(-Δ))` is negligible for |Δ| > 8.
Implement as: inline function with `ln_1p(exp(-delta))` for small Δ; 0.0 for Δ > 8.
This avoids a LUT while keeping full f32 precision. An optional LUT path (256 entries,
step 0.03125 over [0,8]) can be added later for SIMD.

**Normalization**: subtract `max(log_α[i])` every stage (n=32 is short enough that per-stage
normalization is cheap and ensures no f32 saturation). Per NASA trellis decoding report and
Springer Log-MAP analysis, normalization every 2-4 stages suffices for longer codes, but at
n=32 the overhead of per-stage normalization is negligible.

**Bit ordering**: standard binary order (columns 0..n-1) is provably optimal for RM code
trellis state complexity (Kasami & Takata; McEliece 1996). Use H columns in their natural
systematic form ordering.

**Branch metrics** (from combined LLR `l = L_ch[i] + L_apriori[i]`):
```
log_γ(c_i=0) = +l / 2
log_γ(c_i=1) = -l / 2
```

**Forward pass** (log-alpha):
```
log_α[0][0] = 0.0,  log_α[0][s≠0] = -∞
for i = 0..n:
    log_γ₀ = combined_llrs[i] / 2
    log_γ₁ = -combined_llrs[i] / 2
    for s in 0..NUM_STATES:
        if log_α[i][s] > -∞:
            log_α[i+1][s]         = max*(log_α[i+1][s],         log_α[i][s] + log_γ₀)
            log_α[i+1][s^h_col[i]] = max*(log_α[i+1][s^h_col[i]], log_α[i][s] + log_γ₁)
    normalize: subtract max(log_α[i+1]) from all entries
```

**Backward pass** (log-beta):
```
log_β[n][0] = 0.0,  log_β[n][s≠0] = -∞
for i = n-1..=0:
    same butterfly update, reversed direction
    normalize at each stage
```

**APP + extrinsic**:
```
for i = 0..n:
    log_p0 = max* over s: log_α[i][s] + log_γ₀ + log_β[i+1][s]
    log_p1 = max* over s: log_α[i][s] + log_γ₁ + log_β[i+1][s^h_col[i]]
    L_APP[i]  = log_p0 - log_p1
    L_ext[i]  = L_APP[i] - combined_llrs[i]
```

### Complexity

| Code          | n  | n-k | States | Ops/decode | vs. SOGRAND (50K queries) |
|---------------|----|-----|--------|------------|---------------------------|
| dRM(32,21)    | 32 | 11  | 2048   | ~130K      | ~390× fewer               |
| eBCH(16,11)   | 16 | 5   | 32     | ~1K        | trivial                   |
| eBCH(32,26)   | 32 | 6   | 64     | ~4K        | ~12K× fewer               |

Memory per decode: 2 × (n+1) × 2^(n-k) × 4 bytes = 2 × 33 × 2048 × 4 ≈ **540KB**
(within one frame; freed after decode).

---

## Implementation Plan

### Step 1: Create `crates/gf2-coding/src/bcjr/mod.rs`

**Struct**:
```rust
pub struct BcjrDecoder {
    h_cols: Vec<u32>,   // i-th entry = i-th column of H as u32 bitmask
    num_states: usize,  // 2^(n-k)
    n: usize,
    k: usize,
}
```

**Constructors** (written AFTER tests, TDD):
```rust
impl BcjrDecoder {
    /// Build from any BitMatrix H (m rows × n cols).
    pub fn new(h: &BitMatrix) -> Self;

    /// Convenience factory for dRM(32,21).
    pub fn for_drm_32_21() -> Self { Self::new(DrmCode::drm_32_21().parity_check()) }

    /// Convenience factory for eBCH(16,11).
    pub fn for_ebch_16_11() -> Self { ... }
}
```

**Core decode method**:
```rust
pub fn decode_siso(&self, combined_llrs: &[Llr]) -> SisoResult
```

Returns `SisoResult` (imported from `crate::grand::sogrand`) with:
- `app_llrs`: exact APP LLRs
- `extrinsic_llrs`: `L_APP - combined_llrs`
- `list_bler_prediction`: always `0.0` (BCJR is exact)
- `query_count`: always `0` (trellis, not query-based)

**Private helpers**:
```rust
fn forward_pass(&self, combined_llrs: &[f32]) -> Vec<Vec<f32>>;  // (n+1) × num_states
fn backward_pass(&self, combined_llrs: &[f32]) -> Vec<Vec<f32>>; // (n+1) × num_states

/// Jacobian logarithm: max*(a,b) = max(a,b) + ln(1 + exp(-|a-b|))
/// Exact for f32 precision. Returns max(a,b) when |a-b| > 8.
fn max_star(a: f32, b: f32) -> f32;

fn normalize_log_probs(buf: &mut [f32]);  // subtract max for numerical stability
```

**File**: `crates/gf2-coding/src/bcjr/mod.rs`

---

### Step 2: Write tests first (TDD)

All tests in `#[cfg(test)] mod tests` within `bcjr/mod.rs`.

**Test 1 — noiseless all-zeros** (`test_noiseless_all_zeros`):
Encode all-zero message → noiseless channel LLRs (+10.0 for bit=0). APP LLRs must all be
large positive, hard decision = all zeros. Syndrome of decoded bits = 0.

**Test 2 — noiseless known codeword** (`test_noiseless_known_codeword`):
Use a non-trivial codeword (random message, encode, LLR = ±10.0). APP hard decision must
match the codeword exactly.

**Test 3 — Hamming(7,4) cross-check** (`test_hamming74_vs_exhaustive`):
Hamming(7,4) has only 16 codewords. At moderate SNR, compare BCJR APP LLRs against an
exhaustive sum over all 16 codewords weighted by noise probability. Must agree to <0.1 LLR
units at SNR ≥ 2 dB (25 random test vectors).

**Test 4 — extrinsic is zero for noiseless** (`test_extrinsic_noiseless`):
At noiseless channel, extrinsic should be ~0 (the code adds no new info beyond the perfect
channel). Formally: `|L_ext[i]| < 1.0` for high-SNR inputs.

**Test 5 — state boundary** (`test_forward_starts_and_ends_zero`):
The forward pass must have `log_α[0][s] = -∞` for s≠0, and `log_α[n][s] = -∞` for all
s≠0 on a valid codeword.

**Test 6 — eBCH(16,11) noiseless** (`test_ebch_noiseless`):
Same as test 1 and 2 but for eBCH(16,11).

---

### Step 3: Update `crates/gf2-coding/src/lib.rs`

Add:
```rust
pub mod bcjr;
```

Re-export `BcjrDecoder` in the bcjr module's public API.

---

### Step 4: Integrate BCJR into `product/mod.rs`

**4a. Add private dispatch enum** (not public):
```rust
// Inside product/mod.rs (private)
enum SisoEngine {
    SoGrand(SoGrand),
    Bcjr(BcjrDecoder),
}
impl SisoEngine {
    fn decode_siso(&self, input: &[Llr]) -> SisoResult { ... }
    fn n(&self) -> usize { ... }
}
```

**4b. Update `TurboDecoderConfig`**:
```rust
pub struct TurboDecoderConfig {
    // existing fields unchanged ...
    pub max_iterations: usize,
    pub alpha: f32,
    pub list_size: usize,
    pub max_queries: usize,
    pub list_bler_threshold: Option<f64>,

    // NEW:
    /// Use BCJR trellis decoder instead of SOGRAND for component SISO.
    /// When true, `list_size` and `max_queries` are ignored.
    pub use_bcjr: bool,  // default: false
}
```

**4c. Update `TurboDecoder` struct**:
```rust
pub struct TurboDecoder<C: ProductComponent> {
    component: C,
    config: TurboDecoderConfig,
    siso: SisoEngine,         // replaces `sogrand: SoGrand`
    product_code: ProductCode<C>,
}
```

**4d. Update `TurboDecoder::new()`**:
```rust
let siso = if config.use_bcjr {
    SisoEngine::Bcjr(BcjrDecoder::new(component.comp_parity_check()))
} else {
    SisoEngine::SoGrand(SoGrand::new(OrbGrand::new(...)))
};
```

**4e. Update decode loop** — replace `self.sogrand.decode_siso(&input)` with
`self.siso.decode_siso(&input)` at both call sites (row step + column step).

**4f. Update `sogrand()` accessor** — keep for backward compat:
```rust
pub fn sogrand(&self) -> &SoGrand {
    match &self.siso {
        SisoEngine::SoGrand(s) => s,
        SisoEngine::Bcjr(_) => panic!("decoder configured with BCJR, not SOGRAND"),
    }
}
```

Update the one internal test that calls `decoder.sogrand().n()` to handle BCJR mode.

---

### Step 5: Update `simulation.rs` / `sim_runner.rs`

Add a BCJR flag to the product-code simulation path so campaigns can set `use_bcjr = true`
in TOML configs. Check how `TurboDecoderConfig` is constructed from campaign config and add
the new `use_bcjr` field.

---

## Success Criteria (Quantitative)

| Criterion | Measurement | Pass threshold |
|-----------|-------------|----------------|
| Unit: noiseless decode | hard decision matches codeword | 100% of test vectors |
| Unit: Hamming(7,4) cross-check | BCJR vs. exhaustive | LLR error < 0.1 for 25 vectors at 2+ dB |
| Syndrome: forward/backward | terminal state = 0 | always |
| Integration: BLER at 1.0 dB | simulation run (100 frames, min 10 errors) | BLER ≤ 0.15 (paper: 0.072, 2× target) |
| Integration: BLER at 0.75 dB | simulation run | BLER < 0.5 (currently 0.811) |
| Integration: eBCH product unchanged | eBCH product BLER at target SNR | within 5% of SOGRAND result |
| CI gate | `cargo test --workspace --all-features --release` | 0 failures, 0 warnings |
| Clippy gate | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | clean |

---

## Files to Create / Modify

| File | Action | Details |
|------|--------|---------|
| `crates/gf2-coding/src/bcjr/mod.rs` | **Create** | BcjrDecoder + tests |
| `crates/gf2-coding/src/lib.rs` | Modify | `pub mod bcjr;` + re-export |
| `crates/gf2-coding/src/product/mod.rs` | Modify | SisoEngine enum, TurboDecoderConfig.use_bcjr, dispatch |
| `crates/gf2-coding/src/simulation.rs` (or sim_runner) | Modify | Add use_bcjr campaign flag |

Key existing functions to reuse:
- `DrmCode::drm_32_21().parity_check()` → `drm.rs:parity_check()`
- `BitMatrix::col_as_bitvec(j)` → `gf2-core/src/matrix.rs:612`
- `log_sum_exp(a, b)` in `grand/orbgrand.rs:692` (f64 version; write f32 variant locally)
- `SisoResult` from `grand/sogrand.rs:86` (import, do not duplicate)
- `OrbGrand::new(h, config)` / `SoGrand::new(orbgrand)` in product/mod.rs:833-834 (keep for non-BCJR path)

---

## SIMD / GPU Acceleration Outlook

### Tier 0: auto-vectorization (free, do this by default)

The inner loop `for s in 0..NUM_STATES` iterating 2048 f32 values is a prime candidate for
auto-vectorization. Laying out `log_alpha[stage]` as a contiguous `Vec<f32>` and writing
the loop without early exits or branches lets rustc emit AVX2 automatically (8 f32/lane),
giving ~4-8x speedup on the inner pass. **No manual SIMD for the initial implementation.**

Key enabler: the `max_star(a, b)` function must be `#[inline]` and branch-free for the
auto-vectorizer to handle it. Use `f32::max(a, b) + correction` form (not if-else).

### Tier 1: butterfly SIMD (post-correctness, optional)

The XOR-permutation `s ^ h_col[i]` is structurally a butterfly network — identical to how
FFT processes its stages. For systematic H = [P^T | I_{n-k}], stages k..n each XOR with a
single bit position (unit columns), making them exact 2-point butterfly operations.

Each butterfly processes a pair `(s, s ^ 2^b)` where b is the active bit. With AVX2, 8
such pairs can be processed in a single SIMD step, giving ~16x throughput on those stages.
This is the "fast BCJR" by analogy to the fast Walsh-Hadamard transform.

Literature confirms: first-order RM codes achieve O(n log n) fast MAP via WHT-based
correlation (Berlekamp-Massey-Seroussi). Higher-order RM codes (like dRM(32,21) which
includes degree-3 monomials) don't get the full WHT speedup, but the systematic-form
H columns still yield n-k=11 pure butterfly stages out of n=32 total.

**Practical SIMD architecture** (for `gf2-kernels-simd` crate):
- `bcjr_butterfly_avx2(alpha: &mut [f32], h_col_bit: u32, log_gamma0: f32, log_gamma1: f32)`
- Processes 2048 states in 256 AVX2 butterfly operations per trellis stage
- Estimated speedup: ~10-16x over scalar inner loop
- Would live in `gf2-kernels-simd` per the unsafe-isolation invariant

### Tier 2: GPU batch decoding (long-term, simulation throughput)

For throughput-mode simulation (batches of 1024+ frames simultaneously), GPU maps well:
- One warp per BCJR decode (32 threads covering 2048/64 state groups)
- Shared memory for `log_alpha` / `log_beta` slices (each 2048 x 4 = 8KB — fits in SM)
- Custom CUDA/WGPU compute kernel with butterfly structure
- Coalesced memory access pattern: states are the "fast" dimension, trellis stages the "slow"

**When it becomes worth it**: at >10K simulation frames per SNR point. The BCJR per-decode
cost is ~130K FLOPs = trivial on GPU, so the win comes from parallelizing across frames,
not from accelerating a single decode. Typical simulation campaign runs 1K-100K frames
per SNR point — the SIMD CPU path should handle this in seconds.

**Alternative GPU path**: batch many component SISO decodes within a single turbo iteration.
A dRM(32,21) product code has 2 x 32 = 64 independent row/column SISO calls per half-iteration.
These are embarrassingly parallel and could be launched as 64 GPU thread groups. This provides
a 64x parallelism factor even within a single frame, making GPU attractive at smaller batch
sizes than the frame-level parallelism approach.

---

## Reference Papers

### Core algorithm
1. **Bahl, Cocke, Jelinek, Raviv** (1974). "Optimal Decoding of Linear Codes for Minimizing
   Symbol Error Rate." *IEEE Trans. Inform. Theory, Vol. IT-20, No. 5.* — original BCJR
   algorithm: forward-backward recursion on code trellis for exact APP computation.
2. **Wolf** (1978). "Efficient Maximum Likelihood Decoding of Linear Block Codes Using a
   Trellis." *IEEE Trans. Inform. Theory, Vol. IT-24, No. 1, pp. 76-80.* — defines how to
   build a trellis from the parity-check matrix of any linear block code.
3. **McEliece** (1996). "On the BCJR Trellis for Linear Block Codes." *IEEE Trans. Inform.
   Theory, Vol. 42, No. 4, pp. 1072-1092.* — minimal-span generator matrices, BCJR trellis
   uniquely minimizes edge count. Standard bit ordering is optimal for RM codes.

### Trellis structure for RM codes
4. **Kasami & Takata** — standard binary bit order is provably optimum for RM code trellis
   state complexity at every position. Treewidth = trelliswidth for RM codes.
5. **NASA** (1998). "Trellis representation of linear block codes." NASA Technical Report
   19980018325. — practical trellis construction and decoding algorithms.

### Numerical implementation
6. **Springer** (2013). "Reduced complexity Log-MAP decoding using the Jensen inequality."
   *EURASIP J. Wireless Commun.* — Log-MAP normalization strategies; max-log approximation
   loses ~0.2-0.5 dB at low SNR; Jacobian logarithm preserves exactness.

### Fast decoding of RM codes
7. **Ye & Abbe** (2020). "Recursive projection-aggregation decoding of Reed-Muller codes."
   *IEEE Trans. Inform. Theory.* — O(n log n) soft decoding for RM codes.
8. **Fathollahi Fard, Ivanov, Johannsen** (2021). "Fast successive-cancellation decoding
   of RM codes using FHT." *arXiv:2108.12550.* — FHT-FSC and FHT-FSCL decoders for RM.

### Target paper
9. **Yuan, Médard, Galligan & Duffy**. "Soft-output (SO) GRAND and Iterative Decoding to
   Outperform LDPCs." — paper whose Fig 1 is the alignment target. Note: the paper likely
   uses BCJR (not SOGRAND) for the n=32 component SISO in its reported results.

---

## Verification Steps

1. `cargo test -p gf2-coding --release -- bcjr` — all unit tests green
2. `cargo test --workspace --all-features --release` — full suite < 60s
3. `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean
4. Run `align_fig1.toml` campaign with `use_bcjr = true` — BLER ≤ 0.15 at 1.0 dB
5. Run `quick_fig3.toml` campaign (eBCH product code) — no regression
6. Check `jit_gate_check-all` before marking done
