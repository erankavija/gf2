# Logical candidate premeasurement evidence (jit:bc091474)

The [portfolio](portfolio.md) was committed before either experimental arm was built or sampled. The accepted [baseline and profile](../2037941f-profile-and-optimize-mid-range-buffer-operations/logical-baselines.md) motivates the two AVX2 loop identities and records the NR constructor's no-candidate outcome. The 64-word generic dispatch-hoist no-win and already-hoisted consumers remain outside this study.

## Build and semantic result

Rust 1.95 release builds use the ordinary runtime feature selector. The baseline has empty Rust flags; the two private candidate executables carry `--cfg gf2_xor_unroll2` and `--cfg gf2_xor_unroll4` respectively. The existing `avx2_xor_into` is the only changed kernel body; it has an explicit pointer, length, aliasing, AVX2-gating, and scalar-fallback safety contract. No new unsafe function, dispatcher, allocation, dependency, or public API is added. The experimental body is absent from ordinary builds.

The standalone harness contract passed 31 tests under each candidate's Rust 1.95 release build. Its shared oracle passed logical bit lengths 0, 1, 63, 64, and 65; aligned and eight-byte-offset XOR across 7/8/9/63/64/65/66 words; and full/tail63 public row operations over the same widths. The staged smoke records also show the scalar route at seven words and SIMD routes from eight words, with identical public selected paths across the paired arms.

Release disassembly of the measured `logical-arm` symbol is preserved below. The baseline loop already contains a compiler-generated two-vector iteration, but both explicit candidates produce distinct code. Symbol size is a code-complexity observation, not a speed result.

The [baseline](asm/baseline-avx2-xor.asm.txt), [factor 2](asm/unroll2-avx2-xor.asm.txt), and [factor 4](asm/unroll4-avx2-xor.asm.txt) assembly records each carry the measured binary digest, exact Rust flags, and symbol size in their headers.

## Canonical untimed smoke

Each plan pins the separate baseline and candidate executables, exact Rust flags, and 24 pilot pairs on every frozen cell. The shared `benchmark-ab-runner smoke` completed in two-cell bounded sessions against one append-only stage per plan. Each record reports 17 cells, 34 validation-arm dispatches, and zero timing windows. The staged logs had eight pauses followed by one terminal completion. `benchmark-ab-runner finalize` refused the smoke stage because it is non-timed.

| Candidate | Isolated XOR | Public row XOR |
|---|---|---|
| Factor 2 | [record](smoke-2-isolated.json), [plan](smoke-plans/2-isolated.plan.json) | [record](smoke-2-row.json), [plan](smoke-plans/2-row.plan.json) |
| Factor 4 | [record](smoke-4-isolated.json), [plan](smoke-plans/4-isolated.plan.json) | [record](smoke-4-row.json), [plan](smoke-plans/4-row.plan.json) |

## Overnight work and decision boundary

The [benchmark queue](../1a379447-zen3-cpu-performance/bench-window/queue.tsv) contains four pilot lines in the portfolio's fixed order, estimated at 15 minutes each, followed by two candidate profile lines at 8 minutes each. The profile lines reuse the existing nine-repetition perf and order-statistic summary machinery, restricted to its ten isolated/public-row cases. Both candidate profile drivers passed the untimed build-mode case-list check; the ordinary baseline build mode still lists its frozen 14 cases.

After each pilot, the issue launcher runs the existing execution-log checker and independent acceptance evaluator, then commits only that receipt directory and its family ledger, including ignored `Cargo.lock` snapshots. A retry with an existing receipt verifies and commits it without collecting new samples. A completed candidate profile is checked for nine finished repetitions before its output directory is committed. This lets later queue lines read a clean, committed ledger.

No candidate timings, adjusted confidence bounds, candidate profile results, or confirmation receipt exist yet. The first window must complete the four pilots and both profiles before the predeclared selection rule can choose one confirmatory identity or a no-candidate outcome. No production route is selected here.
