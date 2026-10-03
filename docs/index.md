# gf2 documentation

## Tutorials

Reproducible research workflows, placed per `@/inv/adapted-diataxis-placement`.

| Page | Workflow |
|---|---|
| [Finite-field linear algebra at scale](tutorials/field-linear-algebra-at-scale.md) | Dense multiplication, PLE solving, inversion and characteristic polynomials over a prime field and its extension, with identity checks |
| [Coded-modulation link simulation](tutorials/coded-modulation-link-simulation.md) | Checkpointed DVB-T2 BICM error-rate sweep through the `gf2-sim` preset, with frame-error-rate confidence intervals |

## How-to guides

Focused adoption tasks, placed per `@/inv/adapted-diataxis-placement`.

| Page | Task |
|---|---|
| [Run and extend the Lean 4 proofs](how-to/formal-verification.md) | Build the proofs, regenerate the Charon/Aeneas extraction, and prove a property of an extracted function |
| [Run a simulation campaign](how-to/run-simulation-campaigns.md) | Configure, run, stop and resume error-rate sweeps, and locate their output |
| [Reproduce a performance claim](how-to/reproduce-performance-evidence.md) | Rebuild a claim's measured revision, rerun its receipt command and compare the raw data with the receipt |
| [Select SIMD, GPU and parallel execution](how-to/select-acceleration.md) | Enable, check and tune the SIMD, HIP and rayon paths for `gf2-core` and `gf2-coding` workloads |

## Concepts

Current architecture and algorithmic choices, placed per `@/inv/adapted-diataxis-placement`.

| Page | Topic |
|---|---|
| [Finite-field arithmetic](concepts/finite-field-arithmetic.md) | Field families, representations, multiplication strategy and defining-polynomial choice |
| [Acceleration architecture](concepts/acceleration-architecture.md) | SIMD dispatch, HIP backend, hybrid executor and parallelism model |

## Reference

Supported configurations, limitations, evidence methodology and stable contracts, placed per `@/inv/adapted-diataxis-placement`.

| Page | Contents |
|---|---|
| [Supported configurations](reference/supported-configurations.md) | Toolchain, platforms, CPU and GPU requirements, Cargo features, installation and limitations |
| [Standards conformance](reference/standards-conformance.md) | Supported DVB-T2 and 5G NR code configurations, bit order and conformance evidence |
| [Performance evidence](reference/performance-evidence.md) | Measurement methodology and commit-pinned evidence for each performance claim |

## Crates

| Crate | Entry page |
|---|---|
| `gf2-core` | [README](../crates/gf2-core/README.md) |
| `gf2-coding` | [README](../crates/gf2-coding/README.md) |
| `gf2-algebra` | [README](../crates/gf2-algebra/README.md) |
| `gf2-sim` | [README](../crates/gf2-sim/README.md) |
