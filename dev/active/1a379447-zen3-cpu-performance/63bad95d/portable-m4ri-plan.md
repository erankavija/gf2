# Portable comparator family plan (jit:63bad95d)

> **Diátaxis Type:** Research

REQ-09 asks for each gf2 build level measured against a competitor with an
equivalent targeting opportunity. This plan states how the family
`63bad95d-portable-matvec-vs-m4ri` answers that for the portable gf2 levels
on the dense matrix-vector product against M4RI [AlbrechtBard2026]. The frozen
rules are in [`portable-m4ri-addendum.md`](portable-m4ri-addendum.md); this
document gives the reasons and the reading of each outcome.

## Question

How does public `BitMatrix::matvec` from the portable gf2 build compare with
the matched M4RI product `mzd_mul(y, A, x, 0)` when M4RI is built without
host-specific targeting?

## The three M4RI families

| Family | gf2 level | M4RI build | Evidence |
|---|---|---|---|
| `2037941f-dense-matvec-vs-m4ri` | `conservative-portable` | `-O3 -march=native` | committed confirmation ([outcome](../50f0bd42/outcome.md)) |
| `63bad95d-native-matvec-vs-m4ri` | `native` | `-O3 -march=native` | [`native-m4ri-plan.md`](native-m4ri-plan.md) |
| `63bad95d-portable-matvec-vs-m4ri` | `conservative-portable` | `-O3`, no targeting option | this family |

The committed family pairs gf2 at its lowest level with M4RI at its highest.
The native family raises the gf2 side to M4RI's level. This family lowers the
M4RI side to gf2's, so each gf2 level has a comparator built with the
targeting opportunity gf2 has at that level.

M4RI stays an external comparator: GPL code, linked only into the benchmark
arm `dense-m4ri-arm` through the existing shim, with no M4RI source in the
repository's crates.

## Which gf2 levels the family measures

One arm, `conservative-portable`, for both portable levels. The
`tuned-portable` level of [`calibration-plan.md`](calibration-plan.md) §3
differs from the conservative level in selectors alone, and the one selector
`BitMatrix::matvec` reads is its lane threshold, a compile-time constant whose
baked value equals the conservative definition; the addendum cites the source
claims and the selector outcome. A `tuned-portable` arm would execute the same
lane at every shape, so splitting the six admitted cells between two arms of
equal selection would halve the shapes measured and add no contrast. If a
later profile changes that selector the statement no longer holds and the
tuned level needs its own cells.

## Arms and why the opportunity is equivalent

The addendum's arm table gives the exact flags of both sides and the limits
that remain. The portable M4RI install is built by the script that built the
qualified install,
[`run-m4ri-matvec-probe.sh`](../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/run-m4ri-matvec-probe.sh),
at the `portable` level of
[`m4ri-build-pins.sh`](../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/m4ri-build-pins.sh):
the same source archive, compiler and configure arguments, with
`-march=native` removed from `CFLAGS`. The build fails unless every compile
invocation in its verbose log runs the pinned compiler with the pinned flags
and none carries a targeting option, so a flag added by configure or by a make
override cannot pass. The install lives beside the qualified one, in its own
`portable-v1` directories under the primary checkout's `.agents/ext/92385645`.

[`m4ri-portable-build-record.txt`](../2037941f-profile-and-optimize-mid-range-buffer-operations/m4ri-portable-build-record.txt),
written by
[`record-m4ri-build.sh`](../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/record-m4ri-build.sh),
records what configure detected. This M4RI release adds no SIMD compiler
flag: its SSE2 paths are compiled where the compiler defines `__SSE2__`,
which is the baseline of the x86-64 target. The one host-derived input is the
three cache sizes configure reads; they are the same in both installs and
select no instruction set. The record also counts the library's instructions
that name a `ymm` or `zmm` register.

The C shim holds measured code: the bit-wise packing and unpacking loops and
M4RI's inline coordinate accessors are compiled into it. `build.rs` therefore
compiles the shim with `-march=native` only when it links the host-targeted
install, refuses an install whose own build record disagrees with the
requested level, and refuses a portable shim whose resolved compile command
carries a targeting option, whatever its origin. The arm's `--backend` line
in the harness validation record states the level it links and the shim's
compile command.

## Cells

