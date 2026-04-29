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

## Rework summary

Lead review found that the first benchmark measured RCM plus a per-call
`unapply_rows` pass. That is useful for callers that require original row
order after every multiply, but it is not the intended amortized layout for D2:
preprocess once, keep input vectors in reordered column order, and keep
syndrome/results in reordered row order across repeated LDPC-style dispatches.

The benchmark now has three leaves:

- `csr`: unchanged CSR matvec, 128 calls per Criterion iteration.
- `rcm_reordered_output`: RCM matvec with pre-permuted input and reordered
  output, 128 calls per Criterion iteration. This is the D2 gate leaf.
- `rcm_original_output`: same RCM matvec plus `unapply_rows` after each call,
  retained as evidence for the original-output cost.

Correctness remains covered by unit/property tests and docs:
`perm.unapply_rows(&reordered.matvec(&perm.apply_cols(&x))) == csr.matvec(&x)`.

## Correctness / CI commands

```bash
CARGO_TARGET_DIR=target-cbf576d1 cargo fmt -p gf2-core -- --check
CARGO_TARGET_DIR=target-cbf576d1 cargo clippy -p gf2-core --release --lib --features rand -- -D warnings
CARGO_TARGET_DIR=target-cbf576d1 cargo nextest run -p gf2-core --release --profile ci -E 'test(rcm)'
CARGO_TARGET_DIR=target-cbf576d1 cargo test -p gf2-core --release --doc RowPermutation --features rand
CARGO_TARGET_DIR=target-cbf576d1 cargo test -p gf2-core --release --doc reorder_rcm --features rand
```

Results:

- `cargo fmt -p gf2-core -- --check`: passed.
- `cargo clippy -p gf2-core --release --lib --features rand -- -D warnings`: passed.
- `cargo nextest ... -E 'test(rcm)'`: 4 tests run, 4 passed, 1833 skipped.
- RowPermutation and reorder_rcm doctests: passed.

The targeted nextest build emitted one pre-existing test-only warning in
`crates/gf2-core/src/gfp/simd_ops.rs` about unreachable code; the command still
completed successfully.

## Criterion amortized benchmark

Command used to populate the root-level Criterion directory consumed by
`ppc-compare.sh`:

```bash
RUSTFLAGS="-C target-cpu=native" \
CARGO_TARGET_DIR=/home/vkaskivuo/Projects/gf2/.claude/worktrees/agent-cbf576d1/target-cbf576d1 \
cargo bench -p gf2-core --bench sparse --features rand -- sparse_matvec_ldpc_rcm_amortized_128
```

The benchmark performs 128 matvecs per Criterion iteration. RCM preprocessing
and input-vector column permutation are performed outside the timed loop.

| case | CSR median | RCM reordered-output median | RCM original-output median | reordered speedup vs CSR |
|---|---:|---:|---:|---:|
| 4096x8192 w6 | 2.317950 ms | 2.644829 ms | 3.277292 ms | 0.876x |
| 8192x16384 w6 | 4.667223 ms | 6.067015 ms | 6.776954 ms | 0.769x |
| 4096x32768 w32 | 10.246525 ms | 10.996919 ms | 11.433697 ms | 0.932x |

Geomean speedup for the D2 gate leaf (`rcm_reordered_output` vs `csr`) is
`0.856x`, i.e. RCM remains slower even after removing the per-call output
unpermutation from the gate leaf.

## `ppc-compare.sh D2` result

Command:

```bash
./dev/benchmarks/ppc-compare.sh D2 --criterion-dir target-cbf576d1/criterion
```

Output summary:

```text
PPC compare — kernel D2 (SparseBitMatrix Cuthill-McKee row reorder)
  bench_target:  sparse_matvec_ldpc_rcm_amortized_128/rcm_reordered_output
  baseline:      current leaf sparse_matvec_ldpc_rcm_amortized_128/csr @ 03b9806d2a8a55bb0dd6228fc8b9fdd768909261
  design sizes:  4096x8192_w6, 8192x16384_w6, 4096x32768_w32
  -----------------------------------------
  size        baseline_ns        new_ns   speedup
  4096x8192_w6    2317950.6     2644829.0    0.876x
  8192x16384_w6    4667223.1     6067014.7    0.769x
  4096x32768_w32   10246524.8    10996918.6    0.932x
  -----------------------------------------
  geomean speedup: 0.856x   (target >= 1.500x)
FAIL
```

