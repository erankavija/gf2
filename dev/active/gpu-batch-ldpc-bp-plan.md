# Plan: HIP/ROCm GPU-Accelerated Batch LDPC BP Decoder

## Context

LDPC belief propagation (BP) decoding is the dominant computational cost in coded
simulation campaigns. Each frame requires 10-50 BP iterations, and simulation campaigns
need 10K-100K frames per SNR point. The current CPU implementation parallelizes across
frames via rayon, but the per-frame decode is serial.

**Hardware**: AMD RX 6950 XT (RDNA2, gfx1030, 80 CUs, 16GB VRAM, 128MB Infinity Cache).
ROCm 7.2, hipcc at `/opt/rocm/bin/hipcc`.

**Existing infrastructure**: `gf2-kernels-hip` crate with build.rs hipcc integration,
`DeviceBuffer` RAII wrapper, safe FFI pattern — all proven by the BCJR batch decoder.

**Goal**: Batch N LDPC frames into a single GPU kernel launch, executing BP iterations
on-device without per-iteration host round-trips. Target: normalized min-sum (NMS)
algorithm, the production default.

---

## Algorithm: LDPC BP on GPU

### BP iteration structure (per frame)

Each BP iteration has three phases:
1. **Check-node update**: For each check node m, for each connected variable n,
   compute check-to-variable message `R[m→n]` from all other `Q[n'→m]` messages.
2. **Variable-node update**: For each variable node n, compute belief and
   variable-to-check messages `Q[n→m]` from channel LLR + all other `R[m'→n]`.
3. **Optional syndrome check**: Hard-decode beliefs, compute `H × ĉ`, stop if zero.

### Data layout for GPU

The key challenge is that LDPC codes have irregular structure — variable and check
degrees vary. We need a GPU-friendly representation of the Tanner graph.

**Edge-based storage** (proven pattern from GPU LDPC literature):

```
// Tanner graph edges, sorted by check node
edges_by_check: [Edge; nnz]     // Each edge = (var_idx, edge_position)
check_offsets:  [u32; m+1]      // CSR-style: edges for check m at [check_offsets[m]..check_offsets[m+1])

// Same edges, sorted by variable node
edges_by_var:   [Edge; nnz]     // Each edge = (check_idx, edge_position)
var_offsets:    [u32; n+1]      // CSR-style: edges for var n at [var_offsets[n]..var_offsets[n+1])

// Messages (one f32 per edge, per direction)
R_messages:     [f32; nnz]      // Check-to-variable (indexed by edges_by_check order)
Q_messages:     [f32; nnz]      // Variable-to-check (indexed by edges_by_var order)

// Per-variable beliefs
beliefs:        [f32; n]        // Current posterior LLR per variable node
channel_llrs:   [f32; n]        // Input channel LLRs (constant during iteration)
```

**Cross-referencing**: Each edge in `edges_by_check` stores a `mate_idx` pointing to
the same edge's position in `edges_by_var` (and vice versa). This allows O(1) lookup
between the two orderings without searching.

### Thread mapping

**Check-node kernel**: One thread per check node.
- Thread m reads Q messages from `edges_by_check[check_offsets[m]..check_offsets[m+1]]`
- For each edge position p: compute R[m→n_p] = NMS of all Q messages except position p
- Write R messages to `R_messages[check_offsets[m]+p]`
- **Complexity**: O(d_c) per thread, where d_c = check degree (typically 3-8 for LDPC)
- **Optimization**: Two-minimum technique — track min1, min2, sign_product in one pass,
  then each exclusion is O(1):
  ```
  R[m→n_p] = alpha * sign_product/sign(Q[n_p→m]) * (p == min1_pos ? min2 : min1)
  ```

**Variable-node kernel**: One thread per variable node.
- Thread n reads R messages from `edges_by_var[var_offsets[n]..var_offsets[n+1]]`
  via mate_idx cross-references
- Computes belief = channel_llr[n] + sum(R messages)
- For each edge position p: Q[n→m_p] = belief - R[m_p→n]
- **Complexity**: O(d_v) per thread, where d_v = variable degree (typically 2-6)

**Syndrome-check kernel**: Parallel reduction over check nodes.
- Each thread handles one check: XOR hard decisions of connected variables
- Block-level OR reduction to detect any unsatisfied check
- Single atomic flag set if syndrome != 0

### Batching strategy

**Grid dimensions**:
- `gridDim.x = batch_size` (one frame per x-block)
- `gridDim.y = ceil(num_nodes / blockDim.x)` (partition nodes across y-blocks)
- `blockDim.x = 256` (threads per block, tunable)

All per-frame arrays are offset by `batch_idx * array_size`. The Tanner graph structure
(edges, offsets) is shared across all frames in the batch since they use the same code.

### Iteration loop

Two options:

**Option A — Host-driven iterations** (simpler, chosen for prototype):
```
for iter in 0..max_iterations:
    launch check_node_kernel(batch)
    launch var_node_kernel(batch)
    if early_termination:
        launch syndrome_kernel(batch)
        sync + read converged flags
        if all_converged: break
```
Kernel launch overhead ~5μs × 2-3 kernels × 50 iterations = ~500μs-750μs.
For batch_size=64, this is amortized across 64 frames.

