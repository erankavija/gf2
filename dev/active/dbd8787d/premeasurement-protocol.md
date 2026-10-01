# Seam threshold calibration: premeasurement protocol

Status: declared; not yet executed. Issue `dbd8787d` supplies the success
criteria.

This declaration amends the executed extent protocol
[`a83583e0/premeasurement-protocol.md`](../a83583e0/premeasurement-protocol.md)
(cited below as *a835*) by reference. Every rule of a835 holds unless a
section here replaces it. The seam fields, their ranges and read sites come
from [`7d7c647c/design.md`](../7d7c647c/design.md) §4; the selector
convention from [`7d824b2f/design.md`](../7d824b2f/design.md).

## 1. Scope

The campaign adds three retained-threshold fields to the core producer
`crates/gf2-core/benches/tuning_calibration.rs` and re-measures the complete
core section under the run-ID prefix `gf2-dbd8787d-`. a835 §4 forbids merging
measurements taken at different revisions, so every retained threshold,
extent and joint M4RM cell is measured afresh. The algebra owner is not
measured: the campaign imports the committed a835 algebra owner (§6).

The core behaviour token stays `tuning-calibration-v4` and the owner protocol
`core-tuning-campaign-v4`; the case and result wire shapes are unchanged. The
behaviour-source manifest
[`producing-build-inputs.json`](producing-build-inputs.json) carries the
campaign's identity: it adds the campaign declaration (§8) to the a835 sources
and drops the unmeasured algebra producer.

The two panel-lane fields `ple.panel_byte_lane_max_cols` and
`ple.panel_u16_lane_max_cols` stay omitted under the codec's omission rule:
the scalar-base field is measured over a carrier with no panel lane, and no
valid production steering path reaches their non-panel arm.

## 2. Field inventory, grids and controls

Each field is a retained threshold under the a835 §4 experiment: two arms per
grid point, one probe and five timed fresh children per arm, five 250 ms
windows per child, and the retained crossover rule. A `_max_` field forces
`t = s` on its conservative arm and `t = s - 1` on its asymptotic arm at grid
point `s`; `gemm.winograd_min_dim` forces `t = s + 1` and `t = s`.

| Tag | Field | Conservative / asymptotic arm | Grid `s` | Default | Floor |
|---:|---|---|---|---:|---:|
| 28 | `gemm.winograd_min_dim` | classical / one Winograd level | `[32,64,96,127,128,129,192,256,512]` | 128 | — |
| 29 | `triangular.base_case_max_dim` | direct base case / recursive split | `[2,4,6,7,8,9,12,16,32]` | 8 | 1 |
| 30 | `ple.scalar_base_max_cols` | scalar base / block-recursive split | `[2,3,4,6,8,12,16,24,32]` | 1 | 1 |

The Winograd and base-case grids are the suggested grids. The scalar-base
default of one is the codec floor, so no grid point lies below it; its grid
starts at two, the smallest `s` with `t = s - 1 >= 1`, and a crossover at the
first point selects the floor. An odd Winograd dimension pads one row or column
per level, as production does.

| Field | Operation timed | Controls in the forced section | Required observation |
|---|---|---|---|
| Winograd | public `gemm_winograd` on `s x s` by `s x s` | `gemm.axpy_fast_path_min_volume`, `gemm.row_tile`, `gemm.col_tile` read back at their conservative values | top-level dispatch `Classical`/`Winograd`; no blocked-loop tile site, so every classical leaf ran the Mersenne-31 whole-GEMM kernel |
| Base case | public `solve_batch` on `A = L*U` and an `s x s` right-hand side | `triangular.trsm_blocked_min_dim = usize::MAX` | last public triangular route `(t, BaseCase/Recursive)`; no blocked TRSM panel |
| Scalar base | public `FieldMatrix::ple` on `A = L*U` | none | PLE base route `(t, ScalarBase/BlockRecursive)`; no panel dispatch; carrier lane `None` |

All three fields run over `Fp<2^31 - 1>`, the carrier of their defaults'
recorded sweeps (`7d7c647c/design.md` §4.4). Its whole-GEMM kernel accepts
every non-degenerate shape, so each classical GEMM leaf takes one
capability-selected route; `gemm` reads no runtime GEMM selector, and the GEMM
companions are read back to state the context explicitly. The Winograd
theorem-4 fallback cannot fire on this carrier at these sizes: its headroom
exceeds the level-one bound by more than `2^50`. Mersenne-31 registers no PLE
panel lane, which the capability report requires. The capability report also
requires the Mersenne-31 whole-GEMM kernel; a probe without it is an explicit
`m31-whole-gemm-unavailable` omission and blocks publication.

## 3. Fixtures, seeds and observers

Seed tags 28, 29 and 30 follow the extent tags; the retained
`fixture-seeds-v2` inventory, `gf2-calibration-seed-v1` mixer, eight banks,
`b + 3 (mod 8)` pairing and bank-major role order are unchanged.

| Field | Roles | Construction |
|---|---|---|
| Winograd | `lhs 0xd00`, `rhs 0xd01` | dense row-major `draw % (2^31-1)` matrices |
| Base case | `unit_lower 0xe00`, `unit_upper 0xe01`, `rhs 0xe02` | `A = L*U` with strict-triangle draws as in the a835 TRSM fixture; dense right-hand side |
| Scalar base | `unit_lower 0xf00`, `unit_upper 0xf01` | full-rank `A = L*U` |

