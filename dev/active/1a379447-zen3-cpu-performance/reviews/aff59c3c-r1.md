## Review: Produce decode selector measurements from a coding owner harness (aff59c3c)

**Verdict:** FAIL

### Gate status

cargo-ci and doc-review passed on the merged tree; code-review failed with one
blocking finding. First review of the issue.

### Success criteria

- [x] REQ-01 to REQ-06 — `crates/gf2-coding/benches/tuning_calibration.rs` and
  `crates/gf2-coding/tests/tuning_calibration_harness.rs`; code-review raised
  no finding against a criterion. The bench checks at Rust 1.95.

### Stale-narrative sweep (Tier 2.5)

The story plan states that the coding section has no producer; it is the
planning record of the story.

### Deferred-items audit (Tier 2.75)

The issue links no design document.

### Holistic findings

code-review F1 (`@/invariant/no-dev-path-coupling`): `crates/gf2-coding/Cargo.toml`
declares `tuning-campaign-support` by the depth-dependent path
`../../dev/tools/tuning-campaign-support`. The named exception
`harness-path-dependencies` covers survey harness manifests under `dev/active/`
only. `crates/gf2-core/Cargo.toml` and `crates/gf2-algebra/Cargo.toml` declare
the same dependency by the same path.

The provenance and emission glue of the producer mirrors the `gf2-algebra`
producer; code-review raised no finding on it.

### Required changes

1. The dependency is declared without a depth-dependent `dev/` path. The form
   is one for the three owner crates and is the invoker's decision, recorded
   as an open escalation in the progress file.
