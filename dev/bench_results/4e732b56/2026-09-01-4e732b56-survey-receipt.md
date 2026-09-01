# Receipt: external-baseline survey for BCH encoding and generator-matrix materialization

Rendered by `baseline-survey/make-receipt.py` from the run directory; every
figure below is read out of the committed CSVs at render time.

| Field | Value |
|---|---|
| Issue | `4e732b56` |
| Run timestamps (UTC) | 2026-08-31T21:33:28Z, 2026-08-31T21:33:28Z |
| gf2 revision | recorded per stage below |
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
| `2026-09-01-4e732b56-small-aff3ct` | [2026-09-01-4e732b56-small-aff3ct.csv](2026-09-01-4e732b56-small-aff3ct.csv) | [2026-09-01-4e732b56-small-aff3ct.log](2026-09-01-4e732b56-small-aff3ct.log) | `GF2_SURVEY_CODES=B1\,B2\,B3\,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/aff3ct_bch_bench all` | recorded (log header) | `c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae` | [2026-09-01-4e732b56-small-host.txt](2026-09-01-4e732b56-small-host.txt) |
| `2026-09-01-4e732b56-small-bchlib` | [2026-09-01-4e732b56-small-bchlib.csv](2026-09-01-4e732b56-small-bchlib.csv) | [2026-09-01-4e732b56-small-bchlib.log](2026-09-01-4e732b56-small-bchlib.log) | `GF2_SURVEY_CODES=B1\,B2\,B3\,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/bchlib_bch_bench` | recorded (log header) | `c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae` | [2026-09-01-4e732b56-small-host.txt](2026-09-01-4e732b56-small-host.txt) |
| `2026-09-01-4e732b56-small-gf2` | [2026-09-01-4e732b56-small-gf2.csv](2026-09-01-4e732b56-small-gf2.csv) | [2026-09-01-4e732b56-small-gf2.log](2026-09-01-4e732b56-small-gf2.log) | `GF2_SURVEY_CODES=B1\,B2\,B3\,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/target/survey-target/release/survey-gf2-side all` | recorded (log header) | `c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae` | [2026-09-01-4e732b56-small-host.txt](2026-09-01-4e732b56-small-host.txt) |
| `2026-09-01-4e732b56-small-itpp` | [2026-09-01-4e732b56-small-itpp.csv](2026-09-01-4e732b56-small-itpp.csv) | [2026-09-01-4e732b56-small-itpp.log](2026-09-01-4e732b56-small-itpp.log) | `GF2_SURVEY_CODES=B1\,B2\,B3\,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/itpp_bch_bench` | recorded (log header) | `c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae` | [2026-09-01-4e732b56-small-host.txt](2026-09-01-4e732b56-small-host.txt) |
| `2026-09-01-4e732b56-small-m4ri` | [2026-09-01-4e732b56-small-m4ri.csv](2026-09-01-4e732b56-small-m4ri.csv) | [2026-09-01-4e732b56-small-m4ri.log](2026-09-01-4e732b56-small-m4ri.log) | `GF2_SURVEY_CODES=B1\,B2\,B3\,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/m4ri_genmatrix_bench dev/bench_results/4e732b56/generators.txt all` | recorded (log header) | `c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae` | [2026-09-01-4e732b56-small-host.txt](2026-09-01-4e732b56-small-host.txt) |
| `2026-09-01-4e732b56-t2n-aff3ct` | [2026-09-01-4e732b56-t2n-aff3ct.csv](2026-09-01-4e732b56-t2n-aff3ct.csv) | [2026-09-01-4e732b56-t2n-aff3ct.log](2026-09-01-4e732b56-t2n-aff3ct.log) | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/aff3ct_bch_bench all` | recorded (log header) | `c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae` | [2026-09-01-4e732b56-t2n-host.txt](2026-09-01-4e732b56-t2n-host.txt) |
| `2026-09-01-4e732b56-t2n-bchlib` | [2026-09-01-4e732b56-t2n-bchlib.csv](2026-09-01-4e732b56-t2n-bchlib.csv) | [2026-09-01-4e732b56-t2n-bchlib.log](2026-09-01-4e732b56-t2n-bchlib.log) | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/bchlib_bch_bench` | recorded (log header) | `c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae` | [2026-09-01-4e732b56-t2n-host.txt](2026-09-01-4e732b56-t2n-host.txt) |
| `2026-09-01-4e732b56-t2n-gf2` | [2026-09-01-4e732b56-t2n-gf2.csv](2026-09-01-4e732b56-t2n-gf2.csv) | [2026-09-01-4e732b56-t2n-gf2.log](2026-09-01-4e732b56-t2n-gf2.log) | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/target/survey-target/release/survey-gf2-side all` | recorded (log header) | `c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae` | [2026-09-01-4e732b56-t2n-host.txt](2026-09-01-4e732b56-t2n-host.txt) |
| `2026-09-01-4e732b56-t2n-itpp` | [2026-09-01-4e732b56-t2n-itpp.csv](2026-09-01-4e732b56-t2n-itpp.csv) | [2026-09-01-4e732b56-t2n-itpp.log](2026-09-01-4e732b56-t2n-itpp.log) | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/itpp_bch_bench` | recorded (log header) | `c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae` | [2026-09-01-4e732b56-t2n-host.txt](2026-09-01-4e732b56-t2n-host.txt) |
| `2026-09-01-4e732b56-t2n-m4ri` | [2026-09-01-4e732b56-t2n-m4ri.csv](2026-09-01-4e732b56-t2n-m4ri.csv) | [2026-09-01-4e732b56-t2n-m4ri.log](2026-09-01-4e732b56-t2n-m4ri.log) | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/m4ri_genmatrix_bench dev/bench_results/4e732b56/generators.txt all` | recorded (log header) | `c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae` | [2026-09-01-4e732b56-t2n-host.txt](2026-09-01-4e732b56-t2n-host.txt) |
| `2026-09-01-4e732b56-small-aff3ct-perf` | (perf counters) | [2026-09-01-4e732b56-small-aff3ct-perf-stat.txt](2026-09-01-4e732b56-small-aff3ct-perf-stat.txt) | `GF2_SURVEY_CODES=B1\,B2\,B3\,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh perf stat -e task-clock\,cycles\,instructions\,branches\,branch-misses\,cache-references\,cache-misses /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/aff3ct_bch_bench w1` | recorded (log header) | `c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae` | [2026-09-01-4e732b56-small-host.txt](2026-09-01-4e732b56-small-host.txt) |
| `2026-09-01-4e732b56-t2n-aff3ct-perf` | (perf counters) | [2026-09-01-4e732b56-t2n-aff3ct-perf-stat.txt](2026-09-01-4e732b56-t2n-aff3ct-perf-stat.txt) | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh perf stat -e task-clock\,cycles\,instructions\,branches\,branch-misses\,cache-references\,cache-misses /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/aff3ct_bch_bench w1` | recorded (log header) | `c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae` | [2026-09-01-4e732b56-t2n-host.txt](2026-09-01-4e732b56-t2n-host.txt) |

