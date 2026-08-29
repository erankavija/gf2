# Determinant companion cost for every campaign cell

This receipt directly measures `FieldMatrix::det` at every cell in the
permanent zero-fraction campaign. It reports determinant marginal cost
only; it does not rank or time permanent backends.

## Verdict

All 63 cells have five measured process outcomes. The largest projected fixed-$N$ determinant addition is 49.859819 s, and every cell fits the twelve-hour operational ceiling.

## Protocol and provenance

| Item | Recorded value |
|---|---|
| Preregistration | `dev/benchmarks/permanent_campaign/determinant-cost-preregistration-v4.md` |
| Machine-readable receipt | `dev/benchmarks/permanent_campaign/determinant-cost-all-cells-v4.csv` |
| Receipt SHA-256 | `58ba75a1b386cc1b049978af3b3f2b04efca0663473f89cfd98401abd276fc44` |
| Schema | `determinant-companion-v4` |
| Source revision | `3b44d511b2e58a23f58feccf4c10e768fb219006` |
| Relevant measurement source dirty | `false` |
| Benchmark executable SHA-256 | `7ebdf37d0750fef0f9e131774ca37360a93f9d1d03fffd3fb6506426c4f88821` |
| Toolchain | `rustc 1.95.0 (59807616e 2026-04-14)`; `release-bench` |
| Host | `fraktaali`; AMD Ryzen 9 5900X 12-Core Processor |
| Kernel | Linux 7.1.8-arch1-3 #1 SMP PREEMPT_DYNAMIC Tue, 11 Aug 2026 09:16:08 +0000 x86_64 GNU/Linux |
| Power policy | governor `powersave`; boost `enabled` |
| Isolation | `dev/scripts/ccx1-bench-flock.sh:ccx1`; affinity `6-11`; one serial worker |
| Process contract | 5 fresh processes per cell; 5 timed repetitions per process; 250 ms target |
| Recorded window | Unix ns `1787993409873112403` through `1787993820608005125` |
| Exact cohort argv | `["/usr/bin/python3","/home/vkaskivuo/Projects/gf2/dev/benchmarks/permanent_campaign/determinant_cost_v4.py","run","--binary","target/release/deps/determinant_companion-214102561bb5c0a5","--output","dev/benchmarks/permanent_campaign/determinant-cost-all-cells-v4.csv","--scratch-dir","/tmp/gf2-ec22205e-determinant-cost-v4"]` |
| Exact process argv | Recorded per process in the CSV `invocation` column |

The cohort ran with the command identity recorded above under the exclusive
benchmark wrapper. Fixture generation and adaptive calibration are outside
the timed windows. The preregistration fixes the address formula and the
calibration stopping rule.

## Pooled marginal cost and fixed-$N$ projection

For measured processes $M_{q,n}$, each cell uses pooled raw totals:

$$
t_{q,n}=\frac{\sum_{e\in M_{q,n}}T_e}{\sum_{e\in M_{q,n}}C_e}.
$$

No representative-order interpolation or mean of process means enters the
table. The projected addition is $N_{q,n}t_{q,n}$ using the protocol's
fixed sample count.

