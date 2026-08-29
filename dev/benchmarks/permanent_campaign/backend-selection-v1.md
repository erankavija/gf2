# Permanent campaign backend selection v1

This is the final, non-draft selection receipt for the 63-cell permanent
zero-fraction campaign freeze. Its permanent path is
`dev/benchmarks/permanent_campaign/backend-selection-v1.md`: it lives beside
the immutable campaign timing receipts, is versioned independently of a JIT
issue directory, and can therefore remain the stable artifact identity stored
by every `CellSpec.backend_receipt`.

## Controlling contract

The controlling contract is
`dev/simulation_results/permanent-zero-fraction/protocol.md`, section
`Backend freeze`, SHA-256
`249f3de398cd234cdd9c1f1d352fc909394f3bacf13acda606d95da343693639`.
The receipt applies its rules in order:

1. A candidate enters only after the shared per-matrix behavioural suite
   agrees with the generic reference, including validation anchors.
2. It remains only when the intended host and build meet documented safety,
   capability, launch-duration, and resource conditions.
3. It remains only with twelve finite `measured` composite outcomes out of
   twelve planned processes. Every censor remains in the denominator and is
   neither replaced nor converted to a finite rate.
4. It remains only with cell-applicable composite draw-pack-evaluate-count
   timing in a hashed raw receipt. Within one matched cohort, the backend with
   the greater arithmetic mean of the complete eligible process-rate cohort is
   selected. No rate is compared across cohorts.

No permanent value, determinant value, zero count, interval, or test result
enters these decisions.

## Timing receipts considered

Every repository-relative path and digest below was recomputed from the
committed bytes. The first two rows are the raw timing receipts that determine
the selections. The three study grids are raw timing receipts considered only
to preserve their outcomes and enforce the stated exclusions; they are never
cross-ranked with either selection cohort.

| Path | SHA-256 | Use |
|---|---|---|
| `dev/benchmarks/permanent_campaign/premeasure-v1-ledger.csv` | `d1efd9dcfa39b8498db1e04ba0720de2bf96677fc6b820ffadc3f1919848d046` | Raw terminal ledger for all 1,440 planned premeasure-v1 positions; selection cohort for 60 cells. |
| `dev/benchmarks/permanent_campaign/backend-ordering.csv` | `57c2fafbb4050d4eacf65837c41bf4c3fe1ab69fd49eac622486bb6282bc8dd2` | Raw-plus-summary 296a41c9 receipt for the three frontier cells. |
| `dev/studies/047b62ed/permanent-campaign-20260814T230032Z-2085453-q3-grid.csv` | `c3829994762236dde8578350dcab0bf85631f09be6497cef9bbbc31a28e3b976` | Study-cohort q3 raw timing; exclusion evidence only. |
| `dev/studies/91605d4d/permanent-campaign-20260814T230032Z-2085453-q5-grid.csv` | `4576047fa082ab4b6a0029cde5f579a093232b7b5bdb44b85f4ee6e404f117fb` | Study-cohort q5 raw timing; exclusion evidence only. |
| `dev/studies/6c7fcb38/permanent-campaign-20260814T230032Z-2085453-q7-grid.csv` | `5c1a902eeb4a1f1a49c0a65b7b66e5f6ab7c73b59de569f955651dbb7a0ce103` | Study-cohort q7 raw timing; exclusion evidence only. |

The committed premeasure projection and contract are also bound here:

| Path | SHA-256 | Role |
|---|---|---|
| `dev/benchmarks/permanent_campaign/premeasure-v1-candidates.csv` | `272a524185c14515394a24a5e7385e07b7dc5620c0a488a12438668b72b8c6b8` | Structurally valid per-process projection; preserves 1,396 measured and 43 harness-censored rows. |
| `dev/benchmarks/permanent_campaign/premeasure-v1-cell-summary.csv` | `a881379d2066e764af4e7b6aa459b5f81c0562d67c42f9ddb81362b325ad65bd` | Derived per-configuration counts and observed finite-rate means. |
| `dev/benchmarks/permanent_campaign/premeasure-plan-v1.csv` | `c268d554411c65bc6c3366247775e8d8d5b177b46f064e850a0da5b3d3b38640` | Frozen 120-configuration schedule. |
| `dev/benchmarks/permanent_campaign/premeasure-plan-v1.md` | `d08eabefcdc30b0c747a3b21566627f6b30cef22503d4c897d889ebdfbd09b1d` | Human-readable fixed process and stopping contract. |
| `dev/benchmarks/permanent_campaign/premeasure-v1-receipt.md` | `46826730ec186963260d55df8b19d53229ffb98b768343611bb4b5a537c21664` | Immutable cohort identity and censor account. |
| `dev/benchmarks/permanent_campaign/premeasure-v1-deviations.md` | `b5e8401a6523e2c05912fa720fe4b5b74491c28b77a7bf2154d2f5a36b1956eb` | Fixed D-01 dual-row accelerator projection. |
| `dev/benchmarks/permanent_campaign/backend-ordering.md` | `7e50381daf157a5f045717c92932e37edd92fe54ca7d1a63ac387c44cfff13ae` | Narrative and mechanical validation for 296a41c9. |

