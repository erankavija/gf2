# cbf576d1 — SparseBitMatrix RCM reorder evidence

Issue: `cbf576d1-8694-4dde-bf29-d047381155c3`

## Host/toolchain

- Host: `fraktaali`, Linux `6.19.11-arch1-1`, x86_64
- CPU: AMD Ryzen 9 5900X 12-Core Processor, 12 cores / 24 threads
- L1d: 384 KiB total, L2: 6 MiB total, L3: 64 MiB total
- Rust: `rustc 1.95.0 (59807616e 2026-04-14)`, LLVM 22.1.2
- Build mode: `--release`
- Target flags: `RUSTFLAGS="-C target-cpu=native"`
- Target dir: `CARGO_TARGET_DIR=target-cbf576d1`

## Correctness / CI commands

```bash
CARGO_TARGET_DIR=target-cbf576d1 cargo fmt -p gf2-core -- --check
CARGO_TARGET_DIR=target-cbf576d1 cargo clippy -p gf2-core --release --lib --features rand -- -D warnings
CARGO_TARGET_DIR=target-cbf576d1 cargo nextest run -p gf2-core --release --profile ci -E 'test(rcm)'
CARGO_TARGET_DIR=target-cbf576d1 cargo test -p gf2-core --release --doc reorder_rcm --features rand
CARGO_TARGET_DIR=target-cbf576d1 cargo test -p gf2-core --release --doc RowPermutation --features rand
RUSTFLAGS="-C target-cpu=native" CARGO_TARGET_DIR=target-cbf576d1 \
  cargo bench -p gf2-core --bench sparse --features rand -- sparse_matvec_ldpc_rcm_amortized_128 --test
```

All commands above completed successfully. `cargo fmt --all -- --check` was
not usable from this nested worktree because Cargo discovers the parent checkout
workspace for the excluded HIP crate; `cargo fmt -p gf2-core -- --check` and
direct `rustfmt --check` on changed files both passed.

## Criterion amortized benchmark

Command:

```bash
RUSTFLAGS="-C target-cpu=native" CARGO_TARGET_DIR=target-cbf576d1 \
  cargo bench -p gf2-core --bench sparse --features rand -- sparse_matvec_ldpc_rcm_amortized_128
```

The benchmark performs 128 matvecs per Criterion iteration. RCM preprocessing
and input-vector column permutation are performed outside the timed loop to
model one-shot layout preprocessing amortized over repeated dispatches. The RCM
row result is unpermuted after each matvec so the benchmark measures the
bit-exact output path.

| case | CSR mean | RCM mean | RCM / CSR | outcome |
|---|---:|---:|---:|---|
| 4096x8192 w6 | 2.6142 ms | 3.2485 ms | 1.24x slower | no speedup |
| 8192x16384 w6 | 4.3906 ms | 6.9136 ms | 1.57x slower | no speedup |
| 4096x32768 w32 | 9.5282 ms | 11.506 ms | 1.21x slower | no speedup |

Geomean RCM/CSR ratio: approximately `1.33x` slower. The aspirational `>=1.5x`
speedup is not met on the deterministic LDPC-like fixture; the likely cause is
that the fixture's hashed column pattern does not expose enough locality for
RCM to offset the extra output unpermutation pass.

## `perf stat -r 10` representative before/after

Representative case: `sparse_matvec_ldpc_rcm_amortized_128/{csr,rcm}/4096x8192_w6`.
Each perf run invokes Criterion with `--warm-up-time 1 --measurement-time 1
--sample-size 10`.

### CSR

Command:

```bash
RUSTFLAGS="-C target-cpu=native" CARGO_TARGET_DIR=target-cbf576d1 \
  perf stat -r 10 -- cargo bench -p gf2-core --bench sparse --features rand -- \
  sparse_matvec_ldpc_rcm_amortized_128/csr/4096x8192_w6 \
  --warm-up-time 1 --measurement-time 1 --sample-size 10
```

Summary:

```text
Performance counter stats for 'cargo bench ... sparse_matvec_ldpc_rcm_amortized_128/csr/4096x8192_w6 ...' (10 runs):

             0      context-switches:u
             0      cpu-migrations:u
        79,983      page-faults:u                    #   6482.9 faults/sec  ( +-  0.15% )
     12,337.49 msec task-clock:u                     #      2.5 CPUs        ( +-  0.38% )
   175,648,524      branch-misses:u                  #      0.5 %           ( +-  3.59% )
33,674,381,070      branches:u                       #   2729.4 M/sec       ( +-  0.51% )
51,236,143,471      cpu-cycles:u                     #      4.2 GHz         ( +-  0.43% )
168,544,478,939     instructions:u                   #      3.3 IPC         ( +-  0.44% )
1,237,082,497       stalled-cycles-frontend:u        #      0.02            ( +-  2.41% )

   4.926371975 +- 0.066563047 seconds time elapsed  ( +-  1.35% )
```

### RCM

Command:

```bash
RUSTFLAGS="-C target-cpu=native" CARGO_TARGET_DIR=target-cbf576d1 \
  perf stat -r 10 -- cargo bench -p gf2-core --bench sparse --features rand -- \
  sparse_matvec_ldpc_rcm_amortized_128/rcm/4096x8192_w6 \
  --warm-up-time 1 --measurement-time 1 --sample-size 10
```

Summary:

```text
Performance counter stats for 'cargo bench ... sparse_matvec_ldpc_rcm_amortized_128/rcm/4096x8192_w6 ...' (10 runs):

             0      context-switches:u
             0      cpu-migrations:u
        79,866      page-faults:u                    #   6197.6 faults/sec  ( +-  0.11% )
     12,886.68 msec task-clock:u                     #      2.4 CPUs        ( +-  0.24% )
   229,948,785      branch-misses:u                  #      0.7 %           ( +-  1.97% )
33,858,664,268      branches:u                       #   2627.4 M/sec       ( +-  0.07% )
53,780,677,111      cpu-cycles:u                     #      4.2 GHz         ( +-  0.21% )
164,765,063,732     instructions:u                   #      3.1 IPC         ( +-  0.06% )
1,461,613,771       stalled-cycles-frontend:u        #      0.03            ( +-  1.41% )

   5.364928880 +- 0.012600380 seconds time elapsed  ( +-  0.23% )
```

Interpretation: this representative perf capture matches Criterion: RCM is
slower on the current deterministic fixture, with more branch misses and cycles
despite slightly fewer retired instructions.
