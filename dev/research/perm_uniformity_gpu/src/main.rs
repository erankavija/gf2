// perm-uniformity-gpu: GPU-accelerated high-N resample of the perm(A)-vs-det(A)
// uniformity experiment over GF(q) for q in {3, 5, 7}.
//
// JIT issue b293af5a.  Follow-up to / supersedes the noise-limited cells of
// 8e4e19a0 (q=3 n in {24,28,32}, plus F_5/F_7 extended past n<=14).
//
// What is reused (no re-implementation):
//   * perm_uniformity::harness::{tvd_from_counts, bootstrap_tvd_ci,
//     bootstrap_diff_ci, CellResult}  -- the 8e4e19a0 SSOT statistics.
//   * perm_uniformity::png::write_png_file               -- the PNG encoder.
//   * gf2_core::field::inverse::det                      -- canonical det.
//   * gf2_algebra::gpu::permanent_batch_bipedal{3,5,7}   -- GPU permanent.
//   * gf2_algebra::testutil::random_matrix_with_rng      -- seed-pinned draws.
//
// The ONLY new logic here is the GPU-batched sampling loop: for each cell we
// draw N independent seeded random matrices *in the same seed->matrix order
// the CPU 8e4e19a0 harness uses* (one Lcg(perm_seed); per sample
// random_matrix_with_rng::<P>(n) -- element draw rng.next_u64() % P, byte for
// byte the 8e4e19a0 perm closure), buffer them, push them through the GPU
// batch permanent in fixed-size chunks, and compute det on the CPU per matrix
// from an independent Lcg(det_seed) stream (again mirroring the 8e4e19a0
// det closure exactly).  Both u8 sample streams are then fed into the reused
// bootstrap functions with the identical per-cell seeds the CPU harness uses,
// so the statistical columns are produced by exactly the same code path.
//
// GPU batch order does NOT perturb the per-sample seed->matrix mapping: all N
// matrices for a cell are generated in strict seed order *before* any chunking,
// so the i-th matrix is identical regardless of chunk size.
//
// Output: dev/benchmarks/perm_uniformity/results-2026-05-17-gpu.csv  (the
// exact 8e4e19a0 column schema, with a hardware-fingerprint + seed `#` header;
// does NOT overwrite the committed CPU results-2026-05-15.csv).
//
// Without --features hip the binary prints a message and exits non-zero so the
// crate stays buildable on non-ROCm hosts (permanent_gpu_crossover precedent).

#[cfg(not(feature = "hip"))]
fn main() {
    eprintln!(
        "perm_uniformity_gpu: this binary requires the `hip` feature.\n\
         Build/run with: cargo run \
         --manifest-path dev/research/perm_uniformity_gpu/Cargo.toml \
         --release --features hip\n\
         (ROCm + gfx1030 device required at runtime)"
    );
    std::process::exit(1);
}

#[cfg(feature = "hip")]
fn main() {
    harness::run();
}

// ---------------------------------------------------------------------------
// HIP-gated harness body
// ---------------------------------------------------------------------------

#[cfg(feature = "hip")]
mod harness {
    use gf2_algebra::gpu::{
        permanent_batch_bipedal3, permanent_batch_bipedal5, permanent_batch_bipedal7,
    };
    use gf2_algebra::packed::{Bipedal3Matrix, Packed5Matrix, Packed7Matrix};
    use gf2_algebra::permanent::{permanent_bipedal3, permanent_bipedal5, permanent_bipedal7};
    use gf2_algebra::testutil::random_matrix_with_rng;
    use gf2_core::field::inverse::det;
    use gf2_core::field::matrix::FieldMatrix;
    use gf2_core::gfp::Fp;
    use gf2_core::rng::Lcg;

    use perm_uniformity::harness::{
        bootstrap_diff_ci, bootstrap_tvd_ci, tvd_from_counts, CellResult,
    };

    use std::fs;
    use std::io::Write;
    use std::time::{Duration, Instant};

    // -----------------------------------------------------------------------
    // Deterministic seed embedded in the CSV header.
    //
    // Identical to the 8e4e19a0 master seed + cell_seed derivation so the
    // perm/det/bootstrap streams are produced by the same arithmetic; the
    // statistical columns are therefore directly comparable.
    // -----------------------------------------------------------------------

    /// Master seed for the sweep (identical to the 8e4e19a0 `SEED`).
    const SEED: u64 = 0xc0_ffee_0000_0001_u64;

    /// Deterministic per-cell seed derived from `(q, n, which)`.
    ///
    /// Byte-for-byte the 8e4e19a0 `cell_seed`; `which` selects the perm
    /// stream (0), det stream (1), bootstrap-perm (2), bootstrap-det (3) and
    /// bootstrap-diff (4) sub-seeds.
    fn cell_seed(q: u64, n: usize, which: u64) -> u64 {
        SEED.wrapping_add(q.wrapping_mul(0x9e37_79b9_7f4a_7c15))
            .wrapping_add((n as u64).wrapping_mul(0x6c62_272e_07bb_0142))
            .wrapping_add(which.wrapping_mul(0x1234_5678_9abc_def0))
    }

