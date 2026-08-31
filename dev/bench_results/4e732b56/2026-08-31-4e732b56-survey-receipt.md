# Receipt: external-baseline survey for BCH encoding and generator-matrix materialization

Rendered by `baseline-survey/make-receipt.py` from the run directory; every
figure below is read out of the committed CSVs at render time.

| Field | Value |
|---|---|
| Issue | `4e732b56` |
| Run timestamps (UTC) | 2026-08-31T17:02:00Z, 2026-08-31T17:18:55Z |
| gf2 revision | recorded per stage below; T2N gf2 is an explicitly separate revision |
| Host | AMD Ryzen 9 5900X 12-Core Processor |
| Cores pinned | CCX1 via `dev/scripts/ccx1-bench-flock.sh` (`taskset -c 6-11`; nice -n -5 requested, denied, and the child ran at inherited default priority) |
| Governor | powersave |
| Kernel | Linux fraktaali 7.1.11-arch1-1 #1 SMP PREEMPT_DYNAMIC Fri, 28 Aug 2026 03:36:07 +0000 x86_64 GNU/Linux |
| C compiler | gcc (GCC) 16.2.1 20260810 |
| C++ compiler | g++ (GCC) 16.2.1 20260810 |
| Rust | rustc 1.97.0 (2d8144b78 2026-07-07) |

## Baseline pins

| Pin | Value |
|---|---|
| aff3ct tag | `v4.7.0` |
| aff3ct commit | `e8a65c5047262d97a15563b9edc961f69b2792cc` |
| bchlib tag | `v2.1.3` |
| bchlib commit | `8d0656ab8f37e734428635501738d360ad80eebd` |
| m4ri version | `20260122` |
| m4ri tarball sha256 | `7e033ca1fd36be8861e2f67d9d124c398fc0d830209bb0226462485876346404` |
| itpp release | `4.3.1 (tarball sha256 50717621c5dfb5ed22f8492f8af32b17776e6e06641dfe3a3a8f82c8d353b877)` |
| itpp soname | `8.2.1` |

## Reference build configuration

* `aff3ct library:  -O3 -march=native -funroll-loops -O3 -DNDEBUG -std=gnu++11 -fPIC`
* `aff3ct defines:  -DAFF3CT_EXT_STRINGS -DAFF3CT_MULTI_PREC -DAFF3CT_POLAR_BIT_PACKING -DMIPP_ENABLE_BACKTRACE -DSPU_COLORS -DSPU_STACKTRACE`
* `m4ri library: CFLAGS='-O3 -march=native -fPIC'`

## Per-stage provenance

Each row identifies the committed output, the stage's invocation(s), the basis for
each invocation, the gf2 revision supplying that stage's data, and the host manifest
covering it. Basis is `recorded (log header)` when the tool's own log carries a
`# command:`/`# environment:` header; for a stage whose log predates header emission,
it is `reconstructed` (derived from the runner's argument construction, cited from the
committed invocation-derivation record) or `session-recorded` (executed directly by an
agent session, also cited from that record). For non-gf2 stages the revision is
inherited from the host manifest; a differing data revision is shown as
`data (host: manifest)` in the same row.