**Option B — Device-side loop** (advanced, future work):
Single persistent kernel with grid-level sync. Avoids launch overhead but requires
cooperative groups and is harder to debug.

### Memory budget

For a 5G NR LDPC BG2 (n=1024, k=441, m=583, nnz≈1750):

| Array | Per-frame | Batch=64 | Location |
|-------|-----------|----------|----------|
| channel_llrs | 4 KB | 256 KB | Global |
| beliefs | 4 KB | 256 KB | Global |
| R_messages | 7 KB | 448 KB | Global |
| Q_messages | 7 KB | 448 KB | Global |
| **Total per-frame** | **22 KB** | **1.4 MB** | |
| Tanner graph | ~28 KB | 28 KB (shared) | Constant/Global |

For DVB-T2 short (n=16200, m=9000, nnz≈27000):

| Array | Per-frame | Batch=64 | Location |
|-------|-----------|----------|----------|
| channel_llrs | 63 KB | 4 MB | Global |
| beliefs | 63 KB | 4 MB | Global |
| R_messages | 105 KB | 6.7 MB | Global |
| Q_messages | 105 KB | 6.7 MB | Global |
| **Total per-frame** | **336 KB** | **21.4 MB** | |
| Tanner graph | ~432 KB | 432 KB (shared) | Global |

Both well within 16GB VRAM. Infinity Cache (128MB) can hold the entire working set
for moderate codes.

### NMS check-node update — two-minimum technique

The normalized min-sum check-node update for check m with degree d_c:

```hip
__global__ void check_node_update_nms(
    const float* Q_messages,       // [batch × nnz]
    float* R_messages,             // [batch × nnz]
    const uint32_t* check_offsets, // [m+1]
    const uint32_t* mate_idx,      // [nnz] — Q index for each R edge
    int m_checks, int nnz, float alpha
) {
    int batch_idx = blockIdx.x;
    int check = blockIdx.y * blockDim.x + threadIdx.x;
    if (check >= m_checks) return;

    int start = check_offsets[check];
    int end   = check_offsets[check + 1];
    int degree = end - start;

    // Pass 1: find min1, min2, sign_product, min1_pos
    float min1 = INFINITY, min2 = INFINITY;
    int min1_pos = 0;
    int sign_product = 0;  // 0 = positive, 1 = negative (XOR of sign bits)

    for (int p = 0; p < degree; p++) {
        float q = Q_messages[batch_idx * nnz + mate_idx[start + p]];
        float abs_q = fabsf(q);
        int sign = (q < 0.0f) ? 1 : 0;
        sign_product ^= sign;

        if (abs_q < min1) {
            min2 = min1; min1 = abs_q; min1_pos = p;
        } else if (abs_q < min2) {
            min2 = abs_q;
        }
    }

    // Pass 2: write R messages
    for (int p = 0; p < degree; p++) {
        float q = Q_messages[batch_idx * nnz + mate_idx[start + p]];
        int sign = (q < 0.0f) ? 1 : 0;
        int result_sign = sign_product ^ sign;  // Exclude this edge's sign
        float magnitude = (p == min1_pos) ? min2 : min1;
        float r = alpha * magnitude;
        R_messages[batch_idx * nnz + start + p] = result_sign ? -r : r;
    }
}
```

---

## Implementation Plan

### Step 1: GPU Tanner graph representation

Create `GpuLdpcGraph` struct that converts `LdpcCode` (SpBitMatrixDual) into
GPU-friendly edge-based format with cross-referencing mate indices.

```rust
pub struct GpuLdpcGraph {
    // Host-side (for upload)
    check_offsets: Vec<u32>,    // CSR offsets for check nodes
    var_offsets: Vec<u32>,      // CSR offsets for variable nodes
    check_edges: Vec<u32>,      // Variable indices per check edge
    var_edges: Vec<u32>,        // Check indices per variable edge
    mate_c2v: Vec<u32>,         // For each check-edge: index in var-edge array
    mate_v2c: Vec<u32>,         // For each var-edge: index in check-edge array
    n: usize,
    m: usize,
    nnz: usize,
}
```

### Step 2: HIP kernels (`hip/ldpc_bp_kernel.hip`)

Three kernels:
1. `check_node_update_nms_kernel` — NMS with two-minimum technique
2. `var_node_update_kernel` — belief accumulation + Q message computation
3. `syndrome_check_kernel` — batched hard-decision + syndrome verification

Host wrapper: `launch_ldpc_bp_batch(...)` runs the iteration loop.

### Step 3: Safe Rust wrapper