    // -----------------------------------------------------------------------
    // GPU watchdog mitigation: bounded-duration sub-batch kernels + cooldown.
    //
    // The gfx1030 driver raises a `GPU Hang` HW exception when a single
    // `permanent_batch_bipedal{5,7}` launch runs too long (a TDR-style
    // watchdog on sustained F_5/F_7 byte/LUT kernels at large n; the Bipedal3
    // F_3 kernel did not trip it, even on a 2300 s single launch at n=32).
    //
    // The fix is to bound the *per-launch* device time, independent of the
    // cell's total N: split the N matrices into sub-batches whose single
    // kernel launch stays well under the watchdog limit, with a short host
    // cooldown (an explicit sleep; the dispatcher already does an implicit
    // hipDeviceSynchronize per launch) between sub-batches so the driver's
    // command queue / watchdog timer resets.
    //
    // Sub-batch size is chosen so `sub_batch * 2^n` (a proxy for per-launch
    // device work, since each block walks a Gray code of length 2^n and the
    // ~num_CU concurrent blocks process the batch in waves) stays under a
    // tunable work budget.  Both the work budget and the cooldown are env-
    // overridable for calibration without recompiling:
    //
    //   PUG_GPU_WORK_BUDGET  (default 6.0e9)  -- sub_batch*2^n upper bound
    //   PUG_GPU_COOLDOWN_MS  (default 400)    -- host sleep between launches
    //   PUG_GPU_MAX_SUBBATCH (default 2048)   -- hard cap on sub-batch size
    //   PUG_GPU_MIN_SUBBATCH (default 1)      -- floor (>=1 always)
    //
    // Chunking is purely a launch-granularity knob: all N matrices are
    // generated in strict seed order *before* any sub-batching, so the i-th
    // sample matrix — and therefore every statistical column — is identical
    // regardless of sub-batch size.  This is asserted by a chunked-vs-
    // unchunked bit-identity check in the smoke tests / validation path.
    // -----------------------------------------------------------------------