| $q$ | $n$ | measured / planned | determinant ($\mu$s/matrix) | fixed $N$ | projected addition (s) | projected addition (h) | fits 12 h |
|---:|---:|---:|---:|---:|---:|---:|:---:|
| 3 | 4 | 5 / 5 | 0.191492784 | 20,000,000 | 3.829856 | 0.001063849 | yes |
| 3 | 5 | 5 / 5 | 0.253843240 | 20,000,000 | 5.076865 | 0.001410240 | yes |
| 3 | 6 | 5 / 5 | 0.325056674 | 20,000,000 | 6.501133 | 0.001805870 | yes |
| 3 | 7 | 5 / 5 | 0.426505769 | 20,000,000 | 8.530115 | 0.002369476 | yes |
| 3 | 8 | 5 / 5 | 0.535851022 | 20,000,000 | 10.717020 | 0.002976950 | yes |
| 3 | 9 | 5 / 5 | 0.620443493 | 20,000,000 | 12.408870 | 0.003446908 | yes |
| 3 | 10 | 5 / 5 | 0.715224116 | 20,000,000 | 14.304482 | 0.003973467 | yes |
| 3 | 11 | 5 / 5 | 0.828406541 | 20,000,000 | 16.568131 | 0.004602259 | yes |
| 3 | 12 | 5 / 5 | 1.000208324 | 20,000,000 | 20.004166 | 0.005556713 | yes |
| 3 | 13 | 5 / 5 | 1.152479226 | 20,000,000 | 23.049585 | 0.006402662 | yes |
| 3 | 14 | 5 / 5 | 1.297757045 | 20,000,000 | 25.955141 | 0.007209761 | yes |
| 3 | 15 | 5 / 5 | 1.507003958 | 20,000,000 | 30.140079 | 0.008372244 | yes |
| 3 | 16 | 5 / 5 | 1.711553218 | 20,000,000 | 34.231064 | 0.009508629 | yes |
| 3 | 17 | 5 / 5 | 1.902367561 | 20,000,000 | 38.047351 | 0.010568709 | yes |
| 3 | 18 | 5 / 5 | 2.101248003 | 20,000,000 | 42.024960 | 0.011673600 | yes |
| 3 | 19 | 5 / 5 | 2.272889755 | 20,000,000 | 45.457795 | 0.012627165 | yes |
| 3 | 20 | 5 / 5 | 2.492990926 | 20,000,000 | 49.859819 | 0.013849950 | yes |
| 3 | 21 | 5 / 5 | 2.764393244 | 222,223 | 0.614312 | 0.000170642 | yes |
| 3 | 22 | 5 / 5 | 3.078355026 | 222,223 | 0.684081 | 0.000190023 | yes |
| 3 | 23 | 5 / 5 | 3.402801971 | 222,223 | 0.756181 | 0.000210050 | yes |
| 3 | 24 | 5 / 5 | 3.805012006 | 222,223 | 0.845561 | 0.000234878 | yes |
| 3 | 25 | 5 / 5 | 4.106460837 | 222,223 | 0.912550 | 0.000253486 | yes |
| 3 | 26 | 5 / 5 | 4.426566950 | 222,223 | 0.983685 | 0.000273246 | yes |
| 3 | 27 | 5 / 5 | 4.806795970 | 222,223 | 1.068181 | 0.000296717 | yes |
| 3 | 28 | 5 / 5 | 5.232339866 | 222,223 | 1.162746 | 0.000322985 | yes |
| 5 | 4 | 5 / 5 | 0.191133737 | 16,000,000 | 3.058140 | 0.000849483 | yes |
| 5 | 5 | 5 / 5 | 0.252498759 | 16,000,000 | 4.039980 | 0.001122217 | yes |
| 5 | 6 | 5 / 5 | 0.332532502 | 16,000,000 | 5.320520 | 0.001477922 | yes |
| 5 | 7 | 5 / 5 | 0.432629250 | 16,000,000 | 6.922068 | 0.001922797 | yes |
| 5 | 8 | 5 / 5 | 0.558512185 | 16,000,000 | 8.936195 | 0.002482276 | yes |
| 5 | 9 | 5 / 5 | 0.646361628 | 16,000,000 | 10.341786 | 0.002872718 | yes |
| 5 | 10 | 5 / 5 | 0.746708728 | 16,000,000 | 11.947340 | 0.003318705 | yes |
| 5 | 11 | 5 / 5 | 0.879911911 | 16,000,000 | 14.078591 | 0.003910720 | yes |
| 5 | 12 | 5 / 5 | 1.067052366 | 16,000,000 | 17.072838 | 0.004742455 | yes |
| 5 | 13 | 5 / 5 | 1.214091018 | 16,000,000 | 19.425456 | 0.005395960 | yes |
| 5 | 14 | 5 / 5 | 1.407383493 | 16,000,000 | 22.518136 | 0.006255038 | yes |
| 5 | 15 | 5 / 5 | 1.635923386 | 16,000,000 | 26.174774 | 0.007270771 | yes |
| 5 | 16 | 5 / 5 | 1.900044845 | 16,000,000 | 30.400718 | 0.008444644 | yes |
| 5 | 17 | 5 / 5 | 2.077163019 | 160,000 | 0.332346 | 0.000092318 | yes |
| 5 | 18 | 5 / 5 | 2.273084615 | 160,000 | 0.363694 | 0.000101026 | yes |
| 5 | 19 | 5 / 5 | 2.495245893 | 160,000 | 0.399239 | 0.000110900 | yes |
| 5 | 20 | 5 / 5 | 2.773602858 | 160,000 | 0.443776 | 0.000123271 | yes |
| 5 | 21 | 5 / 5 | 3.016246401 | 160,000 | 0.482599 | 0.000134055 | yes |
| 5 | 22 | 5 / 5 | 3.341893460 | 160,000 | 0.534703 | 0.000148529 | yes |
| 5 | 23 | 5 / 5 | 3.710176171 | 160,000 | 0.593628 | 0.000164897 | yes |
| 5 | 24 | 5 / 5 | 4.154523057 | 160,000 | 0.664724 | 0.000184645 | yes |
| 7 | 4 | 5 / 5 | 0.196866039 | 12,244,898 | 2.410605 | 0.000669612 | yes |
| 7 | 5 | 5 / 5 | 0.259908242 | 12,244,898 | 3.182550 | 0.000884042 | yes |
| 7 | 6 | 5 / 5 | 0.338994218 | 12,244,898 | 4.150950 | 0.001153042 | yes |
| 7 | 7 | 5 / 5 | 0.449114024 | 12,244,898 | 5.499355 | 0.001527599 | yes |
| 7 | 8 | 5 / 5 | 0.574935005 | 12,244,898 | 7.040020 | 0.001955561 | yes |
| 7 | 9 | 5 / 5 | 0.665202934 | 12,244,898 | 8.145342 | 0.002262595 | yes |
| 7 | 10 | 5 / 5 | 0.763719412 | 12,244,898 | 9.351666 | 0.002597685 | yes |
| 7 | 11 | 5 / 5 | 0.892527184 | 12,244,898 | 10.928904 | 0.003035807 | yes |
| 7 | 12 | 5 / 5 | 1.086112506 | 12,244,898 | 13.299337 | 0.003694260 | yes |
| 7 | 13 | 5 / 5 | 1.237070299 | 12,244,898 | 15.147800 | 0.004207722 | yes |
| 7 | 14 | 5 / 5 | 1.433287075 | 12,244,898 | 17.550454 | 0.004875126 | yes |
| 7 | 15 | 5 / 5 | 1.651533716 | 12,244,898 | 20.222862 | 0.005617462 | yes |
| 7 | 16 | 5 / 5 | 1.914817619 | 12,244,898 | 23.446746 | 0.006512985 | yes |
| 7 | 17 | 5 / 5 | 2.112310895 | 122,449 | 0.258650 | 0.000071847 | yes |
| 7 | 18 | 5 / 5 | 2.292883661 | 122,449 | 0.280761 | 0.000077989 | yes |
| 7 | 19 | 5 / 5 | 2.532792656 | 122,449 | 0.310138 | 0.000086149 | yes |
| 7 | 20 | 5 / 5 | 2.796792921 | 122,449 | 0.342464 | 0.000095129 | yes |

## Preserved failures and contradictions

No failed or censored process outcome was observed in the frozen cohort.
All 315 preregistered process-cell outcomes are measured rows; none was
discarded, replaced, or extended.

The powersave governor and enabled boost state are material limitations.
This receipt supports the determinant budget projection; it does not support
close performance comparisons or a fixed-frequency claim.

## Validation

The committed validator proves the exact 63-cell and 315-row address set,
uniform cohort provenance, raw-to-pooled arithmetic, fixture-address
uniqueness, non-measured-row emptiness, and byte-for-byte agreement between
this rendered document and the machine receipt.

```sh
python3 dev/benchmarks/permanent_campaign/determinant_cost_v4.py validate \
  --receipt dev/benchmarks/permanent_campaign/determinant-cost-all-cells-v4.csv \
  --report dev/benchmarks/permanent_campaign/determinant-cost-all-cells-v4.md
```
