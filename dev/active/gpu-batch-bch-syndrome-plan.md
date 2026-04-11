# Plan: HIP/ROCm GPU-Accelerated Batch BCH Syndrome Evaluation

## Context

BCH syndrome evaluation is the first and most parallelizable step of BCH decoding.
For each received codeword, the decoder evaluates the received polynomial r(x) at
2t points α, α², ..., α^(2t) in GF(2^m). This produces 2t syndrome values that
feed into Berlekamp-Massey for error-locator polynomial computation.

**Current implementation**: `BchCode::compute_syndromes()` evaluates r(x) at each
point sequentially using Horner's method. `eval_batch()` is a serial loop.
`decode_batch()` parallelizes across codewords via rayon but each codeword's 2t
syndrome evaluations are serial.

**Key observation**: Syndrome evaluation is embarrassingly parallel along two axes:
1. **Cross-codeword**: N codewords are independent
2. **Within-codeword**: 2t evaluation points are independent

This gives N × 2t independent Horner evaluations, each requiring n GF(2^m)
multiply-accumulate steps. For DVB-T2 Normal (t=12, n=32400): 24 × 32400 = 777,600
field operations per codeword, perfectly suited for GPU.

**Hardware**: AMD RX 6950 XT (RDNA2, gfx1030, 80 CUs, 16GB VRAM).
ROCm 7.2, hipcc at `/opt/rocm/bin/hipcc`.

**Existing infrastructure**: `gf2-kernels-hip` crate with hipcc build, DeviceBuffer,
FFI pattern.

---

## Algorithm: GF(2^m) Arithmetic on GPU

### Field representation

GF(2^m) elements are m-bit integers. For DVB-T2:
- **Short frames**: m=14, primitive poly `0x4035` (x^14 + x^5 + x^3 + x + 1)
- **Normal frames**: m=16, primitive poly `0x1002D` (x^16 + x^5 + x^3 + x^2 + 1)

Elements fit in a single `uint32_t`. Addition is XOR.

### Multiplication strategies for GPU

**Option A — Log/exp table lookup** (chosen for m ≤ 16):
```c
// Tables in constant/shared memory
__constant__ uint16_t log_table[1 << M];  // log_table[a] = i where α^i = a
__constant__ uint16_t exp_table[1 << M];  // exp_table[i] = α^i

__device__ uint32_t gf_mul(uint32_t a, uint32_t b) {
    if (a == 0 || b == 0) return 0;
    uint32_t order = (1u << M) - 1;
    uint32_t log_sum = (uint32_t)log_table[a] + (uint32_t)log_table[b];
    if (log_sum >= order) log_sum -= order;  // Modular reduction without division
    return (uint32_t)exp_table[log_sum];
}
```

**Memory**: For m=16: 2 × 64K × 2 bytes = 256 KB. Fits in constant memory (64 KB limit
per array on RDNA2) only for m ≤ 15. For m=16, use shared memory or global + L2 cache.

**Revised for m=16**: Upload tables to global memory. The 128 MB Infinity Cache on
RX 6950 XT will cache both tables (256 KB) after the first access — effectively constant
memory speed for repeated lookups.

**Option B — Schoolbook multiply** (fallback for m > 16 or no tables):
```c
__device__ uint32_t gf_mul_schoolbook(uint32_t a, uint32_t b, int m, uint32_t prim_poly) {
    uint32_t result = 0;
    for (int i = 0; i < m; i++) {
        if (b & (1u << i)) result ^= a;
        uint32_t overflow = a & (1u << (m - 1));
        a <<= 1;
        if (overflow) a ^= prim_poly;
    }
    return result & ((1u << m) - 1);
}
```
O(m) bit operations per multiply. No memory access. Good for registers-rich GPU
but m=16 means 16 iterations with branch divergence.

**Decision**: Use log/exp tables for m ≤ 16 (covers all DVB-T2 codes). The table
approach reduces multiplication to 3 memory lookups + 1 comparison + 1 addition,
which is far more GPU-friendly than 16 iterations of schoolbook with branches.

### Horner evaluation on GPU

Each thread evaluates r(x) at one point α^j for one codeword:

```c
__device__ uint32_t horner_eval(
    const uint32_t* coeffs,  // Polynomial coefficients [n]
    int n,                    // Degree + 1
    uint32_t x,              // Evaluation point α^j
    const uint16_t* log_tbl,
    const uint16_t* exp_tbl,
    uint32_t order            // 2^m - 1
) {
    uint32_t result = coeffs[n - 1];
    for (int i = n - 2; i >= 0; i--) {
        result = gf_mul_log(result, x, log_tbl, exp_tbl, order);
        result ^= coeffs[i];  // GF(2^m) addition = XOR
    }
    return result;
}
```

### Thread mapping