    fn env_f64(key: &str, default: f64) -> f64 {
        std::env::var(key)
            .ok()
            .and_then(|s| s.trim().parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(default)
    }
    fn env_usize(key: &str, default: usize) -> usize {
        std::env::var(key)
            .ok()
            .and_then(|s| s.trim().parse::<usize>().ok())
            .filter(|v| *v >= 1)
            .unwrap_or(default)
    }
    fn env_u64(key: &str, default: u64) -> u64 {
        std::env::var(key)
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(default)
    }

    /// Sub-batch size (matrices per single GPU kernel launch) for a cell of
    /// dimension `n`, bounding per-launch device work to keep it under the
    /// gfx1030 watchdog.  `sub_batch * 2^n <= work_budget`, clamped to
    /// `[min, max]` and never larger than the cell's `n_samples`.
    fn gpu_sub_batch(q: u64, n: usize, n_samples: usize) -> usize {
        // gfx1030 calibration (per-launch wall, F_5 vs F_7):
        //   F_5 n=20: 64->6.3s, 512->10.2s, 1024->16.0s  (~0.010 s/mat)
        //   F_5 n=24: 512->190s                            (~0.36 s/mat)
        //   F_7 n=20: 1907->28s                            (~0.015 s/mat)
        //   F_7 n=24: 119->155s                            (~1.30 s/mat)
        // The F_7 LUT kernel is markedly slower per matrix than the F_5
        // byte kernel and the F_3 packed kernel, so the work budget is
        // q-aware: a lower budget for q=7 keeps its launches under the
        // ~190-200 s gfx1030 GPU-Hang boundary with comfortable margin
        // (target per-launch wall ~= 20-30 s).  The budget is the *only*
        // watchdog knob; N is NEVER reduced below the noise-floor
        // requirement to dodge the watchdog.
        //
        //   q=3 : 4.0e9  (Bipedal3 packed kernel tolerated a single
        //                 2300 s launch at n=32 without hanging; keep the
        //                 large headline cells un-fragmented)
        //   q=5 : 1.3e9  (n=20 ~2000 mat ~20s; n=24 ~77 mat ~28s)
        //   q=7 : 3.5e8  (n=20 ~334 mat ~?; n=24 ~20 mat ~26s)
        //
        // Overridable via PUG_GPU_WORK_BUDGET (applies to all q).
        let default_budget = match q {
            3 => 4.0e9,
            5 => 1.3e9,
            _ => 3.5e8, // q == 7 (and any other)
        };
        let work_budget = env_f64("PUG_GPU_WORK_BUDGET", default_budget);
        let max_sb = env_usize("PUG_GPU_MAX_SUBBATCH", 2048);
        let min_sb = env_usize("PUG_GPU_MIN_SUBBATCH", 1);
        // 2^n as f64 (n <= 63 enforced by the GPU dispatcher).
        let pow2n = (n as f64).exp2();
        let by_budget = (work_budget / pow2n).floor();
        let sb = if by_budget < 1.0 {
            1usize
        } else if by_budget > usize::MAX as f64 {
            usize::MAX
        } else {
            by_budget as usize
        };
        sb.clamp(min_sb, max_sb).max(1).min(n_samples.max(1))
    }

    /// Host cooldown between consecutive GPU kernel launches (lets the
    /// driver's watchdog/queue reset).  `PUG_GPU_COOLDOWN_MS`, default 400 ms.
    fn gpu_cooldown() -> Duration {
        Duration::from_millis(env_u64("PUG_GPU_COOLDOWN_MS", 400))
    }

    // -----------------------------------------------------------------------
    // Sweep grid + per-cell N (noise-floor reasoning -- see r4 writeup §2).
    //
    // Monte-Carlo TVD-from-uniform noise floor ~ sqrt((q-1)/(2*pi*N)).
    // We pick N so that floor is comfortably below TVD_det/2 (so the bootstrap
    // diff_q95 < 0 genuinely, criterion-6 PASS) AND so TVD_perm is resolved
    // above its own floor (genuine convergence, not noise).  N is the
    // [aspirational] provisional knob and is refined against measured data.
    // -----------------------------------------------------------------------

    /// One sweep cell: prime, dimension, sample count.
    struct CellSpec {
        q: u64,
        n: usize,
        n_samples: usize,
    }

    /// The GPU resample grid.
    ///
    /// q=3: the small/mid n cells (for the monotonicity criterion, high-N like
    /// the CPU run) PLUS the three 8e4e19a0 noise-excluded cells n in
    /// {24,28,32} at GPU-feasible high N (the headline).
    ///
    /// q=5 / q=7: extended past 8e4e19a0's n<=14 as far as GPU wall-clock
    /// feasibly allows.
    fn sweep_grid() -> Vec<CellSpec> {
        let mut cells = Vec::new();

        // -- F_3 ------------------------------------------------------------
        // Small/mid n: cheap on GPU, take big N (noise floor << TVD_det).
        for &(n, n_samples) in &[
            (6usize, 500_000usize),
            (8, 500_000),
            (10, 500_000),
            (12, 200_000),
            (16, 200_000),
            (20, 100_000),
        ] {
            cells.push(CellSpec { q: 3, n, n_samples });
        }
        // The 8e4e19a0 noise-excluded headline cells.
        //   n=24: floor(N=40k)=sqrt(2/(2*pi*40000))=0.00282 << TVD_det/2~0.045
        //   n=28: floor(N=8k) =0.00631  << 0.045
        //   n=32: floor(N=2k) =0.01262  <  0.045  (still resolves TVD_perm)
        for &(n, n_samples) in &[(24usize, 40_000usize), (28, 8_000), (32, 2_000)] {
            cells.push(CellSpec { q: 3, n, n_samples });
        }

        // -- F_5 ------------------------------------------------------------
        // 8e4e19a0 capped at n<=14 (CPU permanent_bipedal5 single-word limit).
        // GPU path supports n<=63.  TVD_det(q=5) ~ 0.04, so the Monte-Carlo
        // TVD noise floor sqrt((q-1)/(2*pi*N)) = sqrt(4/(2*pi*N)) must stay
        // comfortably below TVD_det/2 ~ 0.02:
        //   N=200k -> floor 0.00178   (n=8,12)
        //   N=40k  -> floor 0.00399   (n=16, already DONE genuine PASS)
        //   N=20k  -> floor 0.00564   (n=20,24,28)  -- 0.00564 << 0.02
        // The watchdog is NOT defeated by shrinking N (that would raise the
        // floor and risk a fake/under-resolved PASS); it is defeated by the
        // bounded-duration sub-batch kernels + cooldown (see GPU_SUB_BATCH).
        // N=20k keeps the q=5 floor at 0.00564 (margin > 3x below TVD_det/2)
        // while the chunked launches keep each kernel short enough to dodge
        // the TDR.
        // q=5 n=24: N=8000 -> floor sqrt(4/(2*pi*8000))=0.00892, the exact
        // 8e4e19a0 q=5-large-n standard ("<< 0.02"; >2x below TVD_det/2).
        // q=5 n=28: at the noise-floor-required N the 2^28 Gray walk makes
        // a single watchdog-safe sub-batch take so long that the cell needs
        // ~50+ h on gfx1030 even fully chunked -- it is hardware-infeasible
        // at the required N (NOT under-sampled to fake a PASS); reported
        // honestly with measured per-launch evidence in the writeup/handoff.
        for &(n, n_samples) in &[
            (8usize, 200_000usize),
            (12, 200_000),
            (16, 40_000),
            (20, 20_000),
            (24, 8_000),
        ] {
            cells.push(CellSpec { q: 5, n, n_samples });
        }

        // -- F_7 ------------------------------------------------------------
        // 8e4e19a0 capped at n<=14 (CPU permanent_bipedal7 LANES=16 limit).
        // TVD_det(q=7) ~ 0.02 (SMALLER than q=5), so the floor must be even
        // lower -- comfortably below TVD_det/2 ~ 0.01.  q=7 therefore needs
        // LARGER N than q=5 at the same n; we do NOT under-sample q=7 to
        // dodge the watchdog (the sub-batch mitigation handles kernel
        // duration regardless of total N).  floor=sqrt(6/(2*pi*N)):
        //   N=300k -> floor 0.00178   (n=8,12)
        //   N=40k  -> floor 0.00489   (n=16,20)
        //   N=20k  -> floor 0.00691   (n=24)  -- 0.00691 < 0.01 (margin OK)
        // q=7 n=24: the F_7 LUT kernel is ~5x slower per matrix than F_5,
        // and TVD_det(q=7) ~ 0.02 demands a LOWER floor (<< 0.01), so the
        // required N (>= 20000 for floor 0.0069) at the watchdog-safe F_7
        // sub-batch (~20 matrices/launch, ~26 s each on gfx1030) needs
        // ~7+ h -- hardware-infeasible at the required N (NOT under-sampled);
        // reported honestly with measured per-launch evidence.  F_7
        // n=8,12,16,20 are feasible and run here.
        for &(n, n_samples) in &[
            (8usize, 300_000usize),
            (12, 300_000),
            (16, 40_000),
            (20, 40_000),
        ] {
            cells.push(CellSpec { q: 7, n, n_samples });
        }

        cells
    }

    // -----------------------------------------------------------------------
    // Generic GPU-batched cell runner.
    //
    // This is the only new logic.  It reproduces the tail of
    // perm_uniformity::harness::run_cell *exactly* (same histogram, same
    // tvd_from_counts, same bootstrap_tvd_ci x2, same bootstrap_diff_ci, same
    // per-cell seeds) but draws/evaluates the perm stream via the GPU batch
    // kernel instead of a per-sample CPU closure.
    //
    // `gen_mats`   : Lcg(perm_seed) -> Vec of N matrices in strict seed order.
    // `gpu_batch`  : &[Matrix] (a chunk) -> Vec<u8> of perm field values.
    // `det_sample` : (&mut Lcg, n) -> u8, the det field value (mirrors the
    //                8e4e19a0 det closure exactly).
    // -----------------------------------------------------------------------
    #[allow(clippy::too_many_arguments)]
    fn run_cell_gpu<M, FG, FB, FD>(
        q: u64,
        n: usize,
        n_samples: usize,
        gen_mats: FG,
        gpu_batch: FB,
        mut det_sample: FD,
    ) -> CellResult
    where
        FG: Fn(&mut Lcg, usize, usize) -> Vec<M>,
        FB: Fn(&[M]) -> Vec<u8>,
        FD: FnMut(&mut Lcg, usize) -> u8,
    {
        let perm_seed = cell_seed(q, n, 0);
        let det_seed = cell_seed(q, n, 1);
        let boot_perm_seed = cell_seed(q, n, 2);
        let boot_det_seed = cell_seed(q, n, 3);
        let boot_diff_seed = cell_seed(q, n, 4);

        // --- perm stream: GPU-batched -------------------------------------
        // Generate ALL N matrices in seed order first; chunking afterwards
        // cannot reorder the seed->matrix mapping.
        let mut rng_perm = Lcg::new(perm_seed);
        let mats = gen_mats(&mut rng_perm, n, n_samples);
        assert_eq!(mats.len(), n_samples);

        let mut perm_counts = vec![0u64; q as usize];
        let mut perm_samples: Vec<u8> = Vec::with_capacity(n_samples);

        // Watchdog-safe: bound per-launch device time via an n-dependent
        // sub-batch, with a host cooldown between launches.  Sub-batching
        // does NOT change which matrix is the i-th sample (mats was already
        // generated in strict seed order), so the statistical columns are
        // identical to the un-chunked path.
        let sub_batch = gpu_sub_batch(q, n, n_samples);
        let cooldown = gpu_cooldown();
        let n_launches = n_samples.div_ceil(sub_batch);
        eprintln!(
            "    GPU sub-batch={} ({} launches) cooldown={}ms  (work_budget bound, n={})",
            sub_batch,
            n_launches,
            cooldown.as_millis(),
            n
        );

        let t_perm_start = Instant::now();
        for (li, chunk) in mats.chunks(sub_batch).enumerate() {
            let t_launch = Instant::now();
            let vals = gpu_batch(chunk);
            let launch_s = t_launch.elapsed().as_secs_f64();
            assert_eq!(vals.len(), chunk.len());
            for v in vals {
                assert!((v as u64) < q, "perm value {v} out of range for q={q}");
                perm_counts[v as usize] += 1;
                perm_samples.push(v);
            }
            // Progress + per-launch wall (so a hang is attributable to a
            // specific launch duration, and so we can tune the work budget).
            if li == 0 || (li + 1) % 16 == 0 || li + 1 == n_launches {
                eprintln!(
                    "    launch {}/{}  {} matrices  {:.2}s",
                    li + 1,
                    n_launches,
                    chunk.len(),
                    launch_s
                );
            }
            // Host cooldown between launches (skip after the last one).
            if li + 1 < n_launches && !cooldown.is_zero() {
                std::thread::sleep(cooldown);
            }
        }
        let perm_elapsed = t_perm_start.elapsed().as_secs_f64();

        // --- det stream: CPU, independent RNG -----------------------------
        let mut rng_det = Lcg::new(det_seed);
        let mut det_counts = vec![0u64; q as usize];
        let mut det_samples: Vec<u8> = Vec::with_capacity(n_samples);
        let t_det_start = Instant::now();
        for _ in 0..n_samples {
            let v = det_sample(&mut rng_det, n);
            assert!((v as u64) < q, "det value {v} out of range for q={q}");
            det_counts[v as usize] += 1;
            det_samples.push(v);
        }
        let det_elapsed = t_det_start.elapsed().as_secs_f64();

        // --- statistics: reused 8e4e19a0 harness functions, verbatim ------
        let tvd_perm = tvd_from_counts(&perm_counts, n_samples as u64, q);
        let tvd_det = tvd_from_counts(&det_counts, n_samples as u64, q);

        let (pci_lo, pci_hi) = bootstrap_tvd_ci(&perm_samples, q, 1000, boot_perm_seed);
        let (dci_lo, dci_hi) = bootstrap_tvd_ci(&det_samples, q, 1000, boot_det_seed);
        let (_, diff_q95) = bootstrap_diff_ci(&perm_samples, &det_samples, q, 1000, boot_diff_seed);

        CellResult {
            q,
            n,
            n_samples,
            tvd_perm,
            tvd_perm_ci_lo: pci_lo,
            tvd_perm_ci_hi: pci_hi,
            tvd_det,
            tvd_det_ci_lo: dci_lo,
            tvd_det_ci_hi: dci_hi,
            diff_q95,
            mean_us_perm: perm_elapsed * 1e6 / n_samples as f64,
            mean_us_det: det_elapsed * 1e6 / n_samples as f64,
        }
    }

    // -----------------------------------------------------------------------
    // Per-prime matrix generation + GPU batch wrappers.
    //
    // The `gen_*` closures draw exactly as the 8e4e19a0 perm closure does
    // (random_matrix_with_rng::<P> == element-by-element rng.next_u64() % P,
    // row-major n*n), so the i-th GPU sample matrix is bit-identical to the
    // i-th CPU sample matrix for the same seed.
    //
    // The `det_*` closures draw exactly as the 8e4e19a0 det closure does
    // (FieldMatrix::set in (r,c) order, rng.next_u64() % P each), so the det
    // stream is byte-for-byte the CPU stream for the same seed.
    // -----------------------------------------------------------------------

    fn gen_mats_f3(rng: &mut Lcg, n: usize, m: usize) -> Vec<Bipedal3Matrix> {
        (0..m)
            .map(|_| {
                let elems: Vec<Fp<3>> = random_matrix_with_rng::<3>(rng, n);
                Bipedal3Matrix::from_row_major(&elems, n, n)
            })
            .collect()
    }
    fn gen_mats_f5(rng: &mut Lcg, n: usize, m: usize) -> Vec<Packed5Matrix> {
        (0..m)
            .map(|_| {
                let elems: Vec<Fp<5>> = random_matrix_with_rng::<5>(rng, n);
                Packed5Matrix::from_row_major(&elems, n, n)
            })
            .collect()
    }
    fn gen_mats_f7(rng: &mut Lcg, n: usize, m: usize) -> Vec<Packed7Matrix> {
        (0..m)
            .map(|_| {
                let elems: Vec<Fp<7>> = random_matrix_with_rng::<7>(rng, n);
                Packed7Matrix::from_row_major(&elems, n, n)
            })
            .collect()
    }

    fn det_sample_f3(rng: &mut Lcg, size: usize) -> u8 {
        let mut mat = FieldMatrix::<Fp<3>>::zeros(size, size);
        for r in 0..size {
            for c in 0..size {
                mat.set(r, c, Fp::<3>::new(rng.next_u64() % 3));
            }
        }
        det(&mat).value() as u8
    }
    fn det_sample_f5(rng: &mut Lcg, size: usize) -> u8 {
        let mut mat = FieldMatrix::<Fp<5>>::zeros(size, size);
        for r in 0..size {
            for c in 0..size {
                mat.set(r, c, Fp::<5>::new(rng.next_u64() % 5));
            }
        }
        det(&mat).value() as u8
    }
    fn det_sample_f7(rng: &mut Lcg, size: usize) -> u8 {
        let mut mat = FieldMatrix::<Fp<7>>::zeros(size, size);
        for r in 0..size {
            for c in 0..size {
                mat.set(r, c, Fp::<7>::new(rng.next_u64() % 7));
            }
        }
        det(&mat).value() as u8
    }

    // -----------------------------------------------------------------------
    // GPU-vs-CPU correctness validation (must pass before we trust the GPU
    // batch permanent for the headline measurement).
    // -----------------------------------------------------------------------

    /// Assert the GPU batch permanent agrees with the CPU `permanent_bipedal*`
    /// on a small seeded batch for each q, AND that the watchdog-safe
    /// sub-batched GPU loop yields a bit-identical sample stream to a single
    /// un-chunked launch.  Panics on any mismatch.
    fn validate_gpu_matches_cpu() {
        eprintln!("--- GPU-vs-CPU correctness validation ---");
        validate_f3();
        validate_f5();
        validate_f7();
        eprintln!("  PASS: GPU batch permanent == CPU permanent on all probe cells");
        validate_chunked_equals_unchunked();
        eprintln!("  PASS: sub-batched GPU loop == un-chunked single launch (bit-identical)");
    }

    /// Confirm sub-batching does not perturb the sampled permanent stream:
    /// generate one seeded set of `m` matrices, evaluate them (a) in a single
    /// `permanent_batch_bipedal*` launch and (b) split into small sub-batches
    /// exactly as `run_cell_gpu` does, and assert the resulting `u8` value
    /// vectors are identical element-for-element.  Done for F_5 and F_7 (the
    /// kernels whose long launches the chunking exists to bound).
    fn validate_chunked_equals_unchunked() {
        // F_5
        {
            let n = 10usize;
            let m = 200usize;
            let mut rng = Lcg::new(0x5eed_0005_c0c0_0001);
            let mats: Vec<Packed5Matrix> = (0..m)
                .map(|_| {
                    Packed5Matrix::from_row_major(&random_matrix_with_rng::<5>(&mut rng, n), n, n)
                })
                .collect();
            let whole: Vec<u8> = permanent_batch_bipedal5(&mats)
                .into_iter()
                .map(|x| x.value() as u8)
                .collect();
            let mut chunked: Vec<u8> = Vec::with_capacity(m);
            for c in mats.chunks(37) {
                chunked.extend(
                    permanent_batch_bipedal5(c)
                        .into_iter()
                        .map(|x| x.value() as u8),
                );
            }
            assert_eq!(
                whole, chunked,
                "F_5 sub-batched stream differs from un-chunked single launch"
            );
        }
        // F_7
        {
            let n = 11usize;
            let m = 200usize;
            let mut rng = Lcg::new(0x5eed_0007_c0c0_0001);
            let mats: Vec<Packed7Matrix> = (0..m)
                .map(|_| {
                    Packed7Matrix::from_row_major(&random_matrix_with_rng::<7>(&mut rng, n), n, n)
                })
                .collect();
            let whole: Vec<u8> = permanent_batch_bipedal7(&mats)
                .into_iter()
                .map(|x| x.value() as u8)
                .collect();
            let mut chunked: Vec<u8> = Vec::with_capacity(m);
            for c in mats.chunks(37) {
                chunked.extend(
                    permanent_batch_bipedal7(c)
                        .into_iter()
                        .map(|x| x.value() as u8),
                );
            }
            assert_eq!(
                whole, chunked,
                "F_7 sub-batched stream differs from un-chunked single launch"
            );
        }
    }

    fn validate_f3() {
        let n = 10usize;
        let m = 64usize;
        let mut rng = Lcg::new(0x5eed_0003_0000_0001);
        let mats: Vec<Bipedal3Matrix> = (0..m)
            .map(|_| {
                Bipedal3Matrix::from_row_major(&random_matrix_with_rng::<3>(&mut rng, n), n, n)
            })
            .collect();
        let gpu = permanent_batch_bipedal3(&mats);
        for (i, mat) in mats.iter().enumerate() {
            let cpu = permanent_bipedal3(mat);
            assert_eq!(
                gpu[i], cpu,
                "F_3 GPU/CPU permanent mismatch at i={i}: gpu={:?} cpu={:?}",
                gpu[i], cpu
            );
        }
        eprintln!("  F_3 n={n} m={m}: GPU == CPU");
    }

    fn validate_f5() {
        let n = 10usize;
        let m = 64usize;
        let mut rng = Lcg::new(0x5eed_0005_0000_0001);
        let mats: Vec<Packed5Matrix> = (0..m)
            .map(|_| Packed5Matrix::from_row_major(&random_matrix_with_rng::<5>(&mut rng, n), n, n))
            .collect();
        let gpu = permanent_batch_bipedal5(&mats);
        for (i, mat) in mats.iter().enumerate() {
            let cpu = permanent_bipedal5(mat);
            assert_eq!(
                gpu[i], cpu,
                "F_5 GPU/CPU permanent mismatch at i={i}: gpu={:?} cpu={:?}",
                gpu[i], cpu
            );
        }
        eprintln!("  F_5 n={n} m={m}: GPU == CPU");
    }

    fn validate_f7() {
        // CPU permanent_bipedal7 is limited to n <= 16 = Packed7::LANES.
        let n = 12usize;
        let m = 64usize;
        let mut rng = Lcg::new(0x5eed_0007_0000_0001);
        let mats: Vec<Packed7Matrix> = (0..m)
            .map(|_| Packed7Matrix::from_row_major(&random_matrix_with_rng::<7>(&mut rng, n), n, n))
            .collect();
        let gpu = permanent_batch_bipedal7(&mats);
        for (i, mat) in mats.iter().enumerate() {
            let cpu = permanent_bipedal7(mat);
            assert_eq!(
                gpu[i], cpu,
                "F_7 GPU/CPU permanent mismatch at i={i}: gpu={:?} cpu={:?}",
                gpu[i], cpu
            );
        }
        eprintln!("  F_7 n={n} m={m}: GPU == CPU");
    }

    // -----------------------------------------------------------------------
    // CSV emission (exact 8e4e19a0 schema + hardware-fingerprint header).
    // -----------------------------------------------------------------------

    fn probe(cmd: &str, args: &[&str]) -> String {
        std::process::Command::new(cmd)
            .args(args)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string())
    }