| Stage | Output | Provenance log | Exact invocation(s) | Basis | gf2 revision for this stage | Host manifest |
|---|---|---|---|---|---|---|
| `2026-08-31-4e732b56-small-aff3ct` | [2026-08-31-4e732b56-small-aff3ct.csv](2026-08-31-4e732b56-small-aff3ct.csv) | [2026-08-31-4e732b56-small-aff3ct.log](2026-08-31-4e732b56-small-aff3ct.log) | `GF2_SURVEY_CODES=B1,B2,B3,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/aff3ct_bch_bench all`<br>`GF2_SURVEY_CODES=B1,B2,B3 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/aff3ct_bch_bench w2` | reconstructed<br>session-recorded | `3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e` | [2026-08-31-4e732b56-small-host.txt](2026-08-31-4e732b56-small-host.txt) |
| `2026-08-31-4e732b56-small-bchlib` | [2026-08-31-4e732b56-small-bchlib.csv](2026-08-31-4e732b56-small-bchlib.csv) | [2026-08-31-4e732b56-small-bchlib.log](2026-08-31-4e732b56-small-bchlib.log) | `GF2_SURVEY_CODES=B1,B2,B3,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/bchlib_bch_bench` | reconstructed | `3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e` | [2026-08-31-4e732b56-small-host.txt](2026-08-31-4e732b56-small-host.txt) |
| `2026-08-31-4e732b56-small-gf2` | [2026-08-31-4e732b56-small-gf2.csv](2026-08-31-4e732b56-small-gf2.csv) | [2026-08-31-4e732b56-small-gf2.log](2026-08-31-4e732b56-small-gf2.log) | `GF2_SURVEY_CODES=B1,B2,B3,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/.agents/ext/survey-target/release/survey-gf2-side all` | reconstructed | `3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e` | [2026-08-31-4e732b56-small-host.txt](2026-08-31-4e732b56-small-host.txt) |
| `2026-08-31-4e732b56-small-itpp` | [2026-08-31-4e732b56-small-itpp.csv](2026-08-31-4e732b56-small-itpp.csv) | [2026-08-31-4e732b56-small-itpp.log](2026-08-31-4e732b56-small-itpp.log) | `GF2_SURVEY_CODES=B1,B2,B3,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/itpp_bch_bench` | reconstructed | `3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e` | [2026-08-31-4e732b56-small-host.txt](2026-08-31-4e732b56-small-host.txt) |
| `2026-08-31-4e732b56-small-m4ri` | [2026-08-31-4e732b56-small-m4ri.csv](2026-08-31-4e732b56-small-m4ri.csv) | [2026-08-31-4e732b56-small-m4ri.log](2026-08-31-4e732b56-small-m4ri.log) | `GF2_SURVEY_CODES=B1,B2,B3,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/m4ri_genmatrix_bench /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/bench_results/4e732b56/generators.txt all`<br>`GF2_SURVEY_CODES=B1,B2,B3 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/m4ri_genmatrix_bench /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/bench_results/4e732b56/generators.txt all` | reconstructed<br>session-recorded | `3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e` | [2026-08-31-4e732b56-small-host.txt](2026-08-31-4e732b56-small-host.txt) |
| `2026-08-31-4e732b56-t2n-aff3ct` | [2026-08-31-4e732b56-t2n-aff3ct.csv](2026-08-31-4e732b56-t2n-aff3ct.csv) | [2026-08-31-4e732b56-t2n-aff3ct.log](2026-08-31-4e732b56-t2n-aff3ct.log) | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/aff3ct_bch_bench all` | reconstructed | `3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e` | [2026-08-31-4e732b56-t2n-host.txt](2026-08-31-4e732b56-t2n-host.txt) |
| `2026-08-31-4e732b56-t2n-bchlib` | [2026-08-31-4e732b56-t2n-bchlib.csv](2026-08-31-4e732b56-t2n-bchlib.csv) | [2026-08-31-4e732b56-t2n-bchlib.log](2026-08-31-4e732b56-t2n-bchlib.log) | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/bchlib_bch_bench` | reconstructed | `3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e` | [2026-08-31-4e732b56-t2n-host.txt](2026-08-31-4e732b56-t2n-host.txt) |
| `2026-08-31-4e732b56-t2n-gf2` | [2026-08-31-4e732b56-t2n-gf2.csv](2026-08-31-4e732b56-t2n-gf2.csv) | [2026-08-31-4e732b56-t2n-gf2.log](2026-08-31-4e732b56-t2n-gf2.log) | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/.agents/ext/survey-target/release/survey-gf2-side all` | reconstructed | `08e42f096a75309d35255d831782ae38d291f632 (host: 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e)` | [2026-08-31-4e732b56-t2n-host.txt](2026-08-31-4e732b56-t2n-host.txt) |
| `2026-08-31-4e732b56-t2n-itpp` | [2026-08-31-4e732b56-t2n-itpp.csv](2026-08-31-4e732b56-t2n-itpp.csv) | [2026-08-31-4e732b56-t2n-itpp.log](2026-08-31-4e732b56-t2n-itpp.log) | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/itpp_bch_bench` | reconstructed | `3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e` | [2026-08-31-4e732b56-t2n-host.txt](2026-08-31-4e732b56-t2n-host.txt) |
| `2026-08-31-4e732b56-t2n-m4ri` | [2026-08-31-4e732b56-t2n-m4ri.csv](2026-08-31-4e732b56-t2n-m4ri.csv) | [2026-08-31-4e732b56-t2n-m4ri.log](2026-08-31-4e732b56-t2n-m4ri.log) | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/m4ri_genmatrix_bench /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/bench_results/4e732b56/generators.txt all` | reconstructed | `3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e` | [2026-08-31-4e732b56-t2n-host.txt](2026-08-31-4e732b56-t2n-host.txt) |
| `2026-08-31-4e732b56-small-aff3ct-perf` | (perf counters) | [2026-08-31-4e732b56-small-aff3ct-perf-stat.txt](2026-08-31-4e732b56-small-aff3ct-perf-stat.txt) | `GF2_SURVEY_CODES=B1,B2,B3,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh perf stat -e task-clock,cycles,instructions,branches,branch-misses,cache-references,cache-misses /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/aff3ct_bch_bench w1` | reconstructed | `3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e` | [2026-08-31-4e732b56-small-host.txt](2026-08-31-4e732b56-small-host.txt) |
| `2026-08-31-4e732b56-t2n-bchlib-perf` | (perf counters) | [2026-08-31-4e732b56-t2n-bchlib-perf-stat.txt](2026-08-31-4e732b56-t2n-bchlib-perf-stat.txt) | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh perf stat -e task-clock,cycles,instructions,branches,branch-misses,cache-references,cache-misses /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/bchlib_bch_bench` | reconstructed | `3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e` | [2026-08-31-4e732b56-t2n-host.txt](2026-08-31-4e732b56-t2n-host.txt) |