**One thread per (codeword, syndrome_index)**:
- `blockIdx.x` = codeword index in batch
- `threadIdx.x` = syndrome index (0..2t-1)
- Block size = 32 (one wavefront, since 2t ≤ 24 typically)

For batch=128, t=12: 128 × 24 = 3072 threads = 96 wavefronts → fills ~1.2 CUs.
Under-utilization is acceptable because each thread does n=32400 multiply-accumulate
steps — the per-thread work is enormous.

**Alternative mapping for higher occupancy** (for large n):
- Partition the Horner evaluation across multiple threads using parallel prefix
- Not worth the complexity for this prototype — single-thread Horner is simple
  and the inner loop is fully sequential anyway

### Input format

BCH codewords are `BitVec` (binary). We need GF(2^m) polynomial coefficients.
For a systematic BCH code, the received word r = [message | parity] is interpreted
as a polynomial r(x) = r_{n-1} x^{n-1} + ... + r_1 x + r_0 where each r_i ∈ {0, 1}
(embedded in GF(2^m) as 0 or 1).

**Conversion**: Pack bits into u32 array where each element is 0 or 1. This is
trivial — just extract each bit of the BitVec.

### Evaluation points

The evaluation points α^1, α^2, ..., α^{2t} are precomputed on the host and
uploaded as a constant array. Each is an m-bit GF(2^m) element (u32).

---

## Implementation Plan

### Step 1: GF(2^m) GPU tables

Extract log/exp tables from `Gf2mField` and upload to device memory.

```rust
pub struct GpuGf2mTables {
    d_log_table: DeviceBuffer<u16>,   // [2^m]
    d_exp_table: DeviceBuffer<u16>,   // [2^m]
    m: usize,
    order: u32,  // 2^m - 1
}

impl GpuGf2mTables {
    pub fn from_field(field: &Gf2mField) -> Result<Self, HipError>;
}
```

### Step 2: HIP kernel (`hip/bch_syndrome_kernel.hip`)

```c
extern "C" __global__ void bch_syndrome_batch_kernel(
    const uint32_t* __restrict__ received,    // [batch × n], each element 0 or 1
    const uint32_t* __restrict__ eval_points, // [2t], α^1 through α^(2t)
    uint32_t* __restrict__ syndromes,         // [batch × 2t], output
    const uint16_t* __restrict__ log_table,   // [2^m]
    const uint16_t* __restrict__ exp_table,   // [2^m]
    int n,                                     // Codeword length
    int two_t,                                // Number of syndromes (2t)
    uint32_t order                            // 2^m - 1
);
```

Host wrapper: `launch_bch_syndrome_batch(...)`.

### Step 3: Safe Rust wrapper

```rust
pub struct GpuBchSyndromeBatch {
    tables: GpuGf2mTables,
    d_eval_points: DeviceBuffer<u32>,  // [2t]
    d_received: DeviceBuffer<u32>,     // [max_batch × n]
    d_syndromes: DeviceBuffer<u32>,    // [max_batch × 2t]
    n: usize,
    two_t: usize,
    max_batch: usize,
}

impl GpuBchSyndromeBatch {
    pub fn new(code: &BchCode, max_batch: usize) -> Result<Self, HipError>;
    pub fn compute_syndromes_batch(
        &self,
        received: &[&BitVec],
    ) -> Result<Vec<Vec<Gf2mElement>>, HipError>;
}
```

### Step 4: Integration into BchDecoder

Feature-gated path in `decode_batch`:
```rust
#[cfg(feature = "hip")]
if batch.len() >= GPU_SYNDROME_THRESHOLD {
    let syndromes = self.gpu_syndrome.compute_syndromes_batch(&received);
    // Continue with Berlekamp-Massey on CPU (sequential, O(t²) per codeword)
    // GPU syndrome evaluation is the bottleneck; BM is fast
}
```

**Rationale for CPU Berlekamp-Massey**: BM is O(t²) per codeword with t ≤ 12.
That's ~144 field operations — negligible compared to syndrome evaluation's
n × 2t = 777,600 operations. GPU-porting BM would add complexity for no benefit.

### Step 5: Chien search (optional, future)

Chien search (error location) is also parallelizable: evaluate Λ(x) at all α^i
for i=0..n-1. This is n independent evaluations of a degree-t polynomial.
Same pattern as syndrome evaluation but with a shorter polynomial.

For this prototype, Chien search stays on CPU — it's O(n × t) which is smaller
than syndrome evaluation's O(n × 2t) and has lower constant factor.

---

## Memory Budget

For DVB-T2 Normal (n=32400, m=16, t=12):

