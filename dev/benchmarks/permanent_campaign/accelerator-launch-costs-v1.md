# Accelerator launch costs v1

This versioned receipt freezes per-cell accelerator launch-sizing evidence
for the permanent zero-fraction campaign. It is execution evidence only:
it does not select a backend and it contains no campaign draw, scientific
result, estimate, interval, or verdict.

## Bound inputs

Receipt schema: `accelerator-launch-cost-receipt-v1`.

Every input is a repository-relative path bound to lowercase SHA-256.
The validator refuses changed bytes before deriving a value.

| Path | SHA-256 | Role |
|---|---|---|
| `dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829/manifest.json` | `5caa384d9c87f24562ee6d91c61c44dbc04674512b3dbe63e761ca0b9480ae57` | frozen campaign cell and backend map |
| `dev/benchmarks/permanent_campaign/backend-selection-v1.md` | `fe5d37ba7c216a753bf3e546614a222c3c20e563f7f4e1d3c0341af9e1c464fe` | complete-cohort eligibility and backend-selection receipt |
| `dev/benchmarks/permanent_campaign/premeasure-v1-ledger.csv` | `d1efd9dcfa39b8498db1e04ba0720de2bf96677fc6b820ffadc3f1919848d046` | all 1,440 premeasure-v1 terminal process outcomes |
| `dev/benchmarks/permanent_campaign/premeasure-v1-candidates.csv` | `272a524185c14515394a24a5e7385e07b7dc5620c0a488a12438668b72b8c6b8` | premeasure-v1 structurally valid timing projection |
| `dev/benchmarks/permanent_campaign/premeasure-v1-receipt.md` | `46826730ec186963260d55df8b19d53229ffb98b768343611bb4b5a537c21664` | premeasure-v1 cohort identity and censor account |
| `dev/benchmarks/permanent_campaign/premeasure-v1-deviations.md` | `b5e8401a6523e2c05912fa720fe4b5b74491c28b77a7bf2154d2f5a36b1956eb` | native-M accelerator row projection rule |
| `dev/benchmarks/permanent_campaign/backend-ordering.csv` | `57c2fafbb4050d4eacf65837c41bf4c3fe1ab69fd49eac622486bb6282bc8dd2` | all 48 frontier execution outcomes and four summaries |
| `dev/benchmarks/permanent_campaign/backend-ordering.md` | `7e50381daf157a5f045717c92932e37edd92fe54ca7d1a63ac387c44cfff13ae` | frontier protocol, arithmetic, and provenance receipt |
| `dev/benchmarks/permanent_campaign/backend-selection-v1-rng-addendum.md` | `bb33bde423edb4918beb92abf134055157ca6950e67dfa45c8e1054791158314` | measurement RNG and rebuild provenance |
| `dev/benchmarks/permanent_campaign/accelerator_launch_costs_v1.py` | `d0cd531018573d3a4f47e5afc7e706e445e6811c1741d3c84bf5f7c5ccf6e917` | derivation and validator source |
| `dev/benchmarks/permanent_campaign/accelerator-launch-costs-v1.csv` | `079c45dd9039044e1a47dc457ca8933e4e27423b0e6ec4888a37245e79e8e3cf` | production launch-cost CSV |

## Eligibility and arithmetic

One outcome contributes only when its exact $(q,n)$ key is accelerator-backed
in the frozen manifest, its measured backend is `gpu_hip` at $M=1024$ on
that same cell, its identity and collector row-validity checks pass, and all twelve
planned fresh processes in that arm have finite `measured` outcomes.
The 36 contributing rows at $(7,17)$, $(7,18)$, $(7,19)$ retain
`provenance_complete=false`. Each records `unavailable` for exactly
`observed_harness_source_sha`, `observed_deps_source_sha`, `observed_rustc`, `observed_cargo`; their session source revision,
executable SHA-256, CPU, GPU, ROCm, and kernel fields remain observed.
The hash-bound backend-selection receipt states the source, build, host,
and device values applicable to every candidate in this producing cohort.
The hash-bound RNG addendum identifies that same cohort by source revision,
executable SHA-256, and ledger/candidate identities. Those artifacts
supplement the four unavailable probes without relabeling any raw flag.
A signal or harness censor remains in the bound cohort but makes that arm
ineligible; it is never converted, replaced, interpolated, or imputed.

For eligible cell $(q,n)$ with process outcomes $e$, exact decimal seconds
$T_e$ and integer matrix counts $C_e$ are pooled as

$$
u_{q,n}=10^6\frac{\sum_e T_e}{\sum_e C_e}.
$$

The production integer is $\lceil u_{q,n}\rceil$. Rounding upward is
conservative for launch sizing because the stored duration is never below
the pooled observed composite draw-pack-evaluate-count time per matrix.

The bound premeasure cohort retains 44 censored outcomes:
43 harness-censored candidates and
1 signal-censored ledger position. None belongs
to a contributing arm. The frontier receipt has no censored outcome.

## Cohort provenance