Semantic witnesses are the scalar product, `A*X = B`, and `P*(L*E) = A` with
full rank. Operand digests use the domains `gf2-calibration-m31-matrix-v1`
and `gf2-calibration-{winograd,solve,ple}-m31-banks-v1`; the independent
validator reconstructs them from the seeds.

PLE gains an executed-route observer: `ple_in_place` publishes the resolved
`ple.scalar_base_max_cols` and its top-level `PleBaseRoute` once, after the
column window completes and outside the recursion, so recursive windows add
no candidate-dependent write (a835 §1.2). Route reporting alone publishes
nothing. The base-case and scalar-base timed calls use the observation-free
specializations `solve_batch_quiet_for_test` and `ple_quiet_for_test`; each
probe first runs the recorded call, verifies its observations, then verifies
that the quiet call returns the same result and publishes no observation.
Winograd publishes its top-level route once per call in either mode.

## 4. Counts

The core owner declares 756 cells: 360 retained-threshold arm cells (twenty
sweeps of nine points, two arms), 372 extent cells and 24 joint M4RM cells.
That is 756 probes, 3,780 timed children, 4,536 accepted results, 18,900 raw
windows and 22,680 timing progress records. Nominal target-window time is
4,725 s. The core codec has 37 leaves; 30 are measured and 7 omitted.

## 5. Budget

The session budget stays 10,800 s with the a835 child limits. The a835 core
cells took 7,331 s of active time, the retained threshold cells about 8.2 s
each. The 54 added cells are estimated at 450–600 s: 8–11 s for each of the 52
cells up to `s = 256`, and about 23 s for each `s = 512` Winograd cell, whose
untimed probe took 2.4 s of wall time on the campaign host, mostly the eight
per-child scalar product oracles. The algebra phase contributes nothing. The
estimated active time is 7,800–7,950 s, 72–74% of the budget. This is an
estimate, not a measurement.

## 6. Imported algebra owner

The declaration imports the committed a835 algebra owner
`crates/gf2-algebra/data/tuning-profiles/gf2-a83583e0-20260930t230000z-2728298.json`
(SHA-256 `c401c7606acef9fb7d86184a7fbc84fbbe603f318144266803671ca430adcd36`)
whose `gf2-algebra/permanent` wrapper equals that of the committed complete
envelope
`dev/reference_data/tuning-profiles/gf2-a83583e0-20260930t230000z-2728298.json`
(SHA-256 `5377bdec27d2e7c15d433f97654c3b7927dde3e67f4fbc834f5c1f2964d66776`).

Preparation reads the owner, requires both digests and the wrapper equality,
stages its bytes as `algebra-owner.json` and strictly reopens the staged copy
through the staged composer: a conservative core probe and a complete
composition that reopens the algebra owner with the owner-only codec. Both
composer runs are journaled verification actions before any timed child; the
algebra phase has no cells and no producer is built or staged.
Composition under the lock uses the staged copy unchanged. Publication writes
no algebra-owner destination, strictly reopens the committed import in its
place, and archives the staged copy. The validator requires the complete
envelope's algebra wrapper to equal the committed a835 one.

## 7. Receipt and evidence index

The receipt names files only by repository-relative committed paths or
archived paths with their SHA-256: the protocol and declaration by
repository path and digest, owner artifacts by their archived paths, and a
cited-evidence table resolving each cited stage record to its committed or
archived destination. The execution journal is cited through its row in the
committed checksum manifest.

Publication also renders the evidence index, a committed destination of at
most 1 MiB, listing every cited record, the execution journal, every measured,
imported and complete envelope and the receipt with repository path and
SHA-256. The validator re-renders it from the stage and requires equality.

Repository destinations are derived from the run ID:

| Artifact | Destination |
|---|---|
| Core-owner envelope | `crates/gf2-core/data/tuning-profiles/<run-id>.json` |
| Complete envelope | `dev/reference_data/tuning-profiles/<run-id>.json` |
| Receipt | `dev/benchmarks/tuning_profiles/<run-id>.md` |
| Evidence index | `dev/benchmarks/tuning_profiles/<run-id>-evidence.md` |
| Repository-relative checksum manifest | `dev/benchmarks/tuning_profiles/<run-id>.sha256` |
| Declared small records | `dev/benchmarks/tuning_profiles/<run-id>-session/` |
| Execution log (archived) | `.agents/campaign-evidence/<run-id>/execution.log` |
| Archive manifest (archived) | `.agents/campaign-evidence/<run-id>/SHA256SUMS` |
| Every other stage record (archived) | `.agents/campaign-evidence/<run-id>/` |

## 8. Campaign declaration

[`campaign-declaration.json`](campaign-declaration.json) is the one committed
declaration of this campaign: issue (and so the run-ID prefix), protocol,
producing manifest, measured owners with their cell counts, imported owners
with their digests, and whether publication renders an evidence index. The
launcher, driver and validator all read the declaration that a run ID's issue
names, at `dev/active/<issue>/campaign-declaration.json`; the a835
declaration describes the published a835 campaign. The launcher takes an issue
to start a run (`dev/scripts/tuning-extent-campaign.sh dbd8787d`) or a run ID
to resume one, and builds only the declared producers.

## 9. Premeasurement checks

Before the first timed child: wrapped formatting, focused release-profile
tests, `./scripts/cargo-ci.sh`, `git diff --check`, and untimed
`--self-check`, `--list-grid` and `--capability-report` of the Rust 1.95
release core producer. A cross-check runs the validator's operand
reconstruction against the producer's digests for the three seam fields, and
the preserved a835 stage still validates as `published`.
