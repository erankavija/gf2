# Dispatch fallback verification (jit:63bad95d)

> **Diátaxis Type:** Research

This record covers REQ-10 of issue `63bad95d`: a missing feature and an unknown
host select the established deterministic fallback, on standalone `gf2-core`
and on the `gf2-coding` and `gf2-sim` dependency builds. Claim identifiers in
backticks resolve in [`survey/source-evidence.json`](survey/source-evidence.json),
written by `python3 -B survey/make-source-evidence.py`; that ledger tracks the
current tree. Every command runs with Rust 1.95 and takes no timing window, no
host lock and no ignored test.

## What the fallback is

| Condition | Established fallback | Mechanism |
|---|---|---|
| The `simd` cargo feature of `gf2-core` is off | The portable route of every entry point | `core-simd-optional`, `bit-threshold-read`, `transpose-lane-selection`, `shift-route-selection` |
| The feature is on and the host lacks the processor feature a bundle needs | The portable route of that bundle's entry points | `logical-bundle-detect`, `transpose-lane-feature-gate`, `shift-feature-gate`, `clmul-wide-ymm-predicate`, `clmul-batch-feature-gate` |
| The process installs no profile, which is every process on a host without a measured profile | The conservative table, frozen at first access | `tuning-install-one-shot`, `tuning-frozen-before-install` |
| The build does not set `gf2_tuning_baked`, which no cargo feature does | The conservative compile-time constants | `core-baked-cfg-declared`, `bit-threshold-default-alias`, `tuning-baked-module-gate` |

No production crate installs a profile; an application does so explicitly
(`tuning-install-one-shot`).

## The shared contract

`gf2_core::dispatch_contract::assert_fallback_contract`, behind the
`test-support` feature (`contract-function`), asserts on the build it is
compiled into:

- a process without an installed profile resolves the conservative core
  section (`contract-unprofiled`);
- every reporter names the portable route below its compile-time threshold, in
  a build without `simd` and on a host without the kernel bundle, and the
  kernel route otherwise;
- the thresholds are the conservative ones unless the build is baked;
- a build without `simd` takes portable routes only
  (`contract-missing-feature`).

It returns what each reporter answered. Three suites run it: `gf2-core`
(`contract-core-test`), `gf2-coding` (`contract-coding-test`) and `gf2-sim`
(`contract-sim-test`); the two dependent suites also assert which
`gf2-core` feature their own dependency graph selects. The contract covers the
entry points that have a reporter in every build; `transpose_block_lane`
(`transpose-lane-reporter`) is the reporter this issue adds. The run that
preceded the contract module is
[`survey/test-logs/before-contract.txt`](survey/test-logs/before-contract.txt).

## Build configurations

`survey/run-route-witnesses.sh` (no arguments) runs the contract on each
configuration, writes the log each row links and the reporters' answers under
[`survey/route-witness/`](survey/route-witness/dependency-features.tsv); the
generated [route inventory](route-inventory.md#route-witnesses) tabulates the
answers per configuration and the features cargo resolves for the kernel
packages in each package's ordinary dependency build.

| Configuration | Selection of the test run | Log |
|---|---|---|
| Standalone `gf2-core`, default features | `-p gf2-core --test dispatch_fallback` | [core-default](survey/test-logs/core-default.txt) |
| Standalone `gf2-core`, `simd` | `-p gf2-core --features simd --test dispatch_fallback` | [core-simd](survey/test-logs/core-simd.txt) |
| Standalone `gf2-core`, every feature | `-p gf2-core --all-features --test dispatch_fallback` | [core-all-features](survey/test-logs/core-all-features.txt) |
| Standalone `gf2-core`, baked | `RUSTFLAGS="--cfg gf2_tuning_baked"`, `-p gf2-core --features simd,test-support,tuning-profile --test dispatch_fallback` | [core-baked](survey/test-logs/core-baked.txt) |
| `gf2-coding` dependency build | `-p gf2-coding --test core_dispatch_fallback` | [coding-default](survey/test-logs/coding-default.txt) |
| `gf2-coding` test build without default features | `-p gf2-coding --no-default-features --test core_dispatch_fallback` | [coding-no-default-features](survey/test-logs/coding-no-default-features.txt) |
| `gf2-sim` dependency build | `-p gf2-sim --test core_dispatch_fallback` | [sim-default](survey/test-logs/sim-default.txt) |

Each log opens with the toolchain and the full command and closes with the
exit status. What the configurations establish:

- **Missing feature, standalone.** `core-default` builds `gf2-core` without
  `simd` (`core-default-features`); every reporter names its portable route at
  every input.
- **`--all-features` and the baked profile.** `core-all-features` reports the
  baked configuration absent and the same routes as `core-simd`; only
  `core-baked` reports it present. The baked routes equal the conservative
  ones because every baked constant currently equals its conservative value
  (inventory, [selector table](route-inventory.md#selectors-the-tuning-sections-own)).
- **Unknown host.** Every configuration reports the core section as frozen
  before installation, which is the conservative table.
- **`gf2-coding`.** Its default build enables `gf2-core/simd`
  (`coding-default-simd`, `coding-simd-forwards`) and keeps the conservative
  thresholds. Its ordinary build without default features resolves `gf2-core`
  to the feature set of the standalone default build (dependency-features
  record), which `core-default` witnesses. The package's own test build cannot
  reach that feature set: its self dev-dependency keeps the default features,
  so `coding-no-default-features` reports the kernel routes.
- **`gf2-sim`.** It has no default feature (`sim-default-empty`) and takes
  `gf2-coding` and `gf2-algebra` with theirs (`sim-depends-coding-defaults`,
  `sim-depends-algebra-defaults`), so every `gf2-sim` build enables
  `gf2-core/simd`; the suite asserts that, and a `gf2-sim` build without the
  kernel feature does not exist.

## Missing processor feature

The host of these runs has every processor feature the bundles test, so an
undetected bundle is not observable there through detection. The families
that carry a test-only force switch hold the entry point on its portable lane
and compare it with the reference:

| Entry point | Shared-suite test |
|---|---|
| Wide carry-less product | `fallback-clmul-forced` |
| Residual shift | `fallback-shift-forced` |
| GF(2^8) product table | `fallback-gf256-forced` |
| BCH encode kernels of `gf2-coding` | `fallback-coding-forced` |

The automatic population-count routes are asserted against the detected
bundle (`fallback-popcount-automatic`). The logical word operations,
`BitMatrix::matvec` and the transpose block lane have no force switch: their
undetected-bundle branch is pinned by `ops-xor-resolver`,
`matvec-bundle-check` and `transpose-lane-selection`, and their portable route
is executed by the build without `simd`, which compiles the same scalar code.

## Profile lifecycle

`fallback-profile-access` and `fallback-profile-missing-section` cover access
before installation and an installed envelope without the core section;
`fallback-baked-independent` covers an installed profile that cannot move a
compile-time selection in an ordinary build.

## What CI executes

The workspace test step enables `simd` (`ci-feature-flags`), so CI runs the
three contract tests on the kernel build, and its scoped step builds the baked
configuration (`ci-baked-step`). No CI step runs `dispatch_fallback` on a
build without `simd`; the `core-default` log above is that run.