| Item | `premeasure-v1` | `296a41c9` frontier |
|---|---|---|
| Source revision | `1350d5b46cd541093882537a89ea35db05a7afc5` | `414d31f8184a398deee946f151134511522dfca3` |
| Harness / dependency revision | bound by the selection and RNG receipts | `414d31f8184a398deee946f151134511522dfca3` / `d950bbb883845429d378aa2708ae7406b06fa6bc` |
| Executable SHA-256 | `1198cce47de06a6793fd1b5d880d559f020acffebc3d237137fa5c1326775606` | `6e24533cfbac987a0cec20af02f9dfb0a7bd9ce12c9e80cbdc00cad72150ccad` |
| Build toolchain | Rust 1.95.0 release+HIP, bound by the selection receipt | rustc 1.95.0 (59807616e 2026-04-14); cargo 1.95.0 |
| Host processor | AMD Ryzen 9 5900X 12-Core Processor; 24 logical CPUs | AMD Ryzen 9 5900X 12-Core Processor; 24 logical CPUs |
| Accelerator | card0,AMD Radeon RX 6950 XT,0x73a5,Advanced Micro Devices Inc. [AMD/ATI],D4124100,0x0e3a,0xc0,1,42338,gfx1030 | card0,AMD Radeon RX 6950 XT,0x73a5,Advanced Micro Devices Inc. [AMD/ATI],D4124100,0x0e3a,0xc0,1,42338,gfx1030 |
| Runtime / kernel | ROCm 7.2.4; 7.1.8-arch1-3 | ROCm 7.2.4; 7.1.6-arch1-1 |
| Full receipt outcomes | 1396 measured + 43 harness-censored + 1 signal-censored / 1440 | 48 measured + 0 censored / 48 |
| Contributing accelerator outcomes | 156 across 13 cells | 24 across 2 cells |
| Native / supplemented contributing provenance | 120 complete in-row / 36 bound-receipt supplemented | 24 complete in-row / 0 supplemented |

## Rounded launch-cost rows

| $q$ | $n$ | Cohort | finite / planned | censored | $M$ | $\sum C_e$ | $\sum T_e$ (s) | pooled $u_{q,n}$ ($\mu$s/matrix) | $\lceil u_{q,n}\rceil$ |
|---:|---:|---|---:|---:|---:|---:|---:|---:|---:|
| 3 | 16 | `premeasure-v1` | 12 / 12 | 0 | 1024 | 3424256 | 59.100081 | 17.259247264 | 18 |
| 3 | 17 | `premeasure-v1` | 12 / 12 | 0 | 1024 | 1961984 | 59.514978 | 30.334079177 | 31 |
| 3 | 18 | `premeasure-v1` | 12 / 12 | 0 | 1024 | 1071104 | 59.796209 | 55.826706837 | 56 |
| 3 | 19 | `premeasure-v1` | 12 / 12 | 0 | 1024 | 565248 | 60.046866 | 106.231010105 | 107 |
| 3 | 20 | `premeasure-v1` | 12 / 12 | 0 | 1024 | 294912 | 60.892671 | 206.477427165 | 207 |
| 3 | 21 | `premeasure-v1` | 12 / 12 | 0 | 1024 | 152576 | 62.081807 | 406.891037909 | 407 |
| 3 | 22 | `premeasure-v1` | 12 / 12 | 0 | 1024 | 86016 | 69.377408 | 806.563988095 | 807 |
| 3 | 23 | `premeasure-v1` | 12 / 12 | 0 | 1024 | 61440 | 98.779868 | 1607.745247396 | 1608 |
| 3 | 24 | `premeasure-v1` | 12 / 12 | 0 | 1024 | 61440 | 197.309057 | 3211.410432943 | 3212 |
| 3 | 25 | `premeasure-v1` | 12 / 12 | 0 | 1024 | 61440 | 395.654957 | 6439.696565755 | 6440 |
| 3 | 28 | `296a41c9-frontier` | 12 / 12 | 0 | 1024 | 36864 | 1904.156081 | 51653.539523655 | 51654 |
| 7 | 17 | `premeasure-v1` | 12 / 12 | 0 | 1024 | 61440 | 116.396183 | 1894.469124349 | 1895 |
| 7 | 18 | `premeasure-v1` | 12 / 12 | 0 | 1024 | 61440 | 243.599016 | 3964.827734375 | 3965 |
| 7 | 19 | `premeasure-v1` | 12 / 12 | 0 | 1024 | 61440 | 508.293410 | 8273.004720052 | 8274 |
| 7 | 20 | `296a41c9-frontier` | 12 / 12 | 0 | 1024 | 61440 | 1061.085070 | 17270.264811198 | 17271 |

## Validation

The fail-closed validator checks every bound digest; unique manifest, ledger,
candidate, and frontier identities; the exact manifest accelerator-key set;
complete same-cell eligibility; pooled arithmetic; conservative rounding;
the exact stable CSV header and positive integer rows; and byte-for-byte
agreement between this rendered receipt and the production CSV.

```sh
python3 dev/benchmarks/permanent_campaign/accelerator_launch_costs_v1.py validate \
  --manifest dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829/manifest.json \
  --cost-table dev/benchmarks/permanent_campaign/accelerator-launch-costs-v1.csv \
  --receipt dev/benchmarks/permanent_campaign/accelerator-launch-costs-v1.md
```
