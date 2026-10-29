# Integrated bottleneck profiles

> **Diátaxis Type:** Reference

Five profile series for `@/issue/78f0aafd` have nine completed sessions each. Their cases, repetition logs and per-series outputs live under this directory: [decoder](decoder-profile/), [BCH](bch-profile/), [NR encode](nr-encode-profile/), [byte field](byte-field-profile/) and [polynomial](polynomial-profile/). The [FI-3 arm series](fi3-arm-window.sh) is prepared for the benchmark window. The series reuse the shared [profile session loop](../3be770d5/survey/profile-series.sh); the decoder series also uses the existing [profile launcher](../../../bench_results/ed3d490e/run-profile.sh) and [summarizer](../3be770d5/survey/summarize-profile.py). The canonical [workload set](../1362381c/workload-set.json) selects the FI-3 and FI-4 confirmation receipts.

## REQ-01: integrated decoder route

The decoder profile uses the production route under the committed coding envelope with Rust 1.95, a release build and `-C target-cpu=native`. Its [case declaration](decoder-cases.tsv) profiles DVB-T2 R12 and NR BG1 Z=384 at one, six and twelve physical cores and twenty-four logical CPUs, with one-frame cases at one core. The [profile summary](decoder-profile/profile-summary.json) has ten production cells across the two codes. The [build record](preparation/build-identity.json) pins the profiled `ldpc-batch-profile` executable, compiled production inputs and build environment; its executable SHA-256 is `805bdcb457c22d3f713763a29f909bad152fd28bf75eca6d0f1532bbde2eacfa`.

## REQ-02: material-gap coverage

The generated [accounting table](accounting.md) and [accounting data](accounting.json) project 46 primary material-gap cells. Forty-two cite a profile of the measured executable or an identity-matched integrated build. Four FI-3 cells remain `identity-unresolved`; their integrated decoder profile is context because it profiles a separate executable. Each unresolved row names the prepared [FI-3 arm launcher](fi3-arm-window.sh).

The FI-3 case declaration in [arm-profiles.py](arm-profiles.py) expands those material-gap cells from the selected receipts' plans and frozen addenda. It uses their recorded inputs, arm environment, worker counts, resolved CPUs and per-worker batch sizes. The shared executable identity check requires the receipt digest `9af28c77634560523da233de28d713cd03c3a8e43b98b6e0fa254240d6a2ac26`; `GF2_PROFILE_ARM_ROOT` supplies identical bytes when the recorded executable path is unavailable. The series uses the shared arm session driver and loop for nine sessions and the existing whole-process counter summarizer in [make-accounting.py](make-accounting.py). Its counters include setup and warm-up. Coverage requires a completed `fi3-arm-profile/profile-summary.json` tied to the expanded cases, matching the receipt's executable digest, case, worker count and resolved CPUs. The four rows retain `identity-unresolved` while that summary is absent.

The [untimed preparation record](fi3-preparation.txt) records acceptance of the exact executable bytes, refusal of a differing file and dry preparation of all four session requests. No executable or perf invocation runs in that validation. The behavioral tests cover both summary states and reject digest, batch, worker and CPU mismatches.

The NR-encode, byte-field and polynomial series profile restored historical executables. [Restoration records](restoration.json) pin each rebuild command, its recorded build inputs, and matching expected and obtained executable digests. For these historical profiles, the accounting joins a profile case to a receipt only when the expanded case has the same measured executable digest and planned case identity. The wide-polynomial receipt declares no `RUSTFLAGS`; the recorded rebuild reproduces its executable digest with `-C target-cpu=native`. Untimed zero-window smoke runs for the restored executables remain non-passing; the profile sessions use the receipts' requested window counts and complete.

The consumed confirmation derivations preserve the invoker's margin ruling. The freezer refuses the core family's worthwhile and equivalence margins of 1.05 at 0.07 resolution; the ruling uses 1.08 for both. It refuses the decoder family's equivalence margin of 1.05 at 0.06 resolution; the ruling uses 1.07. It refuses the BCH family's equivalence margin of 1.1 at 0.13 resolution; the ruling uses 1.14. Each replacement is the smallest two-decimal margin strictly above one plus the pilot resolution, and no confirmation sample supports the refused margins. The derivations are recorded for [core](../31727c57/core/confirmation/derivation.txt), [decoder](../31727c57/decoder/confirmation/derivation.txt) and [BCH](../1358d6ab/bch/confirmation/derivation.txt).

## REQ-03: mechanism accounting

The generated table contains 119 profiled-cell rows and four FI-3 rows awaiting the window. It names allocation, instructions, dependencies, compiler spills, packing, cache traffic, bandwidth, orchestration and hardware-limit evidence for each row and marks unmeasured mechanisms explicitly. Allocation-witness applicability is limited to its recorded workloads; IPC does not identify dependency latency; static stack references do not establish compiler spills; and the assembly listings are context without an executable-identity join. No approach to a hardware limit is claimed: host bandwidth and instruction/dependency bounds are unmeasured.

## Queue

The [local queue record](window-queue.tsv) lists the five completed series. The FI-3 arm series has this dispatch line for the overnight window; 12 minutes is an estimate, using the local NR-encode series estimate as a scheduling allowance. The command supplies the arm root explicitly and runs from the queued worktree root. The dispatcher owns insertion into the central queue.

```tsv
78f0aafd	.agents/worktrees/agent-78f0aafd	12	GF2_PROFILE_ARM_ROOT="/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-ed3d490e-run/dev/active/ed3d490e-zen3-ldpc-frame-simd/survey/arms/target/release" bash dev/active/1a379447-zen3-cpu-performance/78f0aafd/fi3-arm-window.sh
```

The launcher generates the per-series summary and accounting after the shared loop completes. `python3 -B dev/active/1a379447-zen3-cpu-performance/78f0aafd/make-accounting.py --check` verifies the committed projection. The preparation validation regenerates both accounting files twice with identical bytes.

## Open issues

REQ-02 remains incomplete for four FI-3 material-gap cells until the queued exact-executable profile completes. Its generated summary supplies the measured executable join; the separate native decoder profile does not establish that join.

`test_refuses_to_resume_over_a_differing_quality_file` in `dev/bench_results/ed3d490e/test_run_profile.py` fails with `AssertionError: 0 != 2`. The fixture's redirected quality record does not redirect the catalogue digest used by `quality_files`; `@/issue/ff10cb51` tracks the shared fixture/resolution mismatch.