The only premeasure raw bytes available in addition to the committed ledger
for signal-censored position 539 are preserved exactly:

| Path | SHA-256 |
|---|---|
| `dev/benchmarks/permanent_campaign/premeasure-v1-censored/process-0539-3-26-A/receipt.txt` | `5b08387b4d32bde37d3f41bc839455bbcb272d81d1c0b0930180d8d70926483b` |
| `dev/benchmarks/permanent_campaign/premeasure-v1-censored/process-0539-3-26-A/exit.status` | `f5bde7eb9f6c71611dc5726e8aca3eb4eba3e386da49e0a4ed5c295a90a73a0d` |
| `dev/benchmarks/permanent_campaign/premeasure-v1-censored/process-0539-3-26-A/harness.log` | `65f8097389d6d9a6f0c34d8aee02f385e229ff2724320674ec5cf7bc9145706d` |
| `dev/benchmarks/permanent_campaign/premeasure-v1-censored/process-0539-3-26-A/scratch.csv` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |

The 48 scratch paths named by `backend-ordering.csv` are not committed. Their
digests remain recorded in that canonical CSV, but their bytes cannot be
independently rehashed from this repository. This limitation does not drop an
outcome: the committed CSV contains all 48 execution rows and four summaries.

## Rule 1 and rule 2 evidence

| Path | SHA-256 | Contract supported |
|---|---|---|
| `dev/studies/047b62ed/permanent-campaign-20260814T230032Z-2085453-shared-equivalence.csv` | `7d75c06e0f6761c4ffd166e4c1b0aa174a4fdc434b50c540cb7ef7b6d0527cd7` | Shared per-matrix candidate/reference equivalence. |
| `dev/benchmarks/permanent_campaign/exact-anchors.csv` | `688ac3f425d1eb57c7f81f0538c4464375a1b8bba1426dfd1b3ed79a8582adc7` | Validation anchors. |
| `dev/studies/6c7fcb38/hip-resource-usage-20260814T172506Z-1610002/receipt.txt` | `2828c0ae1c8fa79df9dd4024d4c23170760b3c521bcf114792e24d911284b173` | HIP capability and resource observations. |

All 124 selection-cohort configurations pass rules 1 and 2 at their nominated
cells. The exclusions below prevent absent, unsupported, or study-only
candidates from being treated as additional selectable arms.

## Candidate provenance

### Premeasure-v1 matched cohort

The following values apply to every candidate in the 60-cell premeasure-v1
cohort; the cell table below supplies each candidate's $(q,n)$, batch size,
actual repetition counts, outcome counts, and rate.

| Required item | Recorded value |
|---|---|
| Physical host | Repository benchmark host `fraktaali`; runtime fingerprint AMD Ryzen 9 5900X plus Radeon device `card0` unique identity `D4124100`. |
| CPU / GPU | AMD Ryzen 9 5900X, 12 physical cores / 24 logical CPUs; AMD Radeon RX 6950 XT `gfx1030`. |
| Accelerator runtime / driver | ROCm 7.2.4; Linux amdgpu driver/kernel `7.1.8-arch1-3`. |
| Source revision | `1350d5b46cd541093882537a89ea35db05a7afc5`; committed collector projection at `5e2799f7485280d717868f66a718275216f2993a`. |
| Compiler | `rustc 1.95.0 (59807616e 2026-04-14)`; the preparation script builds with `cargo +1.95.0`. Later ambient runtime probes of 1.97.0 are retained in the projection and are not the build compiler. |
| Build | Release, `hip` feature, thin LTO, one codegen unit; harness SHA-256 `1198cce47de06a6793fd1b5d880d559f020acffebc3d237137fa5c1326775606`; the complete four-binary chain is retained in every candidate row. |
| Workers | CPU batch and intra-matrix arms: 24 Rayon workers; generic Ryser: one CPU worker; accelerator: one GPU worker. |
| Power policy | CPU governor `powersave`; no fixed-frequency claim. |
| Warm-up / stopping | One locked 90-second whole-machine warm-up at schedule position 0; every process has 3 seconds of configuration warm-up, at least five repetitions and five timed seconds subject to the fixed 120-second cap; 12 planned fresh processes per configuration; no replacement or result-dependent extension. |

### 296a41c9 matched frontier cohort