## Measured cells

Throughput is information bits per second for W1 and matrix bits per second
for W2. `Trials` is the number of independent trials the cell's wall budget
allowed; a cell marked *estimate* was projected from a measured per-unit cost
and was never run at that size.

| Workload | Row | Batch | Library | Algorithm | Trials | Median | Min | Max | Spread |
|---|---|---|---|---|---|---|---|---|---|
| W1 | B1 | 1 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 89.97 | 83.55 | 90.91 | 8.2% |
| W1 | B1 | 1 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 4.16 | 3.84 | 4.54 | 16.8% |
| W1 | B1 | 1 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 4.50 | 4.47 | 4.52 | 1.2% |
| W1 | B1 | 1 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 3.85 | 3.53 | 3.89 | 9.4% |
| W1 | B1 | 16 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 102.97 | 93.55 | 103.69 | 9.8% |
| W1 | B1 | 16 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 391.32 | 384.31 | 401.08 | 4.3% |
| W1 | B1 | 16 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 7.00 | 6.64 | 7.01 | 5.2% |
| W1 | B1 | 16 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 6.79 | 6.63 | 6.86 | 3.3% |
| W1 | B1 | 16 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 5.54 | 5.14 | 5.60 | 8.3% |
| W1 | B1 | 256 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 102.53 | 101.39 | 102.56 | 1.1% |
| W1 | B1 | 256 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 394.19 | 390.69 | 400.56 | 2.5% |
| W1 | B1 | 256 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 5.91 | 5.90 | 5.92 | 0.3% |
| W1 | B1 | 256 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 5.90 | 5.89 | 5.92 | 0.5% |
| W1 | B1 | 256 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 6.25 | 6.24 | 6.25 | 0.2% |
| W1 | B1 | 4096 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 99.90 | 99.70 | 100.26 | 0.6% |
| W1 | B1 | 4096 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 370.17 | 368.92 | 371.09 | 0.6% |
| W1 | B1 | 4096 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 6.14 | 6.11 | 6.16 | 0.9% |
| W1 | B1 | 4096 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 6.13 | 6.03 | 6.17 | 2.2% |
| W1 | B1 | 4096 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 6.22 | 6.19 | 6.23 | 0.7% |
| W1 | B2 | 1 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 86.82 | 86.06 | 88.26 | 2.5% |
| W1 | B2 | 1 | bchlib v2.1.3 | `table-remainder` | 7 | 1993.27 | 1626.21 | 2044.15 | 21.0% |
| W1 | B2 | 1 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 1.77 | 1.74 | 1.80 | 3.1% |
| W1 | B2 | 1 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 1.75 | 1.74 | 1.76 | 1.5% |
| W1 | B2 | 1 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.73 | 0.73 | 0.74 | 0.8% |
| W1 | B2 | 16 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 87.33 | 85.02 | 87.72 | 3.1% |
| W1 | B2 | 16 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 180.73 | 177.18 | 182.89 | 3.2% |
| W1 | B2 | 16 | bchlib v2.1.3 | `table-remainder` | 7 | 2036.31 | 1966.40 | 2239.01 | 13.4% |
| W1 | B2 | 16 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 1.95 | 1.94 | 1.97 | 1.4% |
| W1 | B2 | 16 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 1.97 | 1.96 | 1.98 | 0.8% |
| W1 | B2 | 16 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.67 | 0.67 | 0.67 | 0.1% |
| W1 | B2 | 256 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 78.96 | 78.07 | 79.40 | 1.7% |
| W1 | B2 | 256 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 178.75 | 175.33 | 180.87 | 3.1% |
| W1 | B2 | 256 | bchlib v2.1.3 | `table-remainder` | 7 | 2259.42 | 2248.81 | 2266.24 | 0.8% |
| W1 | B2 | 256 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 1.88 | 1.88 | 1.91 | 1.5% |
| W1 | B2 | 256 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 1.90 | 1.89 | 1.91 | 1.1% |
| W1 | B2 | 256 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.66 | 0.66 | 0.66 | 0.5% |
| W1 | B2 | 4096 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 75.98 | 75.31 | 76.39 | 1.4% |
| W1 | B2 | 4096 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 159.77 | 153.12 | 161.80 | 5.4% |
| W1 | B2 | 4096 | bchlib v2.1.3 | `table-remainder` | 7 | 2240.90 | 2235.24 | 2251.43 | 0.7% |
| W1 | B2 | 4096 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 1.90 | 1.89 | 1.91 | 0.9% |
| W1 | B2 | 4096 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 1.90 | 1.90 | 1.91 | 0.9% |
| W1 | B2 | 4096 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.66 | 0.65 | 0.66 | 0.6% |
| W1 | B3 | 1 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 112.56 | 111.36 | 113.28 | 1.7% |
| W1 | B3 | 1 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 3.25 | 3.21 | 3.27 | 2.0% |
| W1 | B3 | 1 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 3.24 | 3.23 | 3.28 | 1.4% |
| W1 | B3 | 1 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.35 | 0.35 | 0.35 | 0.9% |
| W1 | B3 | 16 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 98.37 | 98.10 | 99.17 | 1.1% |
| W1 | B3 | 16 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 443.08 | 432.58 | 451.59 | 4.3% |
| W1 | B3 | 16 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 3.51 | 3.49 | 3.57 | 2.3% |
| W1 | B3 | 16 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 3.49 | 3.47 | 3.51 | 1.1% |
| W1 | B3 | 16 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.37 | 0.36 | 0.37 | 0.7% |
| W1 | B3 | 256 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 97.85 | 96.57 | 97.94 | 1.4% |
| W1 | B3 | 256 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 437.37 | 432.09 | 439.87 | 1.8% |
| W1 | B3 | 256 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 3.39 | 3.37 | 3.41 | 1.3% |
| W1 | B3 | 256 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 3.42 | 3.41 | 3.42 | 0.5% |
| W1 | B3 | 256 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.36 | 0.36 | 0.36 | 0.4% |
| W1 | B3 | 4096 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 96.72 | 96.63 | 97.67 | 1.1% |
| W1 | B3 | 4096 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 432.48 | 430.62 | 434.12 | 0.8% |
| W1 | B3 | 4096 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 3.43 | 3.42 | 3.43 | 0.4% |
| W1 | B3 | 4096 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 3.43 | 3.42 | 3.43 | 0.4% |
| W1 | B3 | 4096 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.39 | 0.36 | 0.39 | 8.4% |
| W1 | T2N | 1 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 38.61 | 38.52 | 38.63 | 0.3% |
| W1 | T2N | 1 | bchlib v2.1.3 | `table-remainder` | 7 | 4636.90 | 4188.23 | 4671.61 | 10.4% |
| W1 | T2N | 1 | gf2 08e42f096a75309d35255d831782ae38d291f632 | `encode-batch` | 7 | 0.67 | 0.67 | 0.68 | 1.2% |
| W1 | T2N | 1 | gf2 08e42f096a75309d35255d831782ae38d291f632 | `encode-loop` | 7 | 0.68 | 0.67 | 0.69 | 2.2% |
| W1 | T2N | 16 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 38.67 | 38.57 | 38.79 | 0.6% |
| W1 | T2N | 16 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 41.15 | 40.20 | 43.72 | 8.6% |
| W1 | T2N | 16 | bchlib v2.1.3 | `table-remainder` | 7 | 4650.60 | 4647.78 | 4654.58 | 0.1% |
| W1 | T2N | 16 | gf2 08e42f096a75309d35255d831782ae38d291f632 | `encode-batch` | 7 | 0.68 | 0.67 | 0.68 | 0.3% |
| W1 | T2N | 16 | gf2 08e42f096a75309d35255d831782ae38d291f632 | `encode-loop` | 7 | 0.68 | 0.67 | 0.68 | 1.1% |
| W1 | T2N | 256 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 38.39 | 38.23 | 38.53 | 0.8% |
| W1 | T2N | 256 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 41.39 | 41.22 | 41.95 | 1.8% |
| W1 | T2N | 256 | bchlib v2.1.3 | `table-remainder` | 7 | 4644.85 | 4640.69 | 4646.95 | 0.1% |
| W1 | T2N | 256 | gf2 08e42f096a75309d35255d831782ae38d291f632 | `encode-batch` | 7 | 0.68 | 0.68 | 0.68 | 0.1% |
| W1 | T2N | 256 | gf2 08e42f096a75309d35255d831782ae38d291f632 | `encode-loop` | 7 | 0.68 | 0.68 | 0.68 | 0.9% |
| W1 | T2N | 4096 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 38.37 | 38.31 | 38.39 | 0.2% |
| W1 | T2N | 4096 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 41.57 | 41.47 | 41.81 | 0.8% |
| W1 | T2N | 4096 | bchlib v2.1.3 | `table-remainder` | 7 | 4672.27 | 4638.78 | 4680.73 | 0.9% |
| W1 | T2N | 4096 | gf2 08e42f096a75309d35255d831782ae38d291f632 | `encode-batch-projected` | *estimate* | 0.65 | — | — | — |
| W1 | T2S | 1 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 45.04 | 44.83 | 45.21 | 0.8% |
| W1 | T2S | 1 | bchlib v2.1.3 | `table-remainder` | 7 | 4575.20 | 4552.78 | 4604.07 | 1.1% |
| W1 | T2S | 1 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 0.78 | 0.78 | 0.79 | 1.3% |
| W1 | T2S | 1 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 0.79 | 0.78 | 0.79 | 1.4% |
| W1 | T2S | 16 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 42.87 | 42.82 | 43.22 | 0.9% |
| W1 | T2S | 16 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 51.99 | 51.85 | 52.28 | 0.8% |
| W1 | T2S | 16 | bchlib v2.1.3 | `table-remainder` | 7 | 4567.90 | 4551.30 | 4588.83 | 0.8% |
| W1 | T2S | 16 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 0.77 | 0.77 | 0.78 | 1.1% |
| W1 | T2S | 16 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 0.77 | 0.76 | 0.78 | 1.7% |
| W1 | T2S | 256 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 42.85 | 41.88 | 43.11 | 2.9% |
| W1 | T2S | 256 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 68.57 | 68.40 | 69.21 | 1.2% |
| W1 | T2S | 256 | bchlib v2.1.3 | `table-remainder` | 7 | 4544.95 | 4533.26 | 4547.55 | 0.3% |
| W1 | T2S | 256 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 0.77 | 0.76 | 0.77 | 0.9% |
| W1 | T2S | 256 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 0.76 | 0.76 | 0.76 | 0.2% |
| W1 | T2S | 4096 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 42.62 | 42.54 | 42.69 | 0.4% |
| W1 | T2S | 4096 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 50.78 | 50.59 | 50.82 | 0.4% |
| W1 | T2S | 4096 | bchlib v2.1.3 | `table-remainder` | 7 | 4533.24 | 4529.20 | 4559.91 | 0.7% |
| W1 | T2S | 4096 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 3 | 0.76 | 0.76 | 0.76 | 0.1% |
| W1 | T2S | 4096 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 3 | 0.76 | 0.76 | 0.76 | 0.1% |
| W2 | B1 | 5 | aff3ct v4.7.0 | `basis-encode-pack` | 7 | 264.05 | 174.08 | 265.75 | 34.7% |
| W2 | B1 | 5 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `generator-matrix` | 7 | 6.26 | 6.22 | 6.29 | 1.2% |
| W2 | B1 | 5 | m4ri 20260122 | `echelonize` | 7 | 276.45 | 189.56 | 279.06 | 32.4% |
| W2 | B1 | 5 | m4ri 20260122 | `genmatrix-rref` | 7 | 281.03 | 215.86 | 308.84 | 33.1% |
| W2 | B1 | 256 | m4ri 20260122 | `matmul-m4rm` | 7 | 112.73 | 85.25 | 115.94 | 27.2% |
| W2 | B1 | 4096 | m4ri 20260122 | `matmul-m4rm` | 7 | 141.30 | 120.26 | 156.58 | 25.7% |
| W2 | B2 | 64 | aff3ct v4.7.0 | `basis-encode-pack` | 7 | 150.65 | 119.47 | 160.68 | 27.4% |
| W2 | B2 | 64 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `generator-matrix` | 7 | 6.92 | 6.91 | 6.98 | 1.1% |
| W2 | B2 | 64 | m4ri 20260122 | `echelonize` | 7 | 1491.07 | 1212.18 | 1507.50 | 19.8% |
| W2 | B2 | 64 | m4ri 20260122 | `genmatrix-rref` | 7 | 1291.09 | 1056.08 | 1351.13 | 22.9% |
| W2 | B2 | 256 | m4ri 20260122 | `matmul-m4rm` | 7 | 2147.38 | 1792.16 | 2174.08 | 17.8% |
| W2 | B2 | 4096 | m4ri 20260122 | `matmul-m4rm` | 7 | 2549.66 | 2353.84 | 2672.81 | 12.5% |
| W2 | B3 | 223 | aff3ct v4.7.0 | `basis-encode-pack` | 7 | 100.25 | 71.92 | 114.02 | 42.0% |
| W2 | B3 | 223 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `generator-matrix` | 7 | 7.29 | 7.23 | 7.47 | 3.3% |
| W2 | B3 | 223 | m4ri 20260122 | `echelonize` | 7 | 1476.09 | 1286.47 | 1551.60 | 18.0% |
| W2 | B3 | 223 | m4ri 20260122 | `genmatrix-rref` | 7 | 1733.73 | 1500.88 | 1785.68 | 16.4% |
| W2 | B3 | 256 | m4ri 20260122 | `matmul-m4rm` | 7 | 2347.16 | 2110.56 | 2397.10 | 12.2% |
| W2 | B3 | 4096 | m4ri 20260122 | `matmul-m4rm` | 7 | 2918.71 | 2378.27 | 2990.96 | 21.0% |
| W2 | T2N | 256 | m4ri 20260122 | `matmul-m4rm` | 7 | 84.66 | 84.24 | 85.04 | 0.9% |
| W2 | T2N | 4096 | m4ri 20260122 | `matmul-m4rm` | 7 | 93.62 | 90.06 | 93.87 | 4.1% |
| W2 | T2N | 32208 | aff3ct v4.7.0 | `basis-encode-pack` | 4 | 41.23 | 41.17 | 41.27 | 0.2% |
| W2 | T2N | 32208 | gf2 08e42f096a75309d35255d831782ae38d291f632 | `generator-matrix-projected` | *estimate* | 0.66 | — | — | — |
| W2 | T2N | 32208 | m4ri 20260122 | `echelonize` | 7 | 175.22 | 172.27 | 176.90 | 2.6% |
| W2 | T2N | 32208 | m4ri 20260122 | `genmatrix-rref` | 7 | 446.82 | 430.33 | 447.35 | 3.8% |
| W2 | T2S | 256 | m4ri 20260122 | `matmul-m4rm` | 7 | 339.91 | 336.56 | 340.18 | 1.1% |
| W2 | T2S | 4096 | m4ri 20260122 | `matmul-m4rm` | 7 | 457.52 | 454.15 | 460.09 | 1.3% |
| W2 | T2S | 7032 | aff3ct v4.7.0 | `basis-encode-pack` | 7 | 48.28 | 48.23 | 48.33 | 0.2% |
| W2 | T2S | 7032 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `generator-matrix` | 3 | 1.58 | 1.58 | 1.58 | 0.1% |
| W2 | T2S | 7032 | m4ri 20260122 | `echelonize` | 7 | 788.77 | 786.37 | 793.32 | 0.9% |
| W2 | T2S | 7032 | m4ri 20260122 | `genmatrix-rref` | 7 | 1754.17 | 1747.76 | 1768.44 | 1.2% |