Six cells, all `warm`, each a mirror of a committed comparator cell with its
seed, and the same six the native family mirrors, so one fixture is measured
by all three families: the two fresh mid-range cells of the committed
confirmation, the two retained-state cells of the same shapes, and the
one-bit and full-single-word small inputs REQ-09 names. P-20 admits six
comparisons on a first attempt; the addendum shows the arithmetic and the
harness contract recomputes it. The pilot measures exactly the confirmatory
cells in one cache state, so the freezer's whole-pilot resolution is the
confirmatory cells' own.

## Decision rule and margins

The equivalence margin, material-gap threshold and resolution ceiling are the
committed comparator family's, because the boundaries and the gf2 arm are the
same; the addendum's margin table states them. The pilot budget is one pilot
and one replacement for a voided pilot.
By invoker ruling of 2026-10-11, the pilot measured while another session loaded the host counts as the first trial, and the repeat on a quiet host (run id `v4-r2`) is the second.

## What each outcome means for REQ-09

The addendum's decision table maps each recorded decision to a reading. Per
cell:

| Reading | Meaning for REQ-09 |
|---|---|
| gf2 ahead | The portable gf2 build leads M4RI at equal targeting at this boundary. Beside the committed confirmation, the lead does not depend on M4RI's build level. |
| equivalent, or no material gap | The two are comparable at this boundary at equal targeting. Recorded as the result; no production change follows from a comparator. |
| gf2 behind | A material external gap at equal targeting. Recorded as a loss with its interval; it warrants attribution (profile and assembly of the gf2 route at that shape), not selection of an M4RI route. |
| inconclusive, unstable or not confirmatory | No reading for the cell; the one-attempt rule makes it terminal, and REQ-09's portable comparator leg stays open for that boundary. |
| `resolution-insufficient` after the pilot | No confirmation runs; the pilot is committed as exploratory evidence. |

A cell of this family and the cell it mirrors differ in the M4RI build level
alone, so the pair also shows what host targeting gives M4RI at that shape.
The fresh and the retained-state cells are read separately, as in the
mirrored family.

## Pilot outcome

The pilot receipt
[`v4-r1-pilot`](../../../bench_results/63bad95d/63bad95d-portable-matvec-vs-m4ri/v4-r1-pilot/acceptance-summary.md)
is accepted exploratory evidence; its arm table names the portable gf2 arm as
the baseline and its summary holds each cell's exploratory decision.
[`portable-m4ri-pilot-resolution.md`](portable-m4ri-pilot-resolution.md),
written by
[`survey/freeze-m4ri-confirmation.sh`](survey/freeze-m4ri-confirmation.sh),
applies the frozen resolution rule and records `resolution-insufficient`: its
table shows one fresh mid-range cell and the full-single-word cell above the
ceiling and the other cells within it. The family runs no confirmation and reserves
no comparison, and REQ-09's portable comparator leg has no confirmatory
reading.

The host was loaded when the pilot ran. Each session's `host-observation`
record in the receipt's
[`execution.log`](../../../bench_results/63bad95d/63bad95d-portable-matvec-vs-m4ri/v4-r1-pilot/execution.log)
states the `load_average` it observed, and the window runner's job-start line
for key `646f169b95c76bc5` in the primary checkout's
`.agents/bench-window/window.log` states the load at launch. The receipt
stands as measured.

## Execution

| Step | Command | Window |
|---|---|---|
| Portable M4RI install and its record | `M4RI_BUILD_LEVEL=portable record-m4ri-build.sh` | none |
| Build and untimed validation | `run-dense-harness.sh build --m4ri --native --m4ri-portable` | none |
| Pilot plan smoke | `run-dense-harness.sh window ... --m4ri-portable --smoke <record>` | none |
| Pilot | this family's queue line in [`queue.tsv`](../bench-window/queue.tsv) | first |
| Freeze the confirmation from the committed pilot receipt with the canonical freezer | | none |
| Confirmation | the same launcher with `--confirmation` | second |

The untimed validation record is
[`dense-harness-validation.txt`](../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/dense-harness-validation.txt):
on each cell's own fixture the gf2 builds pass the independent parity oracle
(`comparator-*` cases) and the portable M4RI arm agrees with the gf2 peer
(`m4ri-cell-*` cases of the portable arm). The pilot plan smoke is
[`survey/portable-m4ri-pilot-smoke.json`](survey/portable-m4ri-pilot-smoke.json).