| Required item | Recorded value |
|---|---|
| Physical host | Repository benchmark host `fraktaali`; AMD Ryzen 9 5900X and AMD Radeon RX 6950 XT. |
| CPU / GPU | 12 physical cores / 24 logical CPUs; AVX2 present, AVX-512F absent; Radeon RX 6950 XT `gfx1030`. |
| Accelerator runtime / driver | ROCm 7.2.4; Linux amdgpu driver/kernel `7.1.6-arch1-1`. |
| Source revision | Harness `414d31f8184a398deee946f151134511522dfca3`; dependency tree `d950bbb883845429d378aa2708ae7406b06fa6bc`. |
| Compiler | `rustc 1.95.0 (59807616e 2026-04-14)`; Cargo 1.95.0. Ambient 1.97 probes are preserved separately and are not the build compiler. |
| Build | Release plus `hip`; executable SHA-256 `6e24533cfbac987a0cec20af02f9dfb0a7bd9ce12c9e80cbdc00cad72150ccad`. |
| Workers | Intra-matrix and batch Rayon: 24; accelerator: one GPU worker. |
| Power policy | CPU governor `powersave`; no fixed-frequency claim. |
| Warm-up / stopping | One initial locked 90-second machine warm-up, at least 3 seconds of per-configuration warm-up, exact repetitions shown below, 12 fresh processes per configuration, and a fixed 120-second cap; no restart, replacement, or discard. |

## Exclusion ledger

- **Rule 1.** The nominated selection arms pass the shared equivalence and
  anchor evidence above. A backend without a cell-applicable shared-suite row
  is excluded before timing and is not silently promoted from a neighbouring
  order or from evaluation-only evidence.
- **Rule 2.** The in-tree F5 registry has no intra-matrix Rayon or AVX2
  permanent backend. For F7 at $n>16$, packed scalar, packed batch/Rayon, and
  AVX2 paths exceed the documented 16-lane limit; intra-matrix Rayon is not
  exposed. Those candidates are capability-excluded. The study-only
  `fold-gf3`, `f5-three-plane`, and `f7-three-plane-permanent` prototypes are
  not production `Backend` arms. The 2026-08-17 owner decision keeps them out
  of freeze selection: q3's `fold-gf3` interior observations at $n=16,20,24$
  remain preserved under the q3 no-go, while the retained q5 and q7 prototype
  observations remain study evidence only.
- **Rule 3.** Exactly five premeasure configurations fail the literal 12/12
  finite rule: `(3,26)` accelerator has 11 measured plus the signal-censored
  position 539; `(3,27)` accelerator has 5 measured plus 7 harness censors;
  and `(5,21)`, `(5,22)`, `(5,23)` accelerator each have 0 measured plus 12
  harness censors. No censor is replaced or omitted. In particular, at
  `(3,27)` the ineligible accelerator's finite-only mean is 38.750780/s,
  greater than intra-matrix Rayon's complete-cohort mean 33.762700/s; rule 3
  still excludes the accelerator and selects `intra_matrix_parallel`.
- **Rule 4.** The slower complete arm is excluded in each two-eligible-arm row
  below. At `(5,24)` and `(7,20)` 296a41c9 preregistered only one configuration,
  so there is no same-cohort comparison: `(5,24)` selects `batch_parallel` and
  preserves the q5 study's censored accelerator outcomes; `(7,20)` selects
  `accelerator` and preserves the study's generic/accelerator and excluded
  prototype outcomes. The q3/q5/q7 study grids have different source, binary,
  workload schedule, and timing protocol and are never ranked against
  premeasure-v1 or 296a41c9. The 296a41c9 `(3,28)` accelerator/intra-matrix
  comparison is ranked only inside 296a41c9.

The old feasibility, determinant-cost, and historical draft artifacts are not
ranking inputs. Determinant timing is evaluation-only, and the drafts are not
bound by the final manifest.

## Cell decisions

Rates are arithmetic means of finite per-process composite
draw-pack-evaluate-count rates. A rate printed for an ineligible arm is an
observed finite-only quantity and is explicitly not ranked. Repetition lists
are in schedule order. `P` denotes the premeasure-v1 ledger and `F` denotes
`backend-ordering.csv`, with the full paths and hashes above.