## Files

| File | SHA-256 | Bytes |
|---|---|---|
| `2026-08-31-4e732b56-determinism-agreement.txt` | `6af6b89771a8854b…` | 11975 |
| `2026-08-31-4e732b56-generator-agreement.txt` | `ccd3474e8296a67e…` | 170 |
| `2026-08-31-4e732b56-generator-matrix-agreement.txt` | `209a7faba6396fea…` | 1566 |
| `2026-08-31-4e732b56-invocation-derivation.md` | `193da0ae8c2915cc…` | 7261 |
| `2026-08-31-4e732b56-kodo-retrieval.txt` | `cecfeef3993e7730…` | 6679 |
| `2026-08-31-4e732b56-m4ri-thread-evidence.txt` | `f6ecb9472812db92…` | 3258 |
| `2026-08-31-4e732b56-small-aff3ct-perf-stat.txt` | `c5444ff8a39b37c9…` | 3213 |
| `2026-08-31-4e732b56-small-aff3ct.csv` | `1a590dd775613b35…` | 18976 |
| `2026-08-31-4e732b56-small-aff3ct.log` | `e50f696856fd3508…` | 3062 |
| `2026-08-31-4e732b56-small-bchlib.csv` | `ae2f68ed79a55681…` | 5046 |
| `2026-08-31-4e732b56-small-bchlib.log` | `b5ef3a892b1d30bd…` | 919 |
| `2026-08-31-4e732b56-small-gf2.csv` | `ed806f9235b0dd41…` | 27312 |
| `2026-08-31-4e732b56-small-gf2.log` | `38967f1be0f4decc…` | 2961 |
| `2026-08-31-4e732b56-small-host.txt` | `48d874f6602133c5…` | 6947 |
| `2026-08-31-4e732b56-small-itpp.csv` | `144d658f7610b981…` | 8175 |
| `2026-08-31-4e732b56-small-itpp.log` | `d73de26d9321da47…` | 1053 |
| `2026-08-31-4e732b56-small-m4ri.csv` | `55fe110f555e4892…` | 9407 |
| `2026-08-31-4e732b56-small-m4ri.log` | `022e9707c7bb5c52…` | 1886 |
| `2026-08-31-4e732b56-t2n-aff3ct.csv` | `68dac5879e0c5d61…` | 4962 |
| `2026-08-31-4e732b56-t2n-aff3ct.log` | `2b23d6cdad68bcc3…` | 780 |
| `2026-08-31-4e732b56-t2n-bchlib-perf-stat.txt` | `41549b5083a6a249…` | 1393 |
| `2026-08-31-4e732b56-t2n-bchlib.csv` | `471e8b8d959eb79b…` | 2708 |
| `2026-08-31-4e732b56-t2n-bchlib.log` | `a4262934c26f034c…` | 470 |
| `2026-08-31-4e732b56-t2n-gf2.csv` | `9f0b2209b3cdc731…` | 5468 |
| `2026-08-31-4e732b56-t2n-gf2.log` | `37359a00592290c9…` | 939 |
| `2026-08-31-4e732b56-t2n-host.txt` | `fdbe9cf0695132e6…` | 6938 |
| `2026-08-31-4e732b56-t2n-itpp.csv` | `eebab57153ce9b42…` | 90 |
| `2026-08-31-4e732b56-t2n-itpp.log` | `d2f9b2a8b59c9a6c…` | 275 |
| `2026-08-31-4e732b56-t2n-m4ri.csv` | `1a9b71ddc4d9e46c…` | 2683 |
| `2026-08-31-4e732b56-t2n-m4ri.log` | `6d3eff1f308e6301…` | 409 |
| `generators.txt` | `612d5c84ee02ff69…` | 551 |

## Reproduction

```
dev/active/4e732b56/baseline-survey/fetch-build.sh
dev/active/4e732b56/baseline-survey/run-survey.sh dev/bench_results/4e732b56 <codes> <prefix>
dev/active/4e732b56/baseline-survey/make-receipt.py dev/bench_results/4e732b56
```