## Measured cells

Throughput is information bits per second for W1 and matrix bits per second
for W2. `Trials` is the number of independent trials the cell's wall budget
allowed; a cell marked *estimate* was projected from a measured per-unit cost
and was never run at that size.

| Workload | Row | Batch | Library | Algorithm | Trials | Median | Min | Max | Spread |
|---|---|---|---|---|---|---|---|---|---|
| W1 | B1 | 1 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 84.70 | 74.69 | 89.87 | 17.9% |
| W1 | B1 | 1 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-batch` | 7 | 4.44 | 3.08 | 4.57 | 33.6% |
| W1 | B1 | 1 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-loop` | 7 | 4.15 | 3.57 | 4.49 | 22.2% |
| W1 | B1 | 1 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 3.45 | 2.94 | 3.77 | 24.0% |
| W1 | B1 | 16 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 91.82 | 75.43 | 102.18 | 29.1% |
| W1 | B1 | 16 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 363.97 | 281.75 | 392.35 | 30.4% |
| W1 | B1 | 16 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-batch` | 7 | 5.99 | 5.48 | 6.96 | 24.6% |
| W1 | B1 | 16 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-loop` | 7 | 5.98 | 5.19 | 6.71 | 25.4% |
| W1 | B1 | 16 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 4.82 | 4.27 | 5.42 | 24.0% |
| W1 | B1 | 256 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 91.58 | 88.40 | 98.30 | 10.8% |
| W1 | B1 | 256 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 341.75 | 315.31 | 392.28 | 22.5% |
| W1 | B1 | 256 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-batch` | 7 | 5.63 | 4.74 | 5.73 | 17.6% |
| W1 | B1 | 256 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-loop` | 7 | 5.16 | 4.80 | 5.87 | 20.8% |
| W1 | B1 | 256 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 5.35 | 4.50 | 6.13 | 30.5% |
| W1 | B1 | 4096 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 92.60 | 84.91 | 98.94 | 15.2% |
| W1 | B1 | 4096 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 349.17 | 322.69 | 372.61 | 14.3% |
| W1 | B1 | 4096 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-batch` | 7 | 5.72 | 4.81 | 6.09 | 22.3% |
| W1 | B1 | 4096 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-loop` | 7 | 5.36 | 5.05 | 6.08 | 19.3% |
| W1 | B1 | 4096 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 5.37 | 4.59 | 6.08 | 27.7% |
| W1 | B2 | 1 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 89.33 | 77.74 | 91.84 | 15.8% |
| W1 | B2 | 1 | bchlib v2.1.3 | `table-remainder` | 7 | 1983.68 | 1149.31 | 2057.06 | 45.8% |
| W1 | B2 | 1 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-batch` | 7 | 1.49 | 1.41 | 1.75 | 22.8% |
| W1 | B2 | 1 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-loop` | 7 | 1.50 | 1.37 | 1.73 | 24.0% |
| W1 | B2 | 1 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.63 | 0.55 | 0.72 | 27.2% |
| W1 | B2 | 16 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 87.96 | 77.08 | 90.61 | 15.4% |
| W1 | B2 | 16 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 168.50 | 160.75 | 186.91 | 15.5% |
| W1 | B2 | 16 | bchlib v2.1.3 | `table-remainder` | 7 | 1905.06 | 1419.01 | 2183.12 | 40.1% |
| W1 | B2 | 16 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-batch` | 7 | 1.67 | 1.51 | 1.94 | 25.8% |
| W1 | B2 | 16 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-loop` | 7 | 1.87 | 1.66 | 1.95 | 15.6% |
| W1 | B2 | 16 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.56 | 0.53 | 0.66 | 22.6% |
| W1 | B2 | 256 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 79.47 | 61.54 | 85.84 | 30.6% |
| W1 | B2 | 256 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 174.76 | 162.24 | 188.88 | 15.2% |
| W1 | B2 | 256 | bchlib v2.1.3 | `table-remainder` | 7 | 1853.52 | 1734.53 | 2210.43 | 25.7% |
| W1 | B2 | 256 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-batch` | 7 | 1.73 | 1.56 | 1.86 | 16.8% |
| W1 | B2 | 256 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-loop` | 7 | 1.69 | 1.55 | 1.86 | 18.2% |
| W1 | B2 | 256 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.57 | 0.53 | 0.59 | 10.0% |
| W1 | B2 | 4096 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 71.78 | 66.71 | 80.79 | 19.6% |
| W1 | B2 | 4096 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 162.32 | 155.96 | 171.26 | 9.4% |
| W1 | B2 | 4096 | bchlib v2.1.3 | `table-remainder` | 7 | 1931.90 | 1698.80 | 2204.73 | 26.2% |
| W1 | B2 | 4096 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-batch` | 7 | 1.71 | 1.69 | 1.72 | 1.9% |
| W1 | B2 | 4096 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-loop` | 7 | 1.69 | 1.67 | 1.71 | 2.2% |
| W1 | B2 | 4096 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.58 | 0.57 | 0.59 | 2.7% |
| W1 | B3 | 1 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 106.71 | 84.13 | 109.46 | 23.7% |
| W1 | B3 | 1 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-batch` | 7 | 3.00 | 2.77 | 3.20 | 14.3% |
| W1 | B3 | 1 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-loop` | 7 | 2.89 | 2.57 | 3.20 | 21.8% |
| W1 | B3 | 1 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.34 | 0.29 | 0.37 | 25.3% |
| W1 | B3 | 16 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 91.76 | 66.11 | 97.08 | 33.7% |
| W1 | B3 | 16 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 381.96 | 299.69 | 431.11 | 34.4% |
| W1 | B3 | 16 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-batch` | 7 | 3.18 | 2.87 | 3.47 | 19.1% |
| W1 | B3 | 16 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-loop` | 7 | 3.09 | 3.02 | 3.47 | 14.3% |
| W1 | B3 | 16 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.36 | 0.34 | 0.37 | 9.7% |
| W1 | B3 | 256 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 84.55 | 71.51 | 93.07 | 25.5% |
| W1 | B3 | 256 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 393.84 | 336.47 | 425.06 | 22.5% |
| W1 | B3 | 256 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-batch` | 7 | 3.08 | 3.00 | 3.17 | 5.5% |
| W1 | B3 | 256 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-loop` | 7 | 3.11 | 3.02 | 3.19 | 5.3% |
| W1 | B3 | 256 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.35 | 0.34 | 0.35 | 3.4% |
| W1 | B3 | 4096 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 83.74 | 74.50 | 89.43 | 17.8% |
| W1 | B3 | 4096 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 381.32 | 325.28 | 426.55 | 26.6% |
| W1 | B3 | 4096 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-batch` | 7 | 3.11 | 3.09 | 3.12 | 1.2% |
| W1 | B3 | 4096 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-loop` | 7 | 3.10 | 3.08 | 3.12 | 1.2% |
| W1 | B3 | 4096 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.34 | 0.34 | 0.35 | 1.4% |
| W1 | T2N | 1 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 36.44 | 30.07 | 37.97 | 21.7% |
| W1 | T2N | 1 | bchlib v2.1.3 | `table-remainder` | 7 | 3765.31 | 3272.78 | 4560.32 | 34.2% |
| W1 | T2N | 1 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-batch` | 7 | 0.60 | 0.59 | 0.62 | 5.5% |
| W1 | T2N | 1 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-loop` | 7 | 0.60 | 0.59 | 0.61 | 3.1% |
| W1 | T2N | 16 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 34.59 | 32.76 | 36.08 | 9.6% |
| W1 | T2N | 16 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 52.89 | 51.83 | 59.72 | 14.9% |
| W1 | T2N | 16 | bchlib v2.1.3 | `table-remainder` | 7 | 4078.32 | 3354.81 | 4553.97 | 29.4% |
| W1 | T2N | 16 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-batch` | 7 | 0.60 | 0.60 | 0.61 | 1.4% |
| W1 | T2N | 16 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-loop` | 7 | 0.60 | 0.59 | 0.60 | 1.3% |
| W1 | T2N | 256 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 34.21 | 33.58 | 34.61 | 3.0% |
| W1 | T2N | 256 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 54.99 | 54.52 | 55.14 | 1.1% |
| W1 | T2N | 256 | bchlib v2.1.3 | `table-remainder` | 7 | 4226.42 | 3495.30 | 4546.10 | 24.9% |
| W1 | T2N | 256 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-batch` | 7 | 0.60 | 0.60 | 0.60 | 0.5% |
| W1 | T2N | 256 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-loop` | 7 | 0.60 | 0.60 | 0.61 | 0.4% |
| W1 | T2N | 4096 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 33.70 | 32.87 | 34.75 | 5.6% |
| W1 | T2N | 4096 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 54.61 | 53.86 | 55.15 | 2.4% |
| W1 | T2N | 4096 | bchlib v2.1.3 | `table-remainder` | 7 | 4063.82 | 3916.12 | 4144.96 | 5.6% |
| W1 | T2N | 4096 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-batch-projected` | *estimate* | 0.57 | — | — | — |
| W1 | T2S | 1 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 41.51 | 38.53 | 44.02 | 13.2% |
| W1 | T2S | 1 | bchlib v2.1.3 | `table-remainder` | 7 | 4062.24 | 3702.91 | 4379.39 | 16.7% |
| W1 | T2S | 1 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-batch` | 7 | 0.68 | 0.63 | 0.73 | 15.1% |
| W1 | T2S | 1 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-loop` | 7 | 0.66 | 0.64 | 0.75 | 17.5% |
| W1 | T2S | 16 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 40.72 | 36.23 | 42.09 | 14.4% |
| W1 | T2S | 16 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 62.32 | 59.19 | 66.60 | 11.9% |
| W1 | T2S | 16 | bchlib v2.1.3 | `table-remainder` | 7 | 3783.83 | 3417.56 | 4452.94 | 27.4% |
| W1 | T2S | 16 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-batch` | 7 | 0.68 | 0.67 | 0.68 | 1.5% |
| W1 | T2S | 16 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-loop` | 7 | 0.68 | 0.67 | 0.68 | 2.6% |
| W1 | T2S | 256 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 38.45 | 38.24 | 39.27 | 2.7% |
| W1 | T2S | 256 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 62.35 | 59.08 | 64.20 | 8.2% |
| W1 | T2S | 256 | bchlib v2.1.3 | `table-remainder` | 7 | 4073.64 | 3433.03 | 4433.22 | 24.6% |
| W1 | T2S | 256 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-batch` | 7 | 0.69 | 0.68 | 0.69 | 0.6% |
| W1 | T2S | 256 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-loop` | 7 | 0.68 | 0.68 | 0.69 | 1.2% |
| W1 | T2S | 4096 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 38.24 | 37.99 | 38.54 | 1.4% |
| W1 | T2S | 4096 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 60.26 | 59.74 | 60.83 | 1.8% |
| W1 | T2S | 4096 | bchlib v2.1.3 | `table-remainder` | 7 | 4018.06 | 3420.49 | 4442.00 | 25.4% |
| W1 | T2S | 4096 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-batch` | 3 | 0.68 | 0.68 | 0.69 | 0.2% |
| W1 | T2S | 4096 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `encode-loop` | 3 | 0.68 | 0.68 | 0.68 | 0.0% |
| W2 | B1 | 5 | aff3ct v4.7.0 | `basis-encode-pack` | 7 | 245.47 | 181.59 | 264.46 | 33.8% |
| W2 | B1 | 5 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `generator-matrix` | 7 | 5.51 | 4.60 | 6.20 | 29.0% |
| W2 | B1 | 5 | m4ri 20260122 | `echelonize` | 7 | 256.02 | 186.34 | 276.22 | 35.1% |
| W2 | B1 | 5 | m4ri 20260122 | `genmatrix-rref` | 7 | 267.36 | 193.57 | 303.57 | 41.1% |
| W2 | B1 | 256 | m4ri 20260122 | `matmul-m4rm` | 7 | 104.79 | 95.59 | 115.54 | 19.0% |
| W2 | B1 | 4096 | m4ri 20260122 | `matmul-m4rm` | 7 | 140.00 | 127.08 | 155.79 | 20.5% |
| W2 | B2 | 64 | aff3ct v4.7.0 | `basis-encode-pack` | 7 | 141.87 | 129.88 | 158.95 | 20.5% |
| W2 | B2 | 64 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `generator-matrix` | 7 | 6.21 | 5.75 | 6.81 | 17.1% |
| W2 | B2 | 64 | m4ri 20260122 | `echelonize` | 7 | 1428.97 | 1026.81 | 1440.90 | 29.0% |
| W2 | B2 | 64 | m4ri 20260122 | `genmatrix-rref` | 7 | 1189.97 | 1049.20 | 1293.97 | 20.6% |
| W2 | B2 | 256 | m4ri 20260122 | `matmul-m4rm` | 7 | 1950.69 | 1825.51 | 2141.37 | 16.2% |
| W2 | B2 | 4096 | m4ri 20260122 | `matmul-m4rm` | 7 | 2402.34 | 2107.75 | 2663.84 | 23.1% |
| W2 | B3 | 223 | aff3ct v4.7.0 | `basis-encode-pack` | 7 | 104.20 | 90.22 | 114.80 | 23.6% |
| W2 | B3 | 223 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `generator-matrix` | 7 | 6.45 | 5.98 | 7.01 | 16.0% |
| W2 | B3 | 223 | m4ri 20260122 | `echelonize` | 7 | 1532.43 | 1326.19 | 1552.38 | 14.8% |
| W2 | B3 | 223 | m4ri 20260122 | `genmatrix-rref` | 7 | 1569.88 | 1464.20 | 1794.67 | 21.1% |
| W2 | B3 | 256 | m4ri 20260122 | `matmul-m4rm` | 7 | 2079.82 | 1698.10 | 2336.40 | 30.7% |
| W2 | B3 | 4096 | m4ri 20260122 | `matmul-m4rm` | 7 | 2823.05 | 2456.63 | 3011.46 | 19.7% |
| W2 | T2N | 256 | m4ri 20260122 | `matmul-m4rm` | 7 | 74.44 | 71.30 | 75.19 | 5.2% |
| W2 | T2N | 4096 | m4ri 20260122 | `matmul-m4rm` | 7 | 82.02 | 81.34 | 82.20 | 1.0% |
| W2 | T2N | 32208 | aff3ct v4.7.0 | `basis-encode-pack` | 4 | 36.26 | 36.17 | 36.29 | 0.3% |
| W2 | T2N | 32208 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `generator-matrix-projected` | *estimate* | 0.58 | — | — | — |
| W2 | T2N | 32208 | m4ri 20260122 | `echelonize` | 7 | 142.89 | 141.94 | 143.60 | 1.2% |
| W2 | T2N | 32208 | m4ri 20260122 | `genmatrix-rref` | 7 | 346.89 | 339.39 | 349.42 | 2.9% |
| W2 | T2S | 256 | m4ri 20260122 | `matmul-m4rm` | 7 | 298.48 | 281.64 | 333.65 | 17.4% |
| W2 | T2S | 4096 | m4ri 20260122 | `matmul-m4rm` | 7 | 402.58 | 389.94 | 409.22 | 4.8% |
| W2 | T2S | 7032 | aff3ct v4.7.0 | `basis-encode-pack` | 7 | 42.74 | 42.14 | 43.26 | 2.6% |
| W2 | T2S | 7032 | gf2 c155b1b0c32e073cd0c4dd52d7ca52b169fba9ae | `generator-matrix` | 3 | 1.28 | 1.28 | 1.41 | 10.1% |
| W2 | T2S | 7032 | m4ri 20260122 | `echelonize` | 7 | 698.86 | 685.52 | 714.81 | 4.2% |
| W2 | T2S | 7032 | m4ri 20260122 | `genmatrix-rref` | 7 | 1560.56 | 1546.70 | 1589.11 | 2.7% |