```rust
pub struct GpuLdpcBpBatch {
    graph: GpuLdpcGraph,           // Tanner graph (persistent, uploaded once)
    d_check_offsets: DeviceBuffer<u32>,
    d_var_offsets: DeviceBuffer<u32>,
    d_mate_c2v: DeviceBuffer<u32>,
    d_mate_v2c: DeviceBuffer<u32>,
    d_channel_llrs: DeviceBuffer<f32>,
    d_beliefs: DeviceBuffer<f32>,
    d_r_messages: DeviceBuffer<f32>,
    d_q_messages: DeviceBuffer<f32>,
    d_converged: DeviceBuffer<u32>,
    max_batch: usize,
    n: usize,
    m: usize,
    nnz: usize,
}

impl GpuLdpcBpBatch {
    pub fn new(code: &LdpcCode, max_batch: usize) -> Result<Self, HipError>;
    pub fn decode_batch(
        &self,
        channel_llrs: &[Vec<f32>],
        max_iterations: usize,
        alpha: f32,
    ) -> Result<Vec<GpuBpResult>, HipError>;
}

pub struct GpuBpResult {
    pub beliefs: Vec<f32>,
    pub converged: bool,
    pub iterations: usize,
}
```

### Step 4: Feature-gate in `gf2-coding`

Add `GpuBp` variant to decoder dispatch. The `LdpcDecoder` gains:
```rust
pub fn decode_batch_gpu(
    code: &LdpcCode,
    llr_blocks: &[&[Llr]],
    max_iterations: usize,
    config: &DecoderConfig,
) -> Vec<DecoderResult>
```

### Step 5: Integration with simulation harness

Wire `decode_batch_gpu` into the simulation runner when `hip` feature is active
and batch_size >= threshold.

---

## Verification

### Unit tests (in `gf2-kernels-hip`)

1. **Tanner graph construction**: Verify `GpuLdpcGraph` edge counts, offsets, and
   mate indices match `SpBitMatrixDual` row/col iterators for Hamming(7,4).
2. **GPU NMS matches CPU NMS**: For Hamming(7,4), 25 test vectors, GPU beliefs
   match CPU `LdpcDecoder` (MinSum) within 0.05 LLR after 10 iterations.
3. **GPU NMS matches CPU NMS (5G NR)**: For BG2 (256,121), 10 noiseless test vectors,
   GPU hard decisions match CPU hard decisions exactly.
4. **Batch correctness**: `decode_batch_gpu(64 inputs)` matches 64 individual CPU
   `decode_iterative()` calls (hard decisions identical, beliefs within 0.1 LLR).
5. **Convergence equivalence**: GPU and CPU converge on the same iteration for
   10 high-SNR frames (iteration count must match exactly).
6. **Early termination**: GPU with early termination uses fewer iterations than
   max_iterations on high-SNR noiseless input.

### Cross-check tests (in `gf2-kernels-hip/tests/`)

7. **DVB-T2 short Rate1/2 cross-check**: 10 frames, GPU vs CPU NMS(0.875),
   hard decisions identical, beliefs within 0.1 LLR.
8. **Throughput benchmark**: GPU batch-64 decode of BG2(1024,441) is at least
   3x faster than 64 serial CPU decodes (measured wall-clock).

### Property-based tests

9. **prop_gpu_cpu_equivalence**: For random LLRs on Hamming(7,4), GPU and CPU
   produce identical hard decisions after 10 NMS iterations.
10. **prop_batch_matches_serial**: Random batch size 1-16, random LLRs on
    BG2(256,121), GPU batch matches CPU serial (hard decisions).

---

## Expected Performance

| Code | Batch | CPU (serial) | GPU (est.) | Speedup |
|------|-------|-------------|------------|---------|
| BG2(256,121) | 64 | 64 × 0.3ms = 19ms | ~2ms | ~10x |
| BG2(1024,441) | 64 | 64 × 1.5ms = 96ms | ~8ms | ~12x |
| DVB-T2 short (16200) | 64 | 64 × 15ms = 960ms | ~40ms | ~24x |
| DVB-T2 normal (64800) | 16 | 16 × 80ms = 1.28s | ~80ms | ~16x |

GPU advantage increases with code size (more parallelism per frame) and batch size
(amortizes kernel launch overhead).

---

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Irregular check degrees → warp divergence | Two-minimum technique ensures uniform work per thread regardless of exclusion position |
| Mate-index indirection → random memory access | Edges sorted within each node; sequential access within a node's edges. Infinity Cache helps |
| Host-driven iteration loop → launch overhead | For 50 iterations × 3 kernels: ~750μs overhead, small vs compute for batch≥16 |
| f32 reduction ordering CPU≠GPU | Tolerance-based comparison (< 0.1 LLR), hard-decision exact match |
| Large DVB-T2 codes → high memory | 21MB for batch-64 DVB-T2 short — trivial for 16GB VRAM |

---

## References

- Richardson & Urbanke, "The capacity of low-density parity-check codes under message-passing decoding," IEEE Trans. IT, 2001
- Chen et al., "Reduced-complexity decoding of LDPC codes," IEEE Trans. Comm., 2005 (NMS/OMS)
- GPU LDPC decoding surveys: Wang et al., "GPU implementation of LDPC decoder," 2013
- arxiv:2508.07879 — GPU syndrome decoding for quantum LDPC codes (sub-63μs)