| Array | Size | Location |
|-------|------|----------|
| log_table | 128 KB | Global (Infinity Cache) |
| exp_table | 128 KB | Global (Infinity Cache) |
| eval_points | 96 B | Constant |
| received (batch=128) | 128 × 32400 × 4 = 15.8 MB | Global |
| syndromes (batch=128) | 128 × 24 × 4 = 12 KB | Global |
| **Total** | **~16.1 MB** | |

Well within 16GB VRAM. The received array dominates because we expand bits to u32.

**Optimization opportunity** (future): Pack 32 bits per u32 word and have threads
extract bits. Reduces received array by 32x but adds bit-extraction overhead.
Not worth it for prototype — the current layout ensures coalesced global memory reads.

---

## Verification

### Unit tests (in `gf2-kernels-hip`)

1. **GF(2^m) multiply correctness**: For m=4 (GF(16)), verify GPU `gf_mul` matches
   CPU `Gf2mField::mul` for all 16×16 = 256 element pairs (exhaustive).

2. **Horner evaluation correctness**: For m=4, evaluate a known polynomial at all
   15 nonzero field elements on GPU. Compare against CPU `Gf2mPoly::eval()`.
   Must match exactly (no floating point — pure integer GF arithmetic).

3. **Single-codeword syndrome**: For BCH(15,7,2) over GF(2^4), compute syndromes
   of 10 known codewords on GPU. Compare against CPU `compute_syndromes()`.
   Must match exactly (integer results).

4. **DVB-T2 short syndrome**: For BCH(7200,7032,12) over GF(2^14), compute
   syndromes of 10 codewords (5 error-free + 5 with injected errors) on GPU.
   Must match CPU exactly.

5. **DVB-T2 normal syndrome**: For BCH(32400,32208,12) over GF(2^16), compute
   syndromes of 10 codewords on GPU. Must match CPU exactly.

6. **Batch correctness**: `compute_syndromes_batch(128 codewords)` produces
   identical results to 128 individual CPU `compute_syndromes()` calls.

7. **Zero-syndrome property**: For valid codewords (no errors), all 2t syndromes
   must be zero. Verify on GPU for 20 valid DVB-T2 short codewords.

8. **Error-syndrome property**: For codewords with 1..t injected errors, at least
   one syndrome must be nonzero. Verify on GPU for 20 corrupted codewords.

### Cross-check tests (in `gf2-kernels-hip/tests/`)

9. **Full decode pipeline**: GPU syndromes fed to CPU Berlekamp-Massey + Chien
   search produces correct decoded codewords for 20 codewords with 1..5 errors.
   Decoded result must match CPU-only pipeline exactly.

10. **Throughput benchmark**: GPU batch-128 syndrome evaluation of DVB-T2 short
    BCH(7200,7032,12) is at least 5x faster than 128 serial CPU evaluations.

### Property-based tests

11. **prop_gpu_cpu_syndrome_match**: For random received words on BCH(15,7,2),
    GPU and CPU syndromes match exactly.

12. **prop_batch_size_invariant**: For random batch sizes 1-32 on BCH(15,7,2),
    GPU batch result matches CPU serial result exactly.

---

## Expected Performance

| Code | Batch | CPU (serial) | GPU (est.) | Speedup |
|------|-------|-------------|------------|---------|
| BCH(15,7,2) GF(16) | 128 | 128 × 1μs = 128μs | ~20μs | ~6x |
| BCH(7200,7032,12) GF(2^14) | 128 | 128 × 0.8ms = 102ms | ~4ms | ~25x |
| BCH(32400,32208,12) GF(2^16) | 128 | 128 × 4ms = 512ms | ~12ms | ~43x |

GPU advantage scales with n (longer Horner loops = more work per thread) and
batch size. The DVB-T2 normal case is the strongest candidate: each thread
does 32400 multiply-accumulate steps, fully hiding memory latency.

---

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Log/exp tables don't fit constant memory for m=16 | Use global memory + Infinity Cache; 256KB tables cached after first batch |
| Low occupancy (24 threads per block) | Each thread does O(n) work; GPU utilization comes from per-thread compute, not thread count |
| Bit-to-u32 expansion wastes bandwidth | Accepted for prototype; future optimization can use bit-packed format with extraction |
| Integer-only arithmetic → no GPU float units used | GF(2^m) arithmetic is inherently integer; GPU INT32 units are still parallel. RDNA2 has dedicated INT32 pipes |
| BM stays on CPU → pipeline stall | BM is O(t²) ≈ 144 ops vs syndrome O(n×2t) ≈ 778K ops. BM is <0.02% of total decode time |

---

## References

- Berlekamp, "Algebraic Coding Theory," 1968
- Lin & Costello, "Error Control Coding," 2nd ed., Ch. 6 (BCH decoding)
- DVB-T2 ETSI EN 302 755, Annex C (BCH/LDPC concatenated coding)
- GPU GF arithmetic surveys in coding theory / cryptography literature
