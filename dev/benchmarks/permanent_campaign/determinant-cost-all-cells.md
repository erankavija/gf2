# Determinant companion cost for every campaign cell

This receipt directly measures `FieldMatrix::det` at every cell in the
permanent zero-fraction campaign. It reports determinant marginal cost
only; it does not rank or time permanent backends.

## Verdict

All 63 cells have five measured process outcomes. The largest projected fixed-$N$ determinant addition is 49.594101 s, and every cell fits the twelve-hour operational ceiling.

## Protocol and provenance

| Item | Recorded value |
|---|---|
| Preregistration | `dev/benchmarks/permanent_campaign/determinant-cost-preregistration-v3.md` |
| Machine-readable receipt | `dev/benchmarks/permanent_campaign/determinant-cost-all-cells.csv` |
| Receipt SHA-256 | `c74f569bd329c8100f29ef65c557f5e3d022cac6aa08655f235f5edfbddb0bba` |
| Schema | `determinant-companion-v3` |
| Source revision | `66ab70bcb0486616af8ff9df2cdc285c2c99a294` |
| Relevant measurement source dirty | `false` |
| Benchmark executable SHA-256 | `6e1927f9903b8d68c8fde0fb0f3311fc726ec1c7870adfa1887dbd18d9b9380e` |
| Toolchain | `rustc 1.95.0 (59807616e 2026-04-14)`; `release-bench` |
| Host | `fraktaali`; AMD Ryzen 9 5900X 12-Core Processor |
| Kernel | Linux 7.1.8-arch1-3 #1 SMP PREEMPT_DYNAMIC Tue, 11 Aug 2026 09:16:08 +0000 x86_64 GNU/Linux |
| Power policy | governor `powersave`; boost `enabled` |
| Isolation | `dev/scripts/ccx1-bench-flock.sh:ccx1`; affinity `6-11`; one serial worker |
| Process contract | 5 fresh processes per cell; 5 timed repetitions per process; 250 ms target |
| Recorded window | Unix ns `1787990985523864641` through `1787991397265343651` |
| Exact cohort argv | `["/usr/bin/python3","/home/vkaskivuo/Projects/gf2/dev/benchmarks/permanent_campaign/determinant_cost_v3.py","run","--binary","target/release/deps/determinant_companion-214102561bb5c0a5","--output","dev/benchmarks/permanent_campaign/determinant-cost-all-cells.csv","--scratch-dir","/tmp/gf2-ec22205e-determinant-cost-v3"]` |
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
| 3 | 4 | 5 / 5 | 0.191375310 | 20,000,000 | 3.827506 | 0.001063196 | yes |
| 3 | 5 | 5 / 5 | 0.250107640 | 20,000,000 | 5.002153 | 0.001389487 | yes |
| 3 | 6 | 5 / 5 | 0.324637898 | 20,000,000 | 6.492758 | 0.001803544 | yes |
| 3 | 7 | 5 / 5 | 0.417656268 | 20,000,000 | 8.353125 | 0.002320313 | yes |
| 3 | 8 | 5 / 5 | 0.520398799 | 20,000,000 | 10.407976 | 0.002891104 | yes |
| 3 | 9 | 5 / 5 | 0.607302472 | 20,000,000 | 12.146049 | 0.003373903 | yes |
| 3 | 10 | 5 / 5 | 0.709747785 | 20,000,000 | 14.194956 | 0.003943043 | yes |
| 3 | 11 | 5 / 5 | 0.823861915 | 20,000,000 | 16.477238 | 0.004577011 | yes |
| 3 | 12 | 5 / 5 | 0.994701320 | 20,000,000 | 19.894026 | 0.005526118 | yes |
| 3 | 13 | 5 / 5 | 1.178548506 | 20,000,000 | 23.570970 | 0.006547492 | yes |
| 3 | 14 | 5 / 5 | 1.288352945 | 20,000,000 | 25.767059 | 0.007157516 | yes |
| 3 | 15 | 5 / 5 | 1.494059060 | 20,000,000 | 29.881181 | 0.008300328 | yes |
| 3 | 16 | 5 / 5 | 1.696359654 | 20,000,000 | 33.927193 | 0.009424220 | yes |
| 3 | 17 | 5 / 5 | 1.903902942 | 20,000,000 | 38.078059 | 0.010577239 | yes |
| 3 | 18 | 5 / 5 | 2.086914863 | 20,000,000 | 41.738297 | 0.011593971 | yes |
| 3 | 19 | 5 / 5 | 2.271590913 | 20,000,000 | 45.431818 | 0.012619950 | yes |
| 3 | 20 | 5 / 5 | 2.479705053 | 20,000,000 | 49.594101 | 0.013776139 | yes |
| 3 | 21 | 5 / 5 | 2.745905660 | 222,223 | 0.610203 | 0.000169501 | yes |
| 3 | 22 | 5 / 5 | 3.071670180 | 222,223 | 0.682596 | 0.000189610 | yes |
| 3 | 23 | 5 / 5 | 3.376521426 | 222,223 | 0.750341 | 0.000208428 | yes |
| 3 | 24 | 5 / 5 | 3.750348302 | 222,223 | 0.833414 | 0.000231504 | yes |
| 3 | 25 | 5 / 5 | 4.065904216 | 222,223 | 0.903537 | 0.000250983 | yes |
| 3 | 26 | 5 / 5 | 4.353275825 | 222,223 | 0.967398 | 0.000268722 | yes |
| 3 | 27 | 5 / 5 | 4.730434164 | 222,223 | 1.051211 | 0.000292003 | yes |
| 3 | 28 | 5 / 5 | 5.213838452 | 222,223 | 1.158635 | 0.000321843 | yes |
| 5 | 4 | 5 / 5 | 0.189240965 | 16,000,000 | 3.027855 | 0.000841071 | yes |
| 5 | 5 | 5 / 5 | 0.252127578 | 16,000,000 | 4.034041 | 0.001120567 | yes |
| 5 | 6 | 5 / 5 | 0.331840758 | 16,000,000 | 5.309452 | 0.001474848 | yes |
| 5 | 7 | 5 / 5 | 0.428817977 | 16,000,000 | 6.861088 | 0.001905858 | yes |
| 5 | 8 | 5 / 5 | 0.556005903 | 16,000,000 | 8.896094 | 0.002471137 | yes |
| 5 | 9 | 5 / 5 | 0.637832872 | 16,000,000 | 10.205326 | 0.002834813 | yes |
| 5 | 10 | 5 / 5 | 0.740928150 | 16,000,000 | 11.854850 | 0.003293014 | yes |
| 5 | 11 | 5 / 5 | 0.872275885 | 16,000,000 | 13.956414 | 0.003876782 | yes |
| 5 | 12 | 5 / 5 | 1.042840075 | 16,000,000 | 16.685441 | 0.004634845 | yes |
| 5 | 13 | 5 / 5 | 1.198696621 | 16,000,000 | 19.179146 | 0.005327541 | yes |
| 5 | 14 | 5 / 5 | 1.387535728 | 16,000,000 | 22.200572 | 0.006166825 | yes |
| 5 | 15 | 5 / 5 | 1.594561745 | 16,000,000 | 25.512988 | 0.007086941 | yes |
| 5 | 16 | 5 / 5 | 1.851529810 | 16,000,000 | 29.624477 | 0.008229021 | yes |
| 5 | 17 | 5 / 5 | 2.037094062 | 160,000 | 0.325935 | 0.000090538 | yes |
| 5 | 18 | 5 / 5 | 2.230785677 | 160,000 | 0.356926 | 0.000099146 | yes |
| 5 | 19 | 5 / 5 | 2.458326463 | 160,000 | 0.393332 | 0.000109259 | yes |
| 5 | 20 | 5 / 5 | 2.691519409 | 160,000 | 0.430643 | 0.000119623 | yes |
| 5 | 21 | 5 / 5 | 2.977758133 | 160,000 | 0.476441 | 0.000132345 | yes |
| 5 | 22 | 5 / 5 | 3.327481178 | 160,000 | 0.532397 | 0.000147888 | yes |
| 5 | 23 | 5 / 5 | 3.676049406 | 160,000 | 0.588168 | 0.000163380 | yes |
| 5 | 24 | 5 / 5 | 4.106367660 | 160,000 | 0.657019 | 0.000182505 | yes |
| 7 | 4 | 5 / 5 | 0.200544912 | 12,244,898 | 2.455652 | 0.000682126 | yes |
| 7 | 5 | 5 / 5 | 0.260440948 | 12,244,898 | 3.189073 | 0.000885854 | yes |
| 7 | 6 | 5 / 5 | 0.343533008 | 12,244,898 | 4.206527 | 0.001168480 | yes |
| 7 | 7 | 5 / 5 | 0.446162454 | 12,244,898 | 5.463214 | 0.001517559 | yes |
| 7 | 8 | 5 / 5 | 0.566995444 | 12,244,898 | 6.942801 | 0.001928556 | yes |
| 7 | 9 | 5 / 5 | 0.661238622 | 12,244,898 | 8.096799 | 0.002249111 | yes |
| 7 | 10 | 5 / 5 | 0.766418655 | 12,244,898 | 9.384718 | 0.002606866 | yes |
| 7 | 11 | 5 / 5 | 0.895393496 | 12,244,898 | 10.964002 | 0.003045556 | yes |
| 7 | 12 | 5 / 5 | 1.074320023 | 12,244,898 | 13.154939 | 0.003654150 | yes |
| 7 | 13 | 5 / 5 | 1.242152971 | 12,244,898 | 15.210036 | 0.004225010 | yes |
| 7 | 14 | 5 / 5 | 1.433506299 | 12,244,898 | 17.553138 | 0.004875872 | yes |
| 7 | 15 | 5 / 5 | 1.650381633 | 12,244,898 | 20.208755 | 0.005613543 | yes |
| 7 | 16 | 5 / 5 | 1.915020381 | 12,244,898 | 23.449229 | 0.006513675 | yes |
| 7 | 17 | 5 / 5 | 2.108279299 | 122,449 | 0.258157 | 0.000071710 | yes |
| 7 | 18 | 5 / 5 | 2.293505766 | 122,449 | 0.280837 | 0.000078010 | yes |
| 7 | 19 | 5 / 5 | 2.531074114 | 122,449 | 0.309927 | 0.000086091 | yes |
| 7 | 20 | 5 / 5 | 2.782810207 | 122,449 | 0.340752 | 0.000094653 | yes |

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
python3 dev/benchmarks/permanent_campaign/determinant_cost_v3.py validate \
  --receipt dev/benchmarks/permanent_campaign/determinant-cost-all-cells.csv \
  --report dev/benchmarks/permanent_campaign/determinant-cost-all-cells.md
```
