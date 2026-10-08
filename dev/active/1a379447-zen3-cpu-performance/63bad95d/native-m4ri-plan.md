# Native comparator family plan (jit:63bad95d)

> **Diátaxis Type:** Research

REQ-09 asks for the native-optimized gf2 build measured against a competitor
with an equivalent targeting opportunity. This plan states how the family
`63bad95d-native-matvec-vs-m4ri` answers that for the dense matrix-vector
product against M4RI [AlbrechtBard2026]. The frozen rules are in
[`native-m4ri-addendum.md`](native-m4ri-addendum.md); this document gives the
reasons and the reading of each outcome.

## Question

Does public `BitMatrix::matvec` from the `native` gf2 build hold its position
against the matched M4RI product `mzd_mul(y, A, x, 0)` when M4RI is compiled
for the same host?

## The build levels of the committed comparison

The committed comparator family `2037941f-dense-matvec-vs-m4ri`
([outcome](../50f0bd42/outcome.md)) pairs two different levels:

| Side | Level in the committed family | Recorded in |
|---|---|---|
| gf2 | `conservative-portable`: default target, `simd` feature, runtime kernel dispatch, no RUSTFLAGS | the arm table of each receipt's `plan.json` |
| M4RI | host-targeted: `-O3 -march=native` for the library and the C shim | [`m4ri-build-pins.sh`](../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/m4ri-build-pins.sh), [`m4ri-probe-record.txt`](../2037941f-profile-and-optimize-mid-range-buffer-operations/m4ri-probe-record.txt), and the `gf2-m4ri-build-record.txt` and build log inside the install |

The M4RI install the committed family pins is therefore already the
native-level comparator build. The level missing from the evidence is on the
gf2 side: a `native` gf2 arm against that install. This family adds it.

Consequences:

- **No second M4RI build.** A native-level build from the pinned source with
  the pinned compiler would repeat the flags of the qualified install. The
  family links the qualified install unchanged; its source digest, compiler,
  flags and library digest are the committed pins and qualification record
  above, produced by the committed
  [`run-m4ri-matvec-probe.sh`](../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/run-m4ri-matvec-probe.sh).
  The arm verifies at run time that the object the loader gives it has the
  pinned digest.
- **The committed confirmation is not a portable-against-portable result.**
  [`calibration-plan.md`](calibration-plan.md) §3 and
  `survey/planned-families.json` describe it as the conservative level with a
  portable comparator build. The committed confirmation measures gf2 at its
  lowest level against M4RI at its highest, which is the least favourable
  pairing for gf2. The portable-level M4RI build is the comparator of the
  family [`portable-m4ri-plan.md`](portable-m4ri-plan.md) describes.

M4RI stays an external comparator: GPL code, linked only into the benchmark
arm `dense-m4ri-arm` through the existing shim, with no M4RI source in the
repository's crates.

## Arms and why the opportunity is equivalent

The addendum's arm table gives the exact flags of both sides. The gf2 level is
the `native` level of this issue's native ladder: `-C target-cpu=native` with
the baked selectors. `-C target-cpu=native` and `-march=native` each let the
compiler use every instruction-set extension of the measuring host and tune
for it, across the library under test and the code that calls it, at each
toolchain's highest standard optimisation level. The addendum records the two
differences that remain (thin LTO on the gf2 side, default-target Rust glue
around the M4RI shim) as limits.

The two families share the candidate arm, the cells' fixtures and the cost
boundaries, so a cell of this family and the cell it mirrors differ in the gf2
build level alone.

## Cells

Six cells, all `warm`, each a mirror of a committed comparator cell; the
addendum's cell table lists them with the reason for each. The choice:

- the two fresh mid-range cells the committed confirmation measured, for a
  like-for-like comparison with it;
- the two retained-state cells of the same shapes: the committed pilot's
  smallest gf2 lead is at this boundary
  ([tables](../../../bench_results/2037941f/m4ri-gap-tables.md), pilot
  section, row `m4ri-gap-65x4096-retained-warm`), so a changed position would
  show there first;
- two small-input cells, which REQ-09 names: the one-bit product and the full
  single word.

P-20 admits six comparisons on a first attempt; the addendum shows the
arithmetic and the harness contract recomputes it. The pilot measures exactly
the confirmatory cells in one cache state, so the freezer's whole-pilot
resolution is the confirmatory cells' own.

## Decision rule and margins

The equivalence margin, material-gap threshold and resolution ceiling are the
committed comparator family's, because the boundaries and the candidate arm
are the same; the addendum's margin table states them. The pilot budget is
smaller than that family's: one pilot and one replacement for a voided pilot.

## What each outcome means for REQ-09

The addendum's decision table maps each recorded decision to a reading. Per
cell:

| Reading | Meaning for REQ-09 |
|---|---|
| gf2 ahead | The native gf2 build keeps its lead over host-targeted M4RI at this boundary. With the committed confirmation, the lead holds at both gf2 levels against the same M4RI build. |
| equivalent, or no material gap | The two are comparable at this boundary at equal targeting. Recorded as the result; no production change follows from a comparator. |
| gf2 behind | A material external gap at equal targeting. Recorded as a loss with its interval; it warrants attribution (profile and assembly of the gf2 route at that shape), not selection of an M4RI route. |
| inconclusive, unstable or not confirmatory | No reading for the cell; the one-attempt rule makes it terminal, and REQ-09's native comparator leg stays open for that boundary. |
| `resolution-insufficient` after the pilot | No confirmation runs; the pilot is committed as exploratory evidence. |

The fresh and the retained-state cells are read separately. A fresh cell
charges M4RI its bit-wise packing and unpacking, so it answers the
whole-consumer question; a retained-state cell is the closest boundary to
operation throughput and is no pure `mzd_mul` measurement.

## Execution

| Step | Command | Window |
|---|---|---|
| Build and untimed validation | `run-dense-harness.sh build --m4ri --native` | none |
| Pilot plan smoke | `run-dense-harness.sh window ... --m4ri --native --smoke <record>` | none |
| Pilot | the queue line of this family in [`queue.tsv`](../bench-window/queue.tsv) | first |
| Freeze the confirmation from the committed pilot receipt with the canonical freezer | | none |
| Confirmation | the same launcher with `--confirmation` | second |

The untimed validation record is
[`dense-harness-validation.txt`](../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/dense-harness-validation.txt):
on each cell's own fixture the native gf2 build and the portable gf2 build
pass the independent parity oracle (`comparator-*` cases) and the M4RI arm
agrees with the gf2 peer (`m4ri-cell-*` cases). The pilot plan smoke is
[`survey/native-m4ri-pilot-smoke.json`](survey/native-m4ri-pilot-smoke.json).
