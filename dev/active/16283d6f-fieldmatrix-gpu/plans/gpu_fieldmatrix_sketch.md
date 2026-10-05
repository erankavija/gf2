# GPU-accelerated `FieldMatrix` — pre-filed sketch

> Status: **idea / backlog**. Not scoped for dispatch. Filed so the roadmap shows it explicitly.
>
> Prerequisite: epic `bb85c68a` complete, particularly story `64c88ae4` (CPU benchmark against fflas-ffpack + M4RI). We need a measured baseline before we try to beat it.
>
> Target hardware: HIP/ROCm on gfx1030 (same as the existing `gf2-kernels-hip` prototype). NVIDIA via HIP's portability layer is a stretch, not a commitment.

## 1. Why this is a separate epic

1. **Different algorithmic envelope.** GPU wins on throughput-bounded dense kernels (`gemm`, `SpMV`, batch inversion); factorizations (PLE, `trsm`) need pipelined launches and are historically where GPU libraries over finite fields either don't exist or lag. Mixing "easy gemm" with "hard factorization" inside one epic muddies scope.
2. **Different correctness/bench harness.** Device kernels need device-side property tests, round-trip-to-CPU equivalence harnesses, and a separate container image (ROCm, not OpenBLAS).
3. **Non-default build posture.** `gf2-kernels-hip` is already excluded from the default workspace. That stays — non-ROCm hosts must keep building cleanly. The CPU `FieldMatrix` epic must not be gated on GPU toolchain availability.

## 2. Scope — what this epic ships

**In scope:**
- Device-side field arithmetic for the fields we care about most: `Fp<P>` with 32-bit and 64-bit `P`; `Gf2_8`, `Gf2_16`, `Gf2_32`.
- `DFieldMatrix<F>` — device-resident row-major matrix, host ↔ device transfer, conversion to/from `FieldMatrix<F>`.
- Device matrix multiplication (`dgemm` equivalent) with a realistic crossover against CPU matmul.
- Device SpMV for `DSparseFieldMatrix<F>` (CSR).
- Benchmark vs. `hipBLAS` (where applicable) and the CPU baseline from `64c88ae4`.
- Correctness: cross-check every device kernel against its CPU counterpart on identical randomized inputs.

**Out of scope (defer to a future epic):**
- Device PLE / `trsm` / inv / solve — factorizations on GPU. These justify their own epic once we have device matmul + SpMV working.
- Device characteristic / minimal polynomial.
- NVIDIA support beyond what HIP's portability layer gives for free.
- Unified-memory (HMM / ATS) paths — we do explicit copies.

## 3. Architecture sketch

### Storage + transfer model

```rust
// New crate: gf2-core-hip (depends on gf2-core and gf2-kernels-hip).
pub struct DFieldMatrix<F: FiniteField> {
    device_ptr: hip::DevicePtr<F::DeviceRepr>,
    rows: usize,
    cols: usize,
    stream: hip::Stream,
}

impl<F: FiniteField> DFieldMatrix<F> {
    pub fn to_device(host: &FieldMatrix<F>) -> Self;          // explicit H2D
    pub fn to_host(&self) -> FieldMatrix<F>;                   // explicit D2H
    pub fn zeros(rows: usize, cols: usize) -> Self;            // allocate on device
}
```

Explicit transfers. No hidden copies. A host-side `FieldMatrix` never silently becomes a device matrix; the user calls `to_device()`. This is unglamorous but keeps latency understandable.

### Field representation on device

Each field declares a device representation via an associated type:

```rust
pub trait FieldDevice: FiniteField {
    type DeviceRepr: Copy;                                     // what sits in GPU memory
    fn host_to_device(h: Self) -> Self::DeviceRepr;
    fn device_to_host(d: Self::DeviceRepr) -> Self;
    const DEVICE_KERNELS: &'static DeviceKernelVtable<Self>;   // mul / add / reduce
}
```

Montgomery multiplication for `Fp<P>` on device mirrors the CPU Montgomery path (same `P_INV`, same reduction step), just rewritten as a HIP kernel. GF(2^m) uses warp-shuffled polynomial multiplication; falls back to table lookup at m ≤ 16.

