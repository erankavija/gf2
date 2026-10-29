# Integrated bottleneck profiles

> **Diátaxis Type:** Reference

Five profile series for `@/issue/78f0aafd` have nine completed sessions each. Their cases, repetition logs and per-series outputs live under this directory: [decoder](decoder-profile/), [BCH](bch-profile/), [NR encode](nr-encode-profile/), [byte field](byte-field-profile/) and [polynomial](polynomial-profile/). The series reuse the shared [profile session loop](../3be770d5/survey/profile-series.sh); the decoder series also uses the existing [profile launcher](../../../bench_results/ed3d490e/run-profile.sh) and [summarizer](../3be770d5/survey/summarize-profile.py). The canonical [workload set](../1362381c/workload-set.json) selects the FI-3 and FI-4 confirmation receipts.

## REQ-01: integrated decoder route

The decoder profile uses the production route under the committed coding envelope with Rust 1.95, a release build and `-C target-cpu=native`. Its [case declaration](decoder-cases.tsv) profiles DVB-T2 R12 and NR BG1 Z=384 at one, six and twelve physical cores and twenty-four logical CPUs, with one-frame cases at one core. The [profile summary](decoder-profile/profile-summary.json) has ten production cells across the two codes. The [build record](preparation/build-identity.json) pins the profiled `ldpc-batch-profile` executable, compiled production inputs and build environment; its executable SHA-256 is `805bdcb457c22d3f713763a29f909bad152fd28bf75eca6d0f1532bbde2eacfa`.

## REQ-02: material-gap coverage

The generated [accounting table](accounting.md) and [accounting data](accounting.json) project 46 primary material-gap cells. Forty-two cite a profile of the measured executable or an identity-matched integrated build, and none await a window. Four FI-3 cells cite the integrated decoder profile but remain `identity-unresolved`: `dvb-t2-r12-integrated-p12-b8`, `dvb-t2-r12-integrated-w1-b1`, `nr-bg1-z384-integrated-p12-b8` and `nr-bg1-z384-integrated-w1-b1`. The FI-3 receipt pins the measured arm executable digest and target flags, but the available build record does not link that executable to the profile's compiled production-input hashes. These four cells therefore do not yet establish profile coverage of the exact code measured.

The NR-encode, byte-field and polynomial series profile restored historical executables. [Restoration records](restoration.json) pin each rebuild command, its recorded build inputs, and matching expected and obtained executable digests. For these historical profiles, the accounting joins a profile case to a receipt only when the expanded case has the same measured executable digest and planned case identity. The wide-polynomial receipt declares no `RUSTFLAGS`; the recorded rebuild reproduces its executable digest with `-C target-cpu=native`. Untimed zero-window smoke runs for the restored executables remain non-passing; the profile sessions use the receipts' requested window counts and complete.

The consumed confirmation derivations preserve the invoker's margin ruling. The freezer refuses the core family's worthwhile and equivalence margins of 1.05 at 0.07 resolution; the ruling uses 1.08 for both. It refuses the decoder family's equivalence margin of 1.05 at 0.06 resolution; the ruling uses 1.07. It refuses the BCH family's equivalence margin of 1.1 at 0.13 resolution; the ruling uses 1.14. Each replacement is the smallest two-decimal margin strictly above one plus the pilot resolution, and no confirmation sample supports the refused margins. The derivations are recorded for [core](../31727c57/core/confirmation/derivation.txt), [decoder](../31727c57/decoder/confirmation/derivation.txt) and [BCH](../1358d6ab/bch/confirmation/derivation.txt).

## REQ-03: mechanism accounting

The generated table contains 119 profiled-cell rows and names allocation, instructions, dependencies, compiler spills, packing, cache traffic, bandwidth, orchestration and hardware-limit evidence for each row. It marks unmeasured mechanisms explicitly. Allocation-witness applicability is limited to its recorded workloads; IPC does not identify dependency latency; static stack references do not establish compiler spills; and the assembly listings are context without an executable-identity join. No approach to a hardware limit is claimed: host bandwidth and instruction/dependency bounds are unmeasured.

## Queue

The [queue file](window-queue.tsv) records the five completed series. No timed profile work remains queued for this issue.

## Open issues

Four FI-3 material-gap cells still lack a content-identity join between the measured arm executable and the integrated profile build; REQ-02 remains incomplete for those cells. A tracked follow-up needs to supply that identity link or obtain an authorized profile of the exact measured executable.

`test_refuses_to_resume_over_a_differing_quality_file` in `dev/bench_results/ed3d490e/test_run_profile.py` fails with `AssertionError: 0 != 2`. The fixture's redirected quality record does not redirect the catalogue digest used by `quality_files`; `@/issue/ff10cb51` tracks the shared fixture/resolution mismatch.