| q | n | Selected backend | Candidate arm A: rule-3 result | Candidate arm B: rule-3 result | Rule-4 basis | Raw |
|---:|---:|---|---|---|---|:---:|
| 3 | 4 | `generic_ryser` | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 902162.952408/s; reps [33765,33792,34196,33814,33994,34001,33988,33951,33894,33622,34006,34076] | `generic_ryser` ($M=13204$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 5135276.462433/s; reps [1799,1871,1868,1867,1864,1801,1873,1868,1868,1867,1873,1879] | Ranked within premeasure-v1: generic_ryser 5135276.462433/s > batch_parallel 902162.952408/s. | P |
| 3 | 5 | `generic_ryser` | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 772000.624450/s; reps [28720,28994,28605,28524,28802,28713,28662,28918,28922,28659,28897,28708] | `generic_ryser` ($M=13204$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 2270626.263575/s; reps [844,844,842,843,842,840,841,844,840,845,845,844] | Ranked within premeasure-v1: generic_ryser 2270626.263575/s > batch_parallel 772000.624450/s. | P |
| 3 | 6 | `generic_ryser` | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 684535.742492/s; reps [25516,25595,25463,25548,25551,25331,25721,25461,25499,25574,25394,25377] | `generic_ryser` ($M=13204$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 984758.598000/s; reps [373,367,371,368,369,372,374,366,374,370,369,368] | Ranked within premeasure-v1: generic_ryser 984758.598000/s > batch_parallel 684535.742492/s. | P |
| 3 | 7 | `batch_parallel` | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 567358.670075/s; reps [21351,21284,21416,21316,21180,21063,21073,21106,21258,21493,21136,21328] | `generic_ryser` ($M=13204$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 411625.720133/s; reps [154,153,153,153,162,153,162,154,154,153,154,154] | Ranked within premeasure-v1: batch_parallel 567358.670075/s > generic_ryser 411625.720133/s. | P |
| 3 | 8 | `batch_parallel` | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 486371.514067/s; reps [18366,18426,18490,18432,18496,18597,18483,18261,18472,18102,18357,18298] | `generic_ryser` ($M=13204$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 183663.487425/s; reps [69,70,69,69,69,69,70,72,69,69,70,70] | Ranked within premeasure-v1: batch_parallel 486371.514067/s > generic_ryser 183663.487425/s. | P |
| 3 | 9 | `batch_parallel` | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 416820.371900/s; reps [16093,16133,15921,15960,16056,16087,15960,15963,15966,16099,16049,16081] | `generic_ryser` ($M=13204$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 82078.332683/s; reps [32,32,31,31,31,31,31,31,32,31,31,32] | Ranked within premeasure-v1: batch_parallel 416820.371900/s > generic_ryser 82078.332683/s. | P |
| 3 | 10 | `batch_parallel` | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 347270.629058/s; reps [13674,13755,13792,13758,13606,13750,13795,13682,13760,13761,13725,13778] | `generic_ryser` ($M=13204$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 35840.328892/s; reps [14,14,14,14,14,14,14,14,14,14,14,14] | Ranked within premeasure-v1: batch_parallel 347270.629058/s > generic_ryser 35840.328892/s. | P |
| 3 | 11 | `batch_parallel` | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 279859.361042/s; reps [11499,11373,11540,11485,11435,11399,11499,11494,11501,11444,11461,11434] | `generic_ryser` ($M=13204$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 15413.346308/s; reps [6,6,6,6,6,6,6,6,6,6,6,6] | Ranked within premeasure-v1: batch_parallel 279859.361042/s > generic_ryser 15413.346308/s. | P |
| 3 | 12 | `batch_parallel` | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 213876.713092/s; reps [9166,9160,9252,9191,9216,9169,9214,9167,9178,9150,9205,9258] | `generic_ryser` ($M=13204$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 6852.251250/s; reps [5,5,5,5,5,5,5,5,5,5,5,5] | Ranked within premeasure-v1: batch_parallel 213876.713092/s > generic_ryser 6852.251250/s. | P |
| 3 | 13 | `batch_parallel` | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 157463.721442/s; reps [7001,7035,7007,7033,6994,6944,6994,6997,7023,7018,6998,7024] | `generic_ryser` ($M=610$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 3066.141467/s; reps [26,26,26,26,26,26,26,26,26,26,26,26] | Ranked within premeasure-v1: batch_parallel 157463.721442/s > generic_ryser 3066.141467/s. | P |
| 3 | 14 | `batch_parallel` | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 102094.084975/s; reps [4765,4768,4776,4777,4761,4775,4736,4760,4748,4768,4749,4768] | `generic_ryser` ($M=610$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 1403.790442/s; reps [12,12,12,12,12,12,12,12,12,12,12,12] | Ranked within premeasure-v1: batch_parallel 102094.084975/s > generic_ryser 1403.790442/s. | P |
| 3 | 15 | `batch_parallel` | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 60972.374208/s; reps [2932,2946,2945,2944,2950,2945,2951,2965,2945,2970,2941,2952] | `generic_ryser` ($M=610$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 653.565692/s; reps [6,6,6,6,6,6,6,6,6,6,6,6] | Ranked within premeasure-v1: batch_parallel 60972.374208/s > generic_ryser 653.565692/s. | P |
| 3 | 16 | `accelerator` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 57939.895700/s; reps [280,279,280,278,278,279,279,279,278,278,278,278] | `intra_matrix_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 4324.215908/s; reps [225,225,224,226,225,226,224,224,225,226,225,225] | Ranked within premeasure-v1: accelerator 57939.895700/s > intra_matrix_parallel 4324.215908/s. | P |
| 3 | 17 | `accelerator` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 32966.236125/s; reps [160,160,159,160,160,160,159,160,159,160,159,160] | `intra_matrix_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 3889.954358/s; reps [202,202,208,202,202,203,202,203,200,202,201,202] | Ranked within premeasure-v1: accelerator 32966.236125/s > intra_matrix_parallel 3889.954358/s. | P |
| 3 | 18 | `accelerator` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 17912.545233/s; reps [87,87,88,87,87,87,87,87,87,88,87,87] | `intra_matrix_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 3390.324250/s; reps [176,175,179,171,178,179,178,179,172,172,180,178] | Ranked within premeasure-v1: accelerator 17912.545233/s > intra_matrix_parallel 3390.324250/s. | P |
| 3 | 19 | `accelerator` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 9413.475117/s; reps [46,46,46,46,46,46,46,46,46,46,46,46] | `intra_matrix_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 3011.502500/s; reps [157,157,157,157,157,157,157,156,157,156,157,157] | Ranked within premeasure-v1: accelerator 9413.475117/s > intra_matrix_parallel 3011.502500/s. | P |
| 3 | 20 | `accelerator` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 4843.167658/s; reps [24,24,24,24,24,24,24,24,24,24,24,24] | `intra_matrix_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 2892.148117/s; reps [151,150,151,150,151,151,151,150,150,151,151,151] | Ranked within premeasure-v1: accelerator 4843.167658/s > intra_matrix_parallel 2892.148117/s. | P |
| 3 | 21 | `accelerator` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 2457.553792/s; reps [12,13,13,13,12,13,12,13,12,12,12,12] | `intra_matrix_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 1568.374858/s; reps [82,82,82,82,82,82,82,82,82,82,82,82] | Ranked within premeasure-v1: accelerator 2457.553792/s > intra_matrix_parallel 1568.374858/s. | P |
| 3 | 22 | `accelerator` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 1239.831075/s; reps [7,7,7,7,7,7,7,7,7,7,7,7] | `intra_matrix_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 1018.768108/s; reps [54,53,53,53,54,54,53,53,53,53,53,53] | Ranked within premeasure-v1: accelerator 1239.831075/s > intra_matrix_parallel 1018.768108/s. | P |
| 3 | 23 | `accelerator` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 621.990608/s; reps [5,5,5,5,5,5,5,5,5,5,5,5] | `intra_matrix_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 527.446158/s; reps [28,28,28,28,28,28,28,28,28,28,28,28] | Ranked within premeasure-v1: accelerator 621.990608/s > intra_matrix_parallel 527.446158/s. | P |
| 3 | 24 | `accelerator` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 311.390717/s; reps [5,5,5,5,5,5,5,5,5,5,5,5] | `intra_matrix_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 283.095508/s; reps [15,15,15,15,15,15,15,15,15,15,15,15] | Ranked within premeasure-v1: accelerator 311.390717/s > intra_matrix_parallel 283.095508/s. | P |
| 3 | 25 | `accelerator` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 155.287575/s; reps [5,5,5,5,5,5,5,5,5,5,5,5] | `intra_matrix_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 143.484025/s; reps [8,8,8,8,8,8,8,8,8,8,8,8] | Ranked within premeasure-v1: accelerator 155.287575/s > intra_matrix_parallel 143.484025/s. | P |
| 3 | 26 | `intra_matrix_parallel` | `accelerator` ($M=1024$): 11 measured + 0 harness-censored + 1 signal-censored / 12; INELIGIBLE; finite-only mean 56.350236/s (not ranked); reps [5,5,5,5,5,signal-censored,5,5,5,5,5,5] | `intra_matrix_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 66.095717/s; reps [5,5,5,5,5,5,5,5,5,5,5,5] | Only `intra_matrix_parallel` remains eligible; `accelerator` is rule-3-ineligible. | P |
| 3 | 27 | `intra_matrix_parallel` | `accelerator` ($M=1024$): 5 measured + 7 harness-censored + 0 signal-censored / 12; INELIGIBLE; finite-only mean 38.750780/s (not ranked); reps [3,3,3,3,3,3,3,5,5,5,5,5] | `intra_matrix_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 33.762700/s; reps [5,5,5,5,5,5,5,5,5,5,5,5] | Only `intra_matrix_parallel` remains eligible; `accelerator` is rule-3-ineligible. Its higher finite-only observed mean is not ranked. | P |
| 3 | 28 | `accelerator` | `accelerator` ($M=1024$): 12 measured + 0 censored / 12; eligible; mean 19.361167/s; reps [3,3,3,3,3,3,3,3,3,3,3,3] | `intra_matrix_parallel` ($M=96$): 12 measured + 0 censored / 12; eligible; mean 18.066058/s; reps [5,5,5,5,5,5,5,5,5,5,5,5] | Ranked within 296a41c9: accelerator 19.361167/s > intra_matrix_parallel 18.066058/s. | F |
| 5 | 4 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 577682.197800/s; reps [2637,2641,2608,2615,2652,2647,2652,2629,2641,2631,2631,2647] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 758942.050175/s; reps [26971,26960,27072,27031,26981,27180,27238,27241,27229,27176,27183,26990] | Ranked within premeasure-v1: batch_parallel 758942.050175/s > accelerator 577682.197800/s. | P |
| 5 | 5 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 502207.826867/s; reps [2280,2284,2288,2288,2287,2279,2289,2286,2282,2281,2282,2280] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 666180.427375/s; reps [23625,23601,23544,23566,23662,23630,23428,23765,23554,23442,23867,23592] | Ranked within premeasure-v1: batch_parallel 666180.427375/s > accelerator 502207.826867/s. | P |
| 5 | 6 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 420151.416317/s; reps [1911,1914,1908,1915,1918,1916,1895,1904,1919,1916,1919,1918] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 539008.498592/s; reps [19239,19212,19189,19223,19171,19330,19240,19232,19261,19193,19214,19212] | Ranked within premeasure-v1: batch_parallel 539008.498592/s > accelerator 420151.416317/s. | P |
| 5 | 7 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 325655.046492/s; reps [1492,1494,1490,1493,1492,1495,1494,1493,1495,1497,1496,1493] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 433595.807950/s; reps [15946,15956,15969,15946,15935,15891,15870,15853,15936,15840,15831,15888] | Ranked within premeasure-v1: batch_parallel 433595.807950/s > accelerator 325655.046492/s. | P |
| 5 | 8 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 223974.317942/s; reps [1059,1070,1028,1010,1046,1038,1054,1065,1005,1036,1020,1069] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 343825.515383/s; reps [13070,13146,13046,13111,13130,13080,13098,13113,13076,13119,13083,13129] | Ranked within premeasure-v1: batch_parallel 343825.515383/s > accelerator 223974.317942/s. | P |
| 5 | 9 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 144943.844025/s; reps [695,693,687,688,680,685,684,693,685,668,691,656] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 258574.423383/s; reps [10393,10380,10465,10465,10436,10367,10467,10476,10425,10425,10463,10424] | Ranked within premeasure-v1: batch_parallel 258574.423383/s > accelerator 144943.844025/s. | P |
| 5 | 10 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 83568.306817/s; reps [401,392,397,407,399,396,405,405,398,395,404,394] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 179303.874575/s; reps [7640,7655,7643,7631,7655,7624,7642,7638,7650,7637,7628,7640] | Ranked within premeasure-v1: batch_parallel 179303.874575/s > accelerator 83568.306817/s. | P |
| 5 | 11 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 44421.333800/s; reps [215,214,215,214,214,215,214,214,214,215,214,215] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 107277.334592/s; reps [4889,4880,4891,4876,4897,4875,4888,4887,4881,4888,4889,4884] | Ranked within premeasure-v1: batch_parallel 107277.334592/s > accelerator 44421.333800/s. | P |
| 5 | 12 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 22514.079875/s; reps [109,110,109,110,110,110,109,110,109,110,109,110] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 59093.081192/s; reps [2838,2851,2843,2840,2845,2835,2840,2834,2839,2841,2846,2839] | Ranked within premeasure-v1: batch_parallel 59093.081192/s > accelerator 22514.079875/s. | P |
| 5 | 13 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 11050.058925/s; reps [54,54,54,54,54,54,54,54,54,54,54,54] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 30747.706508/s; reps [1534,1535,1539,1538,1540,1537,1541,1530,1535,1527,1534,1524] | Ranked within premeasure-v1: batch_parallel 30747.706508/s > accelerator 11050.058925/s. | P |
| 5 | 14 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 5330.149908/s; reps [26,26,26,26,26,26,26,27,26,27,26,26] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 15068.090300/s; reps [766,766,766,765,768,764,768,767,768,768,768,769] | Ranked within premeasure-v1: batch_parallel 15068.090300/s > accelerator 5330.149908/s. | P |
| 5 | 15 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 2553.870017/s; reps [13,13,13,13,13,13,13,13,13,13,13,13] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 7171.258683/s; reps [371,371,370,369,370,368,369,367,371,371,369,369] | Ranked within premeasure-v1: batch_parallel 7171.258683/s > accelerator 2553.870017/s. | P |
| 5 | 16 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 1220.259117/s; reps [6,6,6,6,6,6,6,6,6,6,6,6] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 3524.199033/s; reps [184,185,182,182,182,182,184,183,183,184,183,182] | Ranked within premeasure-v1: batch_parallel 3524.199033/s > accelerator 1220.259117/s. | P |
| 5 | 17 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 582.558550/s; reps [5,5,5,5,5,5,5,5,5,5,5,5] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 1698.540608/s; reps [88,88,89,89,89,88,89,89,89,88,88,88] | Ranked within premeasure-v1: batch_parallel 1698.540608/s > accelerator 582.558550/s. | P |
| 5 | 18 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 278.597392/s; reps [5,5,5,5,5,5,5,5,5,5,5,5] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 834.974392/s; reps [44,44,44,44,44,43,44,44,45,44,44,44] | Ranked within premeasure-v1: batch_parallel 834.974392/s > accelerator 278.597392/s. | P |
| 5 | 19 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 133.350783/s; reps [5,5,5,5,5,5,5,5,5,5,5,5] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 404.728067/s; reps [21,22,22,22,21,22,21,22,22,21,22,22] | Ranked within premeasure-v1: batch_parallel 404.728067/s > accelerator 133.350783/s. | P |
| 5 | 20 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 63.915542/s; reps [5,5,5,5,5,5,5,5,5,5,5,5] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 197.787167/s; reps [11,11,11,11,11,11,11,11,11,11,11,11] | Ranked within premeasure-v1: batch_parallel 197.787167/s > accelerator 63.915542/s. | P |
| 5 | 21 | `batch_parallel` | `accelerator` ($M=1024$): 0 measured + 12 harness-censored + 0 signal-censored / 12; INELIGIBLE; no finite mean; reps [4,4,4,4,4,4,4,4,4,4,4,4] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 95.751133/s; reps [5,5,5,5,5,5,6,5,5,6,6,5] | Only `batch_parallel` remains eligible; `accelerator` is rule-3-ineligible. | P |
| 5 | 22 | `batch_parallel` | `accelerator` ($M=1024$): 0 measured + 12 harness-censored + 0 signal-censored / 12; INELIGIBLE; no finite mean; reps [2,2,2,2,2,2,2,2,2,2,2,2] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 44.407408/s; reps [5,5,5,5,5,5,5,5,5,5,5,5] | Only `batch_parallel` remains eligible; `accelerator` is rule-3-ineligible. | P |
| 5 | 23 | `batch_parallel` | `accelerator` ($M=1024$): 0 measured + 12 harness-censored + 0 signal-censored / 12; INELIGIBLE; no finite mean; reps [1,1,1,1,1,1,1,1,1,1,1,1] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 21.483283/s; reps [5,5,5,5,5,5,5,5,5,5,5,5] | Only `batch_parallel` remains eligible; `accelerator` is rule-3-ineligible. | P |
| 5 | 24 | `batch_parallel` | `batch_parallel` ($M=96$): 12 measured + 0 censored / 12; eligible; mean 11.573650/s; reps [5,5,5,5,5,5,5,5,5,5,5,5] | No second 296a41c9 arm: 0 planned, 0 measured, 0 censored; study accelerator outcomes are excluded above. | Only one preregistered 296a41c9 arm remains eligible; no cross-cohort ranking. | F |
| 7 | 4 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 635032.812300/s; reps [3008,3017,3025,3024,3026,2987,3017,2974,3012,3017,3011,2997] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 986918.918133/s; reps [41777,41077,41433,41785,41736,41322,41103,41574,41605,41616,41652,41732] | Ranked within premeasure-v1: batch_parallel 986918.918133/s > accelerator 635032.812300/s. | P |
| 7 | 5 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 563676.200225/s; reps [2665,2671,2674,2685,2675,2677,2681,2672,2674,2656,2679,2614] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 872952.518942/s; reps [36196,36046,36040,36168,36103,36267,36122,36288,36305,35951,36248,36491] | Ranked within premeasure-v1: batch_parallel 872952.518942/s > accelerator 563676.200225/s. | P |
| 7 | 6 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 449555.045958/s; reps [2252,2276,2267,2269,2279,1129,1669,2273,2268,2282,2280,2278] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 775050.650775/s; reps [31423,31475,31452,31462,32569,32395,30943,31282,32536,32509,32505,32457] | Ranked within premeasure-v1: batch_parallel 775050.650775/s > accelerator 449555.045958/s. | P |
| 7 | 7 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 366799.908242/s; reps [1756,1749,1745,1752,1741,1746,1719,1751,1750,1741,1734,1742] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 637681.167692/s; reps [26885,26927,27110,26994,26978,27014,25624,26894,27009,26937,24656,25766] | Ranked within premeasure-v1: batch_parallel 637681.167692/s > accelerator 366799.908242/s. | P |
| 7 | 8 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 251180.468633/s; reps [1202,1205,1204,1206,1206,1205,1202,1204,1203,1183,1200,1203] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 494980.898667/s; reps [21478,21355,21299,21323,19177,21340,21432,21332,21519,20516,21428,20651] | Ranked within premeasure-v1: batch_parallel 494980.898667/s > accelerator 251180.468633/s. | P |
| 7 | 9 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 150728.226475/s; reps [730,726,725,726,721,727,728,728,727,726,732,721] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 353896.712367/s; reps [15811,15590,15811,15672,15473,15612,15745,15595,15649,15796,15701,15693] | Ranked within premeasure-v1: batch_parallel 353896.712367/s > accelerator 150728.226475/s. | P |
| 7 | 10 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 82909.903292/s; reps [399,403,404,400,402,401,401,402,401,404,402,403] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 216211.120808/s; reps [10098,10068,9970,10107,10104,10075,10095,10092,10059,10038,10129,10108] | Ranked within premeasure-v1: batch_parallel 216211.120808/s > accelerator 82909.903292/s. | P |
| 7 | 11 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 42525.445733/s; reps [207,207,207,207,207,207,207,207,207,207,208,208] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 118056.080050/s; reps [5804,5805,5742,5753,5744,5749,5761,5760,5784,5707,5750,5746] | Ranked within premeasure-v1: batch_parallel 118056.080050/s > accelerator 42525.445733/s. | P |
| 7 | 12 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 20962.501092/s; reps [103,103,102,103,103,103,102,103,102,103,102,103] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 60773.838017/s; reps [3035,3051,3056,3051,3059,3027,3065,3069,3059,3058,3069,3058] | Ranked within premeasure-v1: batch_parallel 60773.838017/s > accelerator 20962.501092/s. | P |
| 7 | 13 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 10148.296758/s; reps [50,50,50,50,50,50,50,50,50,50,50,50] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 30262.449583/s; reps [1545,1540,1547,1538,1548,1551,1545,1545,1550,1550,1552,1554] | Ranked within premeasure-v1: batch_parallel 30262.449583/s > accelerator 10148.296758/s. | P |
| 7 | 14 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 4860.263067/s; reps [24,24,24,24,24,24,24,24,24,24,24,24] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 14699.630892/s; reps [759,761,759,758,759,756,758,758,758,757,762,758] | Ranked within premeasure-v1: batch_parallel 14699.630892/s > accelerator 4860.263067/s. | P |
| 7 | 15 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 2312.228725/s; reps [12,12,12,12,12,12,12,12,12,12,12,12] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 6418.399642/s; reps [368,367,369,369,148,167,366,370,368,367,367,370] | Ranked within premeasure-v1: batch_parallel 6418.399642/s > accelerator 2312.228725/s. | P |
| 7 | 16 | `batch_parallel` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 1106.050450/s; reps [6,6,6,6,6,6,6,6,6,6,6,6] | `batch_parallel` ($M=96$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 3424.238917/s; reps [179,178,179,179,176,178,178,179,179,178,178,179] | Ranked within premeasure-v1: batch_parallel 3424.238917/s > accelerator 1106.050450/s. | P |
| 7 | 17 | `accelerator` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 527.852925/s; reps [5,5,5,5,5,5,5,5,5,5,5,5] | `generic_ryser` ($M=31$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 143.874275/s; reps [24,24,24,24,24,24,24,24,24,24,24,24] | Ranked within premeasure-v1: accelerator 527.852925/s > generic_ryser 143.874275/s. | P |
| 7 | 18 | `accelerator` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 252.217850/s; reps [5,5,5,5,5,5,5,5,5,5,5,5] | `generic_ryser` ($M=31$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 67.725158/s; reps [11,11,11,11,11,11,11,11,11,11,11,11] | Ranked within premeasure-v1: accelerator 252.217850/s > generic_ryser 67.725158/s. | P |
| 7 | 19 | `accelerator` | `accelerator` ($M=1024$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 120.875183/s; reps [5,5,5,5,5,5,5,5,5,5,5,5] | `generic_ryser` ($M=31$): 12 measured + 0 harness-censored + 0 signal-censored / 12; eligible; mean 32.059292/s; reps [6,6,6,6,6,6,6,6,6,6,6,6] | Ranked within premeasure-v1: accelerator 120.875183/s > generic_ryser 32.059292/s. | P |
| 7 | 20 | `accelerator` | `accelerator` ($M=1024$): 12 measured + 0 censored / 12; eligible; mean 57.903017/s; reps [5,5,5,5,5,5,5,5,5,5,5,5] | No second 296a41c9 arm: 0 planned, 0 measured, 0 censored; study generic/prototype outcomes are excluded above. | Only one preregistered 296a41c9 arm remains eligible; no cross-cohort ranking. | F |

## Completeness checks

The table has 63 addressable cell rows. Selected-backend totals are `accelerator` 15, `batch_parallel` 43, `generic_ryser` 3, `intra_matrix_parallel` 2.
The premeasure portion covers q3 $n=4\ldots27$, q5 $n=4\ldots23$, and
q7 $n=4\ldots19$; 296a41c9 supplies $(3,28)$, $(5,24)$, and $(7,20)$.
Together they are exactly the protocol's 63-cell universe.
