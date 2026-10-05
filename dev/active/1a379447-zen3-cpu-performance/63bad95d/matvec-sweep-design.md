# Matvec threshold sweep in the offline tuning producer (jit:63bad95d)

> **Diátaxis Type:** Research

Design of the retained-threshold sweep of
`bit_matrix.matvec_simd_min_words` that decision 2 of the
[calibration plan](calibration-plan.md#8-decisions) adds to the core producer
`crates/gf2-core/benches/tuning_calibration.rs`. Claim identifiers in
backticks resolve in
[`survey/source-evidence.json`](survey/source-evidence.json). The lane entry,
the producer field and the validator's tables are in the tree;
[`premeasurement-protocol.md`](premeasurement-protocol.md) is the campaign's
protocol.

## 1. One entry for a caller-chosen lane

The producer arms and the cells of `4337c02e` need the two lanes of one
executable at one stride. One entry serves both (`matvec-lane-entry`):

```rust
impl BitMatrix {
    /// [`Self::matvec`] on a caller-chosen lane.
    pub fn matvec_with_route(&self, x: &BitVec, route: MatvecRoute) -> BitVec;
}
```

- `BitMatrix::matvec` and the entry share the scalar lane and one
  always-inlined SIMD-lane helper. `matvec` keeps its own two-arm selection,
  so its instruction text, and that of both lanes, is the same before and
  after in the default and the `simd` build:
  [`survey/matvec-asm-comparison.json`](survey/matvec-asm-comparison.json),
  written by `survey/make-matvec-asm-comparison.py` from the listings under
  `survey/asm/`. Without `simd` the entry delegates to `matvec`, where both
  routes are the scalar lane.
- `MatvecRoute::Simd` runs the scalar lane when the `simd` feature is off,
  when the logical bundle is not detected (`matvec-bundle-check`), and for a
  matrix without columns. A caller that needs the kernel lane checks
  `gf2_core::kernels::simd::maybe_simd()` first, as the producer's bit-backend
  arm does (`calibration-bit-arm`).
- The method is public in every build and adds no process-global switch, so a
  timed arm measures the production code path.
- `tests/simd_equiv_matvec.rs` compares both routes with its bit-level
  reference at the 0, 1, 63, 64 and 65-word strides and the word boundaries
  (`matvec-lane-entry-test`); the logs of the build with and without `simd`
  are `survey/test-logs/matvec-route-simd-build.txt` and
  `survey/test-logs/matvec-route-scalar-build.txt`.

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
| Semantic witness | every output bit equals the word-level parity of its row against the vector |
| Effective observation | `baked_selector_direct_lane`, by analogy with the bit-backend field |
| Decision | the producer's retained crossover rule, unchanged |

The grid brackets the conservative threshold and the 64-word boundary and
holds none of the strides
[`survey/matvec-holdout-cells.json`](survey/matvec-holdout-cells.json)
reserves, so the sweep takes no sample at a held-out stride.

The field is selected at compile time, so no child steers it by installing a
profile: each arm calls its lane directly (`calibration-matvec-field`,
`calibration-matvec-arms`).

## 3. What changes around the field

- **Producer counts.** One more measured field and one fewer omitted field in
  the inventory the producer checks (`calibration-inventory-closed`), and one
  more nine-point two-arm sweep in the declared cell, probe, child and window
  counts; protocol §4 states them, and the producer's untimed `--self-check`
  reports the same counts.
- **Producer identity.** [`survey/producer-identity.json`](survey/producer-identity.json),
  written by `survey/record-producer-identity.py`, records the producer's
  source and executable digests before and after the sweep (both differ), the
  unchanged behaviour token, the measured owner the baked constants pin with
  its digest, and every committed core owner by digest. No committed owner,
  codec source or baked constant changes, so the committed-profile tests
  reopen the same bytes.
- **Behavior token.** `tuning-calibration-v4` stays. The core codec names one
  token (`codec-token-value`) and reopens a calibrated section only under it
  (`codec-accepts-one-token`), so a new token would reject every committed
  measured owner. The seam campaign added three swept fields under the
  unchanged token because case and result wire shapes did not change
  (`seam-token-unchanged`), and they do not change here. The campaign's
  identity is its producing manifest (`seam-identity-in-manifest`).
- **Telling the owners apart.** An owner of this campaign differs from an
  owner that omits the field in three recorded places: its selector body
  states `bit_matrix.matvec_simd_min_words`, so the field moves from the
  omitted to the measured side of the inventory partition; its measurement
  block carries the digest of the producer executable that holds the sweep;
  and its profile identifier is this campaign's run identifier, whose receipt
  pins the campaign's producing manifest.
- **Validator.** `dev/scripts/validate-tuning-extent-campaign.py` carries the
  field's grid, seed tag, roles, route contract and operand reconstruction
  (`validator-matvec-grid`, `calibration-validator-behavior`); the producer's
  operand-check test compares the reconstruction with the producer's digests.
- **Protocol.** [`premeasurement-protocol.md`](premeasurement-protocol.md)
  amends the seam protocol by reference: scope, the field's row, fixture and
  seed roles, counts, budget, and the imported algebra owner.
- **Declaration.** [`campaign-declaration.json`](campaign-declaration.json)
  names `63bad95d`; `survey/make-producing-build-inputs.py` writes its
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

1. One queue line runs the full core campaign; it reads no result of
   `4337c02e` and can run before or after that confirmation.
2. The baked constant and its witnesses follow the campaign and the
   confirmation of `4337c02e`, under the rule of §4.
