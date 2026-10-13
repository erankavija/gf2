# Native comparator measurement addendum

> **Diátaxis Type:** Reference
>
> **Addendum identity:** `63bad95d-native-matvec-vs-m4ri-v1`
>
> **Owning issue:** `63bad95d`
>
> **Frozen UTC:** `2026-10-08T12:43:56Z`

This addendum freezes one question, its cells, margins and resolution rule:
whether public `BitMatrix::matvec` from the `native` gf2 build holds its
position against the matched M4RI [AlbrechtBard2026] product when both sides
are compiled for the host. It specializes the
[Zen 3 measurement contract](../measurement-contract.md) and
`zen3-benchmark-protocol` version 4 in [`protocol.md`](../f547c394/protocol.md)
with [`amendment-v4.md`](../f547c394/amendment-v4.md). A campaign JSON addendum
transcribes the family below into the version-4
[schema](../f547c394/addendum.schema.json); it may supply content pins and a
pilot-derived resolution, and it may not change a cell, margin, limit or rule.

The matched operation, cost boundaries, charged components, fixtures, warm
cache policy, correctness oracle, sampling, estimator and receipt contents are
those of the
[dense-parity addendum](../2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum.md),
identity `2037941f-dense-parity-v2`, for its `matvec-vs-m4ri` question, as
implemented by the
[dense-parity harness](../2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-harness.md).
This document states only what differs. [`native-m4ri-plan.md`](native-m4ri-plan.md)
gives the design rationale and what each outcome means for the owning issue.

## Question and family

| Field | Value |
|---|---|
| Family identity | `63bad95d-native-matvec-vs-m4ri` |
| Purpose | `consumer-family` |
| Ledger | `dev/bench_results/63bad95d/native-matvec-vs-m4ri-ledger.jsonl`, created as an empty genesis file before the pilot |
| Holdout | none declared |

The family measures an external gap. It selects no production route and
changes no selector. No earlier family asks this question on a ledger, so its
first confirmation is attempt 1.

## Arms

| Arm | Build identity | Executable | Compiler and flags |
|---|---|---|---|
| baseline `m4ri-peer-gf2-native` | `native` | harness `dense-arm`, `simd` feature | Rust 1.95 release profile of the harness manifest with `RUSTFLAGS="-C target-cpu=native --cfg gf2_tuning_baked"` |
| candidate `m4ri-mzd-mul` | `external` | harness `dense-m4ri-arm` against the qualified M4RI install | library and C shim: the compiler and `-O3 -march=native` flags of the [build pins](../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/m4ri-build-pins.sh) and the [qualification record](../2037941f-profile-and-optimize-mid-range-buffer-operations/m4ri-probe-record.txt); Rust glue: release profile, no `target-cpu` override |

A speedup above one favours M4RI. The gf2 flags are the harness constant
`routes::NATIVE_RUSTFLAGS`, which the launcher builds with and the plan
records; the gf2 arm refuses a request unless its executable has the baked
configuration and statically enabled AVX2.

The candidate arm is the arm of `2037941f-dense-matvec-vs-m4ri`: the same
qualified install, pinned by the library digest in the qualification record,
the same shim and the same glue. The two families therefore differ in the gf2
build level alone.

Both sides have the same targeting opportunity: each compiler may emit every
instruction-set extension of the measuring host, in the library under test and
in the code that drives it, at its highest standard optimisation level. Two
differences remain and are limits of the comparison. The gf2 arm is linked
with thin LTO, the workspace release setting; M4RI is the shared library its
own build system produces. The candidate's Rust glue allocates the output
words, calls the shim and wraps the result, and is compiled for the default
target.

`BitMatrix::matvec` reads its lane threshold at compile time, so on this
route the baked configuration is the whole tuning profile and the arm installs
no profile envelope.

## Cells

Every cell mirrors one cell of `2037941f-dense-matvec-vs-m4ri`: the same
shape, cost boundary, `warm` cache state and workload seed, hence the same
fixture. A cell's identifier is the mirrored identifier with the prefix
`native-`. Every cell has the `comparator-gap` objective, the
`whole-consumer` metric with conversion and setup costs included,
`single-core-latency` scaling, the `single-core` arm and one worker.