### Kernel dispatch

Sibling trait to the CPU `FieldBackend`:

```rust
pub trait FieldGpuBackend<F: FieldDevice> {
    fn gemm(alpha: F, a: &DFieldMatrix<F>, b: &DFieldMatrix<F>,
            beta: F, c: &mut DFieldMatrix<F>);
    fn spmv(a: &DSparseFieldMatrix<F>, x: &DFieldVec<F>, y: &mut DFieldVec<F>);
    // ...
}
```

The existing `FieldBackend` (CPU SIMD dispatch) is untouched. `gf2-core-hip` provides the GPU-side implementations and surfaces them as `From<FieldMatrix<F>>` conveniences.

### Crossover thresholds

GPU only wins past a size floor that amortizes launch + transfer overhead. Expect n ≥ 512 for matmul, n ≥ 4096 for SpMV to be the right order of magnitude — measured empirically, not assumed. The `to_device()` / `to_host()` boundary is where users choose explicitly; the library never auto-dispatches.

## 4. Tentative story breakdown (to be refined at breakdown time)

| # | Story | Depends on | Notes |
|---|---|---|---|
| 1 | Design `FieldDevice` trait + `DFieldMatrix` memory model | (CPU epic bb85c68a done) | Design-only. Produces a companion to `dev/plans/gpu_fieldmatrix_sketch.md`. |
| 2 | Device `Fp<P>` Montgomery arithmetic + cross-check vs. CPU | #1 | HIP kernels + equivalence test harness. |
| 3 | Device GF(2^m) carry-less multiplication + cross-check | #1 | Same pattern; m ∈ {8, 16, 32}. |
| 4 | `DFieldMatrix<F>` storage + H2D/D2H transfer | #2, #3 | |
| 5 | Device matmul (`dgemm`) + criterion bench | #4 | Target: beat CPU above a measured threshold; compare vs. hipBLAS where comparable. |
| 6 | Device SpMV (`CSR`) + criterion bench | #4 | |
| 7 | Extended benchmark harness (GPU lane) | #5, #6, `64c88ae4` | Reuses the container methodology from `64c88ae4`, with a ROCm layer. Publishes CPU + GPU numbers side-by-side. |

Probably 7 stories, ~3 waves. Not finalized — breakdown comes later, once the CPU epic is close to landing.

## 5. Open decisions flagged for breakdown time

- **hipBLAS as reference?** `hipBLAS` is over `double`, not finite fields. Comparison is apples-to-oranges in the same way CPU float BLAS vs. `fflas-ffpack` was — but it is the only off-the-shelf device gemm baseline.
- **Target arch set.** Prototype stays gfx1030. Widening to gfx1100 / RDNA3 or NVIDIA needs explicit user approval.
- **RNS on device.** Large primes (> 2^64) would need RNS + CRT on device, which the paper (§1.3) already describes for CPU. Almost certainly out of scope for the first GPU epic.
- **Feature gate posture.** Everything GPU stays behind `--features hip`; default builds on non-ROCm hosts are unaffected.

## 6. Explicit non-goals

- **No factorizations on GPU.** No PLE, no `trsm`, no inv, no solve on device. Future epic.
- **No eigenvalues, no characteristic polynomial on device.** Same reason.
- **No abandonment of CPU parity.** Every device kernel has a CPU equivalent it cross-checks against; we do not ship device-only algorithms.

## 7. Success criteria (epic-level, preliminary)

- [hard] Every device kernel passes equivalence tests against its CPU counterpart on randomized inputs across ≥ 3 fields.
- [hard] `DFieldMatrix` H2D + D2H is round-trip lossless.
- [hard] Device matmul beats CPU matmul past some measured size floor, documented in the benchmark publication.
- [hard] Default workspace build (no `hip` feature) remains identical to today — non-ROCm hosts unaffected.
- [aspirational] Within 2× of `hipBLAS` on `double`-equivalent throughput for `Fp<P>` matmul at n ≥ 1024.
- [aspirational] Exceed CPU matmul throughput (GF(p), n = 2048) by ≥ 5×.