    fn cpu_model() -> String {
        std::fs::read_to_string("/proc/cpuinfo")
            .ok()
            .and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with("model name"))
                    .and_then(|l| l.split(':').nth(1))
                    .map(|v| v.trim().to_string())
            })
            .unwrap_or_else(|| "unknown".to_string())
    }

    fn gpu_fingerprint() -> (String, String) {
        let ri = std::process::Command::new("rocminfo")
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
        let gfx = ri
            .lines()
            .filter(|l| l.contains("Name") && l.contains("gfx"))
            .find_map(|l| {
                l.split_whitespace()
                    .find(|w| w.starts_with("gfx"))
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "unknown".to_string());
        let name = ri
            .lines()
            .find(|l| l.contains("Marketing Name") && l.contains("Radeon"))
            .and_then(|l| l.split(':').nth(1))
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        (name, gfx)
    }

    fn write_csv(results: &[CellResult], path: &str) {
        let mut f = fs::File::create(path).expect("cannot create CSV");
        let (gpu_name, gfx) = gpu_fingerprint();
        let rocm = std::fs::read_to_string("/opt/rocm/.info/version")
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        let git = std::process::Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        writeln!(
            f,
            "# perm-uniformity-gpu resample (JIT b293af5a)  supersedes 8e4e19a0 noise-limited cells"
        )
        .unwrap();
        writeln!(f, "# commit: {git}").unwrap();
        writeln!(f, "# rustc: {}", probe("rustc", &["--version"])).unwrap();
        writeln!(f, "# cpu: {}", cpu_model()).unwrap();
        writeln!(f, "# gpu: {gpu_name}").unwrap();
        writeln!(f, "# gfx_target: {gfx}").unwrap();
        writeln!(f, "# rocm_version: {rocm}").unwrap();
        writeln!(f, "# kernel: {}", probe("uname", &["-r"])).unwrap();
        writeln!(f, "# seed: {SEED:#018x}  date=2026-05-17  jit=b293af5a").unwrap();
        writeln!(
            f,
            "q,n,samples,tvd_perm,tvd_perm_ci_lo,tvd_perm_ci_hi,tvd_det,tvd_det_ci_lo,tvd_det_ci_hi,mean_us_perm,mean_us_det"
        )
        .unwrap();
        for r in results {
            writeln!(
                f,
                "{},{},{},{:.8},{:.8},{:.8},{:.8},{:.8},{:.8},{:.4},{:.4}",
                r.q,
                r.n,
                r.n_samples,
                r.tvd_perm,
                r.tvd_perm_ci_lo,
                r.tvd_perm_ci_hi,
                r.tvd_det,
                r.tvd_det_ci_lo,
                r.tvd_det_ci_hi,
                r.mean_us_perm,
                r.mean_us_det
            )
            .unwrap();
        }
    }

    // -----------------------------------------------------------------------
    // Driver
    // -----------------------------------------------------------------------

    pub fn run() {
        let output_dir = std::env::var("OUTPUT_DIR")
            .unwrap_or_else(|_| "dev/benchmarks/perm_uniformity".to_string());
        fs::create_dir_all(&output_dir).expect("cannot create output dir");
        let csv_path = format!("{output_dir}/results-2026-05-17-gpu.csv");

        // PLOT_ONLY: regenerate the figure from the persisted SSOT CSV without
        // re-running any cell. Required because resumed cells are merged into
        // the CSV out-of-band and re-running the full grid to refresh the plot
        // is hardware-infeasible (F_5 n=20 alone is ~4 h). Reuses write_plot
        // (no duplicate plotting logic) and never calls write_csv, so the
        // committed two-commit provenance header is left untouched.
        if std::env::var("PLOT_ONLY").is_ok() {
            let raw = fs::read_to_string(&csv_path)
                .unwrap_or_else(|e| panic!("PLOT_ONLY: cannot read {csv_path}: {e}"));
            let mut results: Vec<CellResult> = Vec::new();
            for line in raw.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') || line.starts_with("q,n,") {
                    continue;
                }
                let c: Vec<&str> = line.split(',').collect();
                assert!(
                    c.len() == 11,
                    "PLOT_ONLY: malformed CSV row ({} cols): {line}",
                    c.len()
                );
                let p = |i: usize| -> f64 {
                    c[i].parse()
                        .unwrap_or_else(|e| panic!("PLOT_ONLY: bad float col {i} in '{line}': {e}"))
                };
                results.push(CellResult {
                    q: c[0].parse().expect("PLOT_ONLY: bad q"),
                    n: c[1].parse().expect("PLOT_ONLY: bad n"),
                    n_samples: c[2].parse().expect("PLOT_ONLY: bad samples"),
                    tvd_perm: p(3),
                    tvd_perm_ci_lo: p(4),
                    tvd_perm_ci_hi: p(5),
                    tvd_det: p(6),
                    tvd_det_ci_lo: p(7),
                    tvd_det_ci_hi: p(8),
                    diff_q95: 0.0, // not stored in CSV; unused by write_plot
                    mean_us_perm: p(9),
                    mean_us_det: p(10),
                });
            }
            assert!(!results.is_empty(), "PLOT_ONLY: no data rows in {csv_path}");
            let plot_path = format!("{output_dir}/tvd_vs_n_gpu.png");
            write_plot(&results, &plot_path);
            println!(
                "PLOT_ONLY: regenerated {plot_path} from {} cells in {csv_path}",
                results.len()
            );
            return;
        }

        println!("perm-uniformity-gpu resample (JIT b293af5a)");
        println!("  seed   = {SEED:#018x}");
        println!("  output = {csv_path}");
        println!();

        // Correctness gate: GPU batch permanent must equal CPU permanent on
        // small seeded matrices before we trust any headline number.
        validate_gpu_matches_cpu();
        println!();

        // Optional cell-range restriction via env (resume support):
        //   CELLS=q3n24,q3n28  perm_uniformity_gpu ...
        let only: Option<Vec<String>> = std::env::var("CELLS")
            .ok()
            .map(|s| s.split(',').map(|t| t.trim().to_string()).collect());

        let grid = sweep_grid();
        let mut results: Vec<CellResult> = Vec::new();
        let t_sweep = Instant::now();

        for spec in &grid {
            let tag = format!("q{}n{}", spec.q, spec.n);
            if let Some(ref sel) = only {
                if !sel.iter().any(|s| s == &tag) {
                    continue;
                }
            }
            let t_cell = Instant::now();
            println!(
                "[{}] q={} n={} N={} ...",
                tag, spec.q, spec.n, spec.n_samples
            );

            let r = match spec.q {
                3 => run_cell_gpu(
                    3,
                    spec.n,
                    spec.n_samples,
                    gen_mats_f3,
                    |c| {
                        permanent_batch_bipedal3(c)
                            .into_iter()
                            .map(|x| x.value() as u8)
                            .collect()
                    },
                    det_sample_f3,
                ),
                5 => run_cell_gpu(
                    5,
                    spec.n,
                    spec.n_samples,
                    gen_mats_f5,
                    |c| {
                        permanent_batch_bipedal5(c)
                            .into_iter()
                            .map(|x| x.value() as u8)
                            .collect()
                    },
                    det_sample_f5,
                ),
                7 => run_cell_gpu(
                    7,
                    spec.n,
                    spec.n_samples,
                    gen_mats_f7,
                    |c| {
                        permanent_batch_bipedal7(c)
                            .into_iter()
                            .map(|x| x.value() as u8)
                            .collect()
                    },
                    det_sample_f7,
                ),
                _ => unreachable!(),
            };

            let secs = t_cell.elapsed().as_secs_f64();
            let floor = (((spec.q - 1) as f64)
                / (2.0 * std::f64::consts::PI * spec.n_samples as f64))
                .sqrt();
            println!(
                "  tvd_perm={:.6} [{:.6},{:.6}]  tvd_det={:.6} [{:.6},{:.6}]  diff_q95={:.6}  noise_floor={:.6}  {:.1}s",
                r.tvd_perm,
                r.tvd_perm_ci_lo,
                r.tvd_perm_ci_hi,
                r.tvd_det,
                r.tvd_det_ci_lo,
                r.tvd_det_ci_hi,
                r.diff_q95,
                floor,
                secs
            );
            results.push(r);

            // Write incrementally so a long sweep never loses completed cells.
            write_csv(&results, &csv_path);
        }

        let total = t_sweep.elapsed().as_secs_f64();
        println!();
        println!("Sweep complete in {:.1}s ({:.1} min)", total, total / 60.0);
        println!("CSV written to {csv_path}");

        // Optional comparison plot (reuses the 8e4e19a0 PNG encoder).
        write_plot(&results, &format!("{output_dir}/tvd_vs_n_gpu.png"));

        // Criterion summaries (informational; the lead runs the gates).
        report_criteria(&results);
    }

    // -----------------------------------------------------------------------
    // Criterion reporting (mirrors 8e4e19a0 main.rs semantics).
    // -----------------------------------------------------------------------

    fn report_criteria(results: &[CellResult]) {
        println!();
        println!("--- Criterion: TVD_perm monotone non-increasing for q=3 ---");
        let f3: Vec<&CellResult> = {
            let mut v: Vec<&CellResult> = results.iter().filter(|r| r.q == 3).collect();
            v.sort_by_key(|r| r.n);
            v
        };
        let mut mono = true;
        for w in f3.windows(2) {
            let (prev, curr) = (w[0], w[1]);
            if curr.tvd_perm_ci_lo > prev.tvd_perm_ci_hi + 1e-9 {
                println!(
                    "  WARN n={} CI_lo={:.6} > prev(n={}) CI_hi={:.6} (no overlap)",
                    curr.n, curr.tvd_perm_ci_lo, prev.n, prev.tvd_perm_ci_hi
                );
                mono = false;
            } else {
                println!(
                    "  OK   n={} TVD_perm={:.6} CI=[{:.6},{:.6}]",
                    curr.n, curr.tvd_perm, curr.tvd_perm_ci_lo, curr.tvd_perm_ci_hi
                );
            }
        }
        println!(
            "  {}",
            if mono {
                "PASS monotone non-increasing within CI"
            } else {
                "FAIL monotonicity"
            }
        );

        println!();
        println!("--- Criterion: TVD_perm <= TVD_det at 95% (diff_q95 < 0) ---");
        let mut ok = true;
        for r in results.iter().filter(|r| r.n >= 8) {
            let floor =
                (((r.q - 1) as f64) / (2.0 * std::f64::consts::PI * r.n_samples as f64)).sqrt();
            let resolved = r.tvd_perm > floor || r.tvd_perm_ci_lo > 0.0;
            if r.diff_q95 < 0.0 {
                println!(
                    "  PASS q={} n={} N={}  perm={:.6} det={:.6} diff_q95={:.6} floor={:.6} resolved={}",
                    r.q, r.n, r.n_samples, r.tvd_perm, r.tvd_det, r.diff_q95, floor, resolved
                );
            } else {
                println!(
                    "  FAIL q={} n={} N={}  perm={:.6} det={:.6} diff_q95={:.6} floor={:.6}",
                    r.q, r.n, r.n_samples, r.tvd_perm, r.tvd_det, r.diff_q95, floor
                );
                ok = false;
            }
        }
        println!(
            "  {}",
            if ok {
                "PASS criterion-6 for every measured n>=8 cell"
            } else {
                "FAIL criterion-6 at one or more cells"
            }
        );
    }

    // -----------------------------------------------------------------------
    // Plot — reuses perm_uniformity::png; faceted by q, log-y, CI ribbons.
    // (Self-contained minimal renderer; the PNG *encoder* is the reused SSOT.)
    // -----------------------------------------------------------------------

    fn write_plot(results: &[CellResult], path: &str) {
        if results.is_empty() {
            return;
        }
        let qs = [3u64, 5, 7];
        let panel_w = 400usize;
        let panel_h = 420usize;
        let margin_left = 80usize;
        let margin_right = 20usize;
        let margin_top = 50usize;
        let margin_bottom = 65usize;
        let gap = 20usize;
        let total_w = qs.len() * panel_w + (qs.len() - 1) * gap;
        let total_h = panel_h;
        let mut px = vec![255u8; total_w * total_h * 3];

        let set = |buf: &mut [u8], x: usize, y: usize, c: (u8, u8, u8)| {
            if x < total_w && y < total_h {
                let b = (y * total_w + x) * 3;
                buf[b] = c.0;
                buf[b + 1] = c.1;
                buf[b + 2] = c.2;
            }
        };
        let blend = |buf: &mut [u8], x: usize, y: usize, c: (u8, u8, u8)| {
            if x < total_w && y < total_h {
                let b = (y * total_w + x) * 3;
                buf[b] = ((buf[b] as u16 * 7 + c.0 as u16 * 3) / 10) as u8;
                buf[b + 1] = ((buf[b + 1] as u16 * 7 + c.1 as u16 * 3) / 10) as u8;
                buf[b + 2] = ((buf[b + 2] as u16 * 7 + c.2 as u16 * 3) / 10) as u8;
            }
        };

        let y_min_log = -5.0f64;
        let y_max_log = 0.0f64;
        let col_perm = [(220, 50, 47), (38, 139, 210), (42, 161, 152)];
        let col_det = [(203, 75, 22), (108, 113, 196), (133, 153, 0)];

        for (qi, &q) in qs.iter().enumerate() {
            let mut cells: Vec<&CellResult> = results.iter().filter(|r| r.q == q).collect();
            if cells.is_empty() {
                continue;
            }
            cells.sort_by_key(|r| r.n);
            let ox = qi * (panel_w + gap);
            let px0 = ox + margin_left;
            let py0 = margin_top;
            let pw = panel_w - margin_left - margin_right;
            let ph = panel_h - margin_top - margin_bottom;
            let nmin = cells.iter().map(|r| r.n).min().unwrap();
            let nmax = cells.iter().map(|r| r.n).max().unwrap();
            let to_x = |n: usize| -> usize {
                let f = if nmax == nmin {
                    0.5
                } else {
                    (n - nmin) as f64 / (nmax - nmin) as f64
                };
                px0 + (f * (pw as f64 - 1.0)) as usize
            };
            let to_y = |t: f64| -> usize {
                let lt = if t <= 0.0 {
                    y_min_log
                } else {
                    t.log10().clamp(y_min_log, y_max_log)
                };
                let f = (lt - y_max_log) / (y_min_log - y_max_log);
                py0 + (f * (ph as f64 - 1.0)) as usize
            };
            for x in px0..=px0 + pw {
                set(&mut px, x, py0 + ph, (80, 80, 80));
            }
            for y in py0..=py0 + ph {
                set(&mut px, px0, y, (80, 80, 80));
            }
            for lt in [-5i32, -4, -3, -2, -1, 0] {
                let yt = to_y(10f64.powi(lt));
                for x in px0..=px0 + pw {
                    set(&mut px, x, yt, (220, 220, 220));
                }
            }
            let cp = col_perm[qi];
            let cd = col_det[qi];
            for c in &cells {
                let x = to_x(c.n);
                for (lo, hi, col) in [
                    (c.tvd_perm_ci_lo.max(1e-5), c.tvd_perm_ci_hi.max(1e-5), cp),
                    (c.tvd_det_ci_lo.max(1e-5), c.tvd_det_ci_hi.max(1e-5), cd),
                ] {
                    let (a, b) = (to_y(lo).min(to_y(hi)), to_y(lo).max(to_y(hi)));
                    for bx in x.saturating_sub(3)..=x + 3 {
                        for y in a..=b {
                            blend(&mut px, bx, y, col);
                        }
                    }
                }
            }
            let mut pp: Option<(usize, usize)> = None;
            let mut pd: Option<(usize, usize)> = None;
            for c in &cells {
                let x = to_x(c.n);
                let yp = to_y(c.tvd_perm.max(1e-5));
                let yd = to_y(c.tvd_det.max(1e-5));
                for (prev, cur, col) in [(&mut pp, (x, yp), cp), (&mut pd, (x, yd), cd)] {
                    if let Some((x0, y0)) = *prev {
                        let steps = ((x as i64 - x0 as i64).abs()
                            + (cur.1 as i64 - y0 as i64).abs()
                            + 1) as usize;
                        for s in 0..=steps {
                            let t = s as f64 / steps as f64;
                            let lx = (x0 as f64 + t * (x as f64 - x0 as f64)) as usize;
                            let ly = (y0 as f64 + t * (cur.1 as f64 - y0 as f64)) as usize;
                            set(&mut px, lx, ly, col);
                        }
                    }
                    *prev = Some(cur);
                }
                for dy in -4i64..=4 {
                    for dx in -4i64..=4 {
                        if dx * dx + dy * dy <= 16 {
                            set(
                                &mut px,
                                (x as i64 + dx) as usize,
                                (yp as i64 + dy) as usize,
                                cp,
                            );
                            set(
                                &mut px,
                                (x as i64 + dx) as usize,
                                (yd as i64 + dy) as usize,
                                cd,
                            );
                        }
                    }
                }
            }
        }

        match perm_uniformity::png::write_png_file(path, &px, total_w, total_h) {
            Ok(()) => println!("  plot written to {path} (reused perm_uniformity::png encoder)"),
            Err(e) => eprintln!("  ERROR writing PNG: {e}"),
        }
    }
}
