# Matvec threshold calibration: premeasurement protocol

> **Diátaxis Type:** Reference

Status: declared; no campaign of this declaration has run. Issue `63bad95d`
supplies the success criteria, and
[`matvec-sweep-design.md`](matvec-sweep-design.md) the design.

This declaration amends the executed seam protocol
[`dbd8787d/premeasurement-protocol.md`](../../dbd8787d/premeasurement-protocol.md)
(cited below as *seam*) by reference, and through it the extent protocol the
seam protocol amends. Every rule of both holds unless a section here replaces
it.

## 1. Scope

The campaign adds one retained-threshold field,
`bit_matrix.matvec_simd_min_words`, to the core producer
`crates/gf2-core/benches/tuning_calibration.rs` and re-measures the complete
core section under the run-identifier prefix `gf2-63bad95d-`. Measurements
taken at different revisions are not merged, so every retained threshold,
extent and joint M4RM cell is measured afresh. The algebra owner is imported
as in seam §6, unchanged.

The core behaviour token stays `tuning-calibration-v4` and the owner protocol
`core-tuning-campaign-v4`: the case and result wire shapes are unchanged, and
the core codec reopens a calibrated section only under that token. The
behaviour-source manifest
[`producing-build-inputs.json`](producing-build-inputs.json), written by
`survey/make-producing-build-inputs.py`, carries the campaign's identity. It
is the seam manifest with this campaign's declaration in place of the seam
one and with the sources the producer's packages have gained since, each
classed in the generator.

## 2. Field, grid and arms

The field follows the retained-threshold experiment of the extent protocol:
two arms per grid point, one probe and five timed fresh children per arm, five
250 ms windows per child, and the retained crossover rule.

| Tag | Field | Conservative / asymptotic arm | Grid `s` (stride words) | Default |
|---:|---|---|---|---:|
| 31 | `bit_matrix.matvec_simd_min_words` | scalar lane / SIMD lane | `[4,7,8,9,32,63,64,65,128]` | 8 |

The field is selected at compile time, so an installed value does not steer
it. Each child installs the strict prepared section with the field forced to
`s` on both arms, as the bit-backend field does, and each arm pins its lane
through the public `BitMatrix::matvec_with_route`: `MatvecRoute::Scalar` on
the conservative arm and `MatvecRoute::Simd` on the asymptotic arm.

| Operation timed | Controls | Required observation |
|---|---|---|
| the allocated whole product of a `1024 x 64s` bit matrix and a `64s`-bit vector | none | route `scalar` or `simd`; effective observation `baked_selector_direct_lane`; capability `scalar_lane` on the conservative arm and the detected logical backend on the asymptotic arm |

A probe without the logical kernel bundle is the existing
`SimdBackendUnavailable` omission on the asymptotic arm and blocks
publication of the field.

The grid brackets the conservative default and the 64-word boundary. It holds
none of the strides
[`survey/matvec-holdout-cells.json`](survey/matvec-holdout-cells.json)
reserves for the protocol families that reconfirm the profile.

## 3. Fixtures, seeds and witness

Seed tag 31 follows the seam tags; the `fixture-seeds-v2` inventory, the
`gf2-calibration-seed-v1` mixer, eight banks, the `b + 3 (mod 8)` pairing and
the bank-major role order are unchanged.

| Roles | Construction |
|---|---|
| `matrix 0x1000`, `vector 0x1001` | per bank, a `1024 x 64s` matrix of raw draws, row-major, and a vector of `s` raw draws |

Bank `b` multiplies its matrix by the vector of bank `b + 3 (mod 8)`. The
semantic witness is the word-level row parity of every row. Operand digests
use the domains `gf2-calibration-bit-matrix-v1`,
`gf2-calibration-bit-vector-v1` and `gf2-calibration-matvec-banks-v1`; the
independent validator reconstructs them from the seeds.

## 4. Counts

The core owner declares 774 cells: 378 retained-threshold arm cells
(twenty-one sweeps of nine points, two arms), 372 extent cells and 24 joint
M4RM cells. That is 774 probes, 3,870 timed children, 4,644 accepted results,
19,350 raw windows and 23,220 timing progress records. Nominal target-window
time is 4,837.5 s. The core codec has 37 leaves; 31 are measured and 6
omitted.

## 5. Budget

The session budget stays 10,800 s with the child limits of the extent
protocol. Seam §5 estimates the active time of its cells and the time of one
retained-threshold cell; the eighteen added cells are estimated at that
per-cell time, which puts the campaign at about 150 s above the seam
estimate. This is an estimate, not a measurement. A session that exhausts
its budget is resumed under the same campaign identifier.

## 6. Declaration and destinations

[`campaign-declaration.json`](campaign-declaration.json) is the one committed
declaration of this campaign. The launcher takes the issue to start a run
(`dev/scripts/tuning-extent-campaign.sh 63bad95d`) or a run identifier to
resume one, and builds only the declared producer. Repository destinations
are derived from the run identifier as in seam §7.

## 7. Premeasurement checks

Before the first timed child: wrapped formatting, the focused suites of the
tuning steps, the validator's self-test, `git diff --check`, and untimed
`--self-check`, `--list-grid` and `--capability-report` of the Rust 1.95
release core producer. The producer's operand-check test runs the validator's
reconstruction against the producer's digests for the field.

## 8. What the campaign does not decide

The published owner states the field's swept value. The baked constant
changes only under the rule of
[`matvec-sweep-design.md`](matvec-sweep-design.md#4-relation-to-the-protocol-confirmation),
after the protocol confirmation of `4337c02e`. The campaign reads no result
of that confirmation and its grid is fixed here, so it can run before or
after it.
