# Matvec threshold sweep in the offline tuning producer (jit:63bad95d)

> **Diátaxis Type:** Research

Design of the retained-threshold sweep of
`bit_matrix.matvec_simd_min_words` that decision 2 of the
[calibration plan](calibration-plan.md#8-decisions) adds to the core producer
`crates/gf2-core/benches/tuning_calibration.rs`. Claim identifiers in
backticks resolve in
[`survey/source-evidence.json`](survey/source-evidence.json). This document
changes no source.

## 1. One entry for a caller-chosen lane

The scalar lane has no public entry (`matvec-scalar-lane-private`), and both
the producer arms and the cells of `4337c02e` need the two lanes of one
executable at one stride. One entry serves both:

```rust
impl BitMatrix {
    /// [`Self::matvec`] on a caller-chosen lane.
    pub fn matvec_with_route(&self, x: &BitVec, route: MatvecRoute) -> BitVec;
}
```

- `BitMatrix::matvec` becomes
  `self.matvec_with_route(x, matvec_route(self.stride_words))`, so the
  production call and a pinned lane run one body, as `BitMatrix::transpose`
  and `transpose_with_block_kernel` do.
- `MatvecRoute::Simd` runs the scalar lane when the `simd` feature is off or
  the logical bundle is not detected, which is what `matvec` does today
  (`matvec-bundle-check`). A caller that needs the kernel lane checks
  `gf2_core::kernels::simd::maybe_simd()` first, as the producer's bit-backend
  arm does (`calibration-bit-arm`).
- The method is public in every build and adds no process-global switch, so a
  timed arm measures the production code path.
- Shared-suite coverage precedes the method: `tests/simd_equiv_matvec.rs`
  compares both routes with its bit-level reference at its boundary shapes.

`4337c02e` uses this entry for its two arms and adds none of its own.

## 2. The producer field

| Aspect | Value |
|---|---|
| Field | `MatvecSimdMinWords`, family `bit_matrix`, schema field `matvec_simd_min_words` |
| Seed tag | 31, the next free tag after the seam fields |
| Conservative arm | `MatvecRoute::Scalar` through `matvec_with_route` |
| Asymptotic arm | `MatvecRoute::Simd`; an undetected bundle is the existing `SimdBackendUnavailable` omission |
| Grid unit | stride words |
| Grid | 4, 7, 8, 9, 32, 63, 64, 65, 128 |
| Operation timed | the allocated whole product of a 1024-row matrix and one vector |
| Fixture | eight banks of one matrix and one vector, drawn from the field's seed streams |
| Semantic witness | the product digest equals the digest of a bit-level reference product |
| Effective observation | `baked_selector_direct_lane`, by analogy with the bit-backend field |
| Decision | the producer's retained crossover rule, unchanged |

The grid brackets the conservative threshold and the 64-word boundary and
holds none of the strides
[`survey/matvec-holdout-cells.json`](survey/matvec-holdout-cells.json)
reserves, so the sweep takes no sample at a held-out stride.

The field is selected at compile time, so no child steers it by installing a
profile: each arm calls its lane directly.

## 3. What changes around the field

- **Producer counts.** One more measured field and one fewer omitted field in
  the inventory the producer checks (`calibration-inventory-closed`), and one
  more nine-point two-arm sweep in the declared cell, probe, child and window
  counts.
- **Behavior token.** `tuning-calibration-v4` stays. The seam campaign added
  three fields under the same token because case and result wire shapes did
  not change, and they do not change here; the core codec accepts exactly this
  token, so every committed measured owner keeps reopening. The campaign's
  identity is its producing manifest, as for the seam campaign.
- **Validator.** `dev/scripts/validate-tuning-extent-campaign.py` gains the
  field's operand reconstruction, its grid and the new counts
  (`calibration-validator-behavior`).
- **Protocol.** A premeasurement protocol under this issue amends the seam
  protocol by reference: scope, the field's row, fixture and seed roles,
  counts, budget, and the imported algebra owner.
- **Declaration.** A `campaign-declaration.json` naming `63bad95d`, with its
  producing manifest (`calibration-launcher-declaration`).
- **Baked constant.** After publication, `tuning::baked::MATVEC_SIMD_MIN_WORDS`
  mirrors the measured owner, the unit test that holds it at the conservative
  value (`baked-matvec-conservative-test`) becomes a measured-owner match, and
  the baked route witness follows. The conservative constant does not change.

## 4. Relation to the protocol confirmation

The sweep and the confirmation of `4337c02e` are separate measurements of one
boundary. The baked threshold changes only when the two agree: every stride
the confirmation records as improved for the scalar lane lies below the
owner's threshold, and no stride it records for the SIMD lane lies below it.
A disagreement keeps the conservative value and both records are committed.

## 5. Order of work

1. `matvec_with_route` with its shared-suite tests.
2. Producer field, validator, protocol amendment, declaration and manifest;
   untimed `--self-check`, `--list-grid` and `--capability-report`.
3. One queue line for the full core campaign.
4. Baked constant and witnesses, after the campaign and the confirmation of
   `4337c02e`.