| Cell | Shape | Boundary | Why it is declared | Role in the confirmation |
|---|---|---|---|---|
| `native-m4ri-gap-65x512-warm` | 65 rows, 512 columns | fresh whole consumer | confirmatory cell of the mirrored family | confirmatory |
| `native-m4ri-gap-65x4096-warm` | 65 rows, 4096 columns | fresh whole consumer | confirmatory cell of the mirrored family | confirmatory |
| `native-m4ri-gap-65x512-retained-warm` | 65 rows, 512 columns | retained state | closest boundary to operation throughput | confirmatory |
| `native-m4ri-gap-65x4096-retained-warm` | 65 rows, 4096 columns | retained state | closest boundary to operation throughput | confirmatory |
| `native-m4ri-gap-1x1-warm` | 1 row, 1 column | fresh whole consumer | smallest call: entry and dispatch overhead | confirmatory |
| `native-m4ri-gap-64x64-warm` | 64 rows, 64 columns | fresh whole consumer | small input on the scalar lane, one full word | confirmatory |

The pilot measures these six cells and no other, so the pilot-derived
resolution is taken over the cells the confirmation measures. The family
holds one cache state. The mirrored family's `63x63` and `65x65` shapes are
not mirrored: a first attempt admits six comparisons. The predeclared
unqualified shapes of the mirrored family have no cell here.

A retained-state cell is not a pure `mzd_mul` measurement and replaces no
fresh cell; the two boundaries are read separately.

## Sampling and comparisons

Every exploratory cell runs the protocol's pilot maximum of paired executions
and every confirmatory cell the protocol's confirmatory count, on fresh pairs.
No pilot pair enters the confirmation.

The confirmation reserves $m=6$ comparisons at attempt $t=1$. With the
protocol's shared settings, $\alpha_1 = 0.05/(1\cdot 2) = 0.025$ and
$\alpha_c = 0.025/6$, so each bootstrap tail holds
$10000\,\alpha_c/2 = 20.8\overline{3}$ expected draws, above the twenty P-20
requires; a seventh cell would fall below it. The harness contract test
`the_native_comparator_family_mirrors_comparator_cells_at_the_native_level`
recomputes this from `SHARED_SETTINGS`. A second confirmation is excluded by
the one-attempt rule and by the same floor.

## Margins and resolution

| Family | Worthwhile speedup | Equivalence margin | Material gap | Resolution ceiling | One plus ceiling | Complexity budget | Pilot trials per cell |
|---|---:|---:|---:|---:|---:|---|---:|
| native comparator | none | 1.05 | 1.10 | 0.040 | 1.040 | 0 unsafe kernels, 0 added source lines | 2 |

- The equivalence margin, the material gap and the resolution ceiling are the
  mirrored family's, for the same boundaries and the same candidate arm.
- The family holds no improvement or non-regression cell, so it declares no
  worthwhile speedup.
- A comparator outcome selects no production change, so the complexity budget
  is zero.
- The pilot budget covers the pilot and one replacement for a pilot voided
  under the protocol's voided-attempt rule.

The pilot-derived resolution $r$ is the largest relative bootstrap half-width
over the six cells at $\alpha_c$, rounded upward to the next 0.001 and pinned
with the pilot receipt and digest by the canonical freezer. If $r$ exceeds
the ceiling the family records `resolution-insufficient` and runs no
confirmation. Both margins strictly exceed one plus the ceiling:
$1.10 > 1.040$ and $1.05 > 1.040$.

## Decision rule

The rule reads the confirmation's acceptance summary and nothing else. Each
cell's interval $[\ell,u]$ is taken at $\alpha_c$, and the decision follows
protocol version 4 as the dense-parity addendum states it for the comparator
gap, with $\theta_{\mathrm{eq}}=1.05$ and the material gap $1.10$:

| Recorded decision | Condition | Reading |
|---|---|---|
| `regressed` | $u < 1/\theta_{\mathrm{eq}}$ | M4RI is slower beyond equivalence: gf2 ahead |
| `not-worse` | $\ell \ge 1/\theta_{\mathrm{eq}}$ and $\ell$ below the material gap | no material external gap; equivalent when also $u \le \theta_{\mathrm{eq}}$ |
| `improved` | $\ell$ at or above the material gap | material external gap: gf2 behind |
| `inconclusive` | otherwise | no reading |

Each cell is read on its own; the family forms no aggregate. Every cell stays
in the receipt with its recorded decision and outcome whatever the reading.

## Search and stopping

One pilot of the six cells, then one confirmation of the same six cells on the
same sources. No adaptive extension, no early stop, no reuse of pilot pairs. An `inconclusive`, `unstable` or
`not-confirmatory` cell is terminal for this family.