Result: the infrastructure mapping is now real and the gate executes, but the
1.5x target is not met.

## `perf stat -r 10` representative before/after

Representative case:
`sparse_matvec_ldpc_rcm_amortized_128/{csr,rcm_reordered_output}/4096x8192_w6`.
Each perf run invokes Criterion with `--warm-up-time 1 --measurement-time 1
--sample-size 10`.

### CSR

Command:

```bash
RUSTFLAGS="-C target-cpu=native" \
CARGO_TARGET_DIR=/home/vkaskivuo/Projects/gf2/.claude/worktrees/agent-cbf576d1/target-cbf576d1 \
perf stat -r 10 -- cargo bench -p gf2-core --bench sparse --features rand -- \
sparse_matvec_ldpc_rcm_amortized_128/csr/4096x8192_w6 \
--warm-up-time 1 --measurement-time 1 --sample-size 10
```

Summary:

```text
Performance counter stats for 'cargo bench ... sparse_matvec_ldpc_rcm_amortized_128/csr/4096x8192_w6 ...' (10 runs):

                 0      context-switches:u
                 0      cpu-migrations:u
            82,669      page-faults:u                    #   6617.2 faults/sec  ( +-  0.09% )
         12,492.96 msec task-clock:u                     #      2.5 CPUs        ( +-  0.57% )
       217,134,976      branch-misses:u                  #      0.7 %           ( +-  6.70% )
    30,733,814,023      branches:u                       #   2460.1 M/sec       ( +-  1.10% )
    51,749,730,326      cpu-cycles:u                     #      4.1 GHz         ( +-  0.61% )
   154,671,236,225      instructions:u                   #      3.0 IPC         ( +-  1.03% )
     1,409,527,332      stalled-cycles-frontend:u        #      0.03            ( +-  3.61% )

       4.923070959 +- 0.056162637 seconds time elapsed  ( +-  1.14% )
```

### RCM reordered output

Command:

```bash
RUSTFLAGS="-C target-cpu=native" \
CARGO_TARGET_DIR=/home/vkaskivuo/Projects/gf2/.claude/worktrees/agent-cbf576d1/target-cbf576d1 \
perf stat -r 10 -- cargo bench -p gf2-core --bench sparse --features rand -- \
sparse_matvec_ldpc_rcm_amortized_128/rcm_reordered_output/4096x8192_w6 \
--warm-up-time 1 --measurement-time 1 --sample-size 10
```

Summary:

```text
Performance counter stats for 'cargo bench ... sparse_matvec_ldpc_rcm_amortized_128/rcm_reordered_output/4096x8192_w6 ...' (10 runs):

                 0      context-switches:u
                 0      cpu-migrations:u
            82,615      page-faults:u                    #   6489.3 faults/sec  ( +-  0.06% )
         12,730.98 msec task-clock:u                     #      2.5 CPUs        ( +-  0.39% )
       259,387,266      branch-misses:u                  #      0.9 %           ( +-  2.72% )
    30,162,140,338      branches:u                       #   2369.2 M/sec       ( +-  0.72% )
    52,778,490,813      cpu-cycles:u                     #      4.1 GHz         ( +-  0.40% )
   152,085,446,889      instructions:u                   #      2.9 IPC         ( +-  0.64% )
     1,549,147,776      stalled-cycles-frontend:u        #      0.03            ( +-  1.97% )

       5.176793898 +- 0.038443797 seconds time elapsed  ( +-  0.74% )
```

Interpretation: removing per-call output unpermutation reduced RCM cost versus
the previous original-output measurement, but the reordered-output RCM path is
still slower than CSR on these deterministic LDPC-like fixtures. The perf
capture shows more branch misses, cycles, and frontend stalls for RCM despite
slightly fewer retired instructions. The `criterion-1.5x` target should be
escalated rather than amended silently.