## Files

| File | SHA-256 | Bytes |
|---|---|---|
| `2026-08-31-4e732b56-generator-agreement.txt` | `ccd3474e8296a67e…` | 170 |
| `2026-08-31-4e732b56-kodo-retrieval.txt` | `cecfeef3993e7730…` | 6679 |
| `2026-08-31-4e732b56-m4ri-thread-evidence.txt` | `f6ecb9472812db92…` | 3258 |
| `2026-09-01-4e732b56-determinism-agreement.txt` | `f5edb4dc022b3691…` | 11666 |
| `2026-09-01-4e732b56-generator-matrix-agreement.txt` | `0d357ea0a111b7a3…` | 1508 |
| `2026-09-01-4e732b56-small-aff3ct-perf-stat.txt` | `7a30323f4947b1e4…` | 3587 |
| `2026-09-01-4e732b56-small-aff3ct.csv` | `3e74c16b79f21483…` | 18963 |
| `2026-09-01-4e732b56-small-aff3ct.log` | `726c82a356236b12…` | 2938 |
| `2026-09-01-4e732b56-small-bchlib.csv` | `d7ec47a9ca9261e8…` | 5046 |
| `2026-09-01-4e732b56-small-bchlib.log` | `3867e5696fc63904…` | 1213 |
| `2026-09-01-4e732b56-small-generator-agreement.txt` | `ccd3474e8296a67e…` | 170 |
| `2026-09-01-4e732b56-small-gf2.csv` | `8a6d273778b4d99c…` | 27359 |
| `2026-09-01-4e732b56-small-gf2.log` | `711e46e2c97677a4…` | 3250 |
| `2026-09-01-4e732b56-small-host.txt` | `1a037587204752ac…` | 6947 |
| `2026-09-01-4e732b56-small-itpp.csv` | `bd74600680f120ae…` | 8204 |
| `2026-09-01-4e732b56-small-itpp.log` | `b4f89fdf8315cb83…` | 1345 |
| `2026-09-01-4e732b56-small-m4ri.csv` | `9d7aceefd62f74a1…` | 9406 |
| `2026-09-01-4e732b56-small-m4ri.log` | `1290ef421aac5c53…` | 1467 |
| `2026-09-01-4e732b56-t2n-aff3ct-perf-stat.txt` | `868d63c9e9fada73…` | 2078 |
| `2026-09-01-4e732b56-t2n-aff3ct.csv` | `9c8121fdb458c47f…` | 4963 |
| `2026-09-01-4e732b56-t2n-aff3ct.log` | `2ca11eccea7382a8…` | 1125 |
| `2026-09-01-4e732b56-t2n-bchlib.csv` | `8927e1da3895cc11…` | 2708 |
| `2026-09-01-4e732b56-t2n-bchlib.log` | `1276d01be9df7616…` | 752 |
| `2026-09-01-4e732b56-t2n-generator-agreement.txt` | `ccd3474e8296a67e…` | 170 |
| `2026-09-01-4e732b56-t2n-gf2.csv` | `abfda2b38dc850a9…` | 5468 |
| `2026-09-01-4e732b56-t2n-gf2.log` | `b6aef03e89a11211…` | 1217 |
| `2026-09-01-4e732b56-t2n-host.txt` | `c56286f359f438e4…` | 6938 |
| `2026-09-01-4e732b56-t2n-itpp.csv` | `eebab57153ce9b42…` | 90 |
| `2026-09-01-4e732b56-t2n-itpp.log` | `35e605e3c1dd47ad…` | 555 |
| `2026-09-01-4e732b56-t2n-m4ri.csv` | `0f399e3416bdde55…` | 2690 |
| `2026-09-01-4e732b56-t2n-m4ri.log` | `e2c887f21914ad6e…` | 770 |
| `generators.txt` | `612d5c84ee02ff69…` | 551 |

## Reproduction

```
dev/active/4e732b56/baseline-survey/fetch-build.sh
dev/active/4e732b56/baseline-survey/run-survey.sh dev/bench_results/4e732b56 <codes> <prefix>
dev/active/4e732b56/baseline-survey/make-receipt.py dev/bench_results/4e732b56
```
