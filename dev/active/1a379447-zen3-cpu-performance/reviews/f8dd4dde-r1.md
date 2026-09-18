## Review: Route residual BitVec shifts through a BMI2-gated funnel kernel (f8dd4dde)

**Verdict:** PASS

### Gate status
cargo-ci, asm-artefact-present, code-review and doc-review pass on the merged
rework; code-review and doc-review report zero findings.

### Prior-findings regression table (Tier 1.5)
| Round | Finding | Status at HEAD | file:line |
|-------|---------|----------------|-----------|
| R1 | REQ-06 lacks evidence that the kernels and shared suite were built and run on Rust 1.95 | closed | `dev/active/f8dd4dde/validate-msrv-record.json`, generator `dev/active/f8dd4dde/validate-msrv.sh` |

### Success criteria
- [x] REQ-01: kernels in `crates/gf2-kernels-simd/src/x86/shift_funnel.rs` with `# Safety` sections; unsafe blocks only in `crates/gf2-kernels-simd/src/shift_funnel.rs`; `crates/gf2-core/src/lib.rs` keeps `#![deny(unsafe_code)]`.
- [x] REQ-02: both shift methods hand the residual branch to `crates/gf2-core/src/residual_shift.rs` behind the `simd` feature; the route has its own bundle gated by its own `bmi2` detection.
- [x] REQ-03: `crates/gf2-core/tests/residual_shift_routes.rs` drives the scalar fallback as a route and checks the gate verdict against the host's detection.
- [x] REQ-04: the shared corpus (lengths and offsets 0, 1, 7, 8, 63, 64, 65, at and beyond the length, incomplete final word) runs over both routes against a bit-addressed zero-fill reference with zero tail padding asserted.
- [x] REQ-05: one selection point, a lane witness and a force switch under `test` or `test-support`; both toggle sections hold one mutex.
- [x] REQ-06: the Rust 1.95 record above; `crates/gf2-kernels-simd/src/x86/asm/shift_funnel.asm.txt` shows both directions; the artefact's emitting toolchain follows the crate's convention and `dev/active/f8dd4dde/retention-rule.md` says so.
- [x] REQ-07: rustdoc on the dispatched path names the `simd` feature and the `bmi2` capability; safety conditions at each boundary.
- [x] REQ-08: `dev/active/f8dd4dde/retention-rule.md`; no timed run and no speed statement.

### Stale-narrative sweep (Tier 2.5)
Zero matches for statements that the residual branch is scalar-only.

### Deferred-items audit (Tier 2.75)
`retention-rule.md`: zero matches.

### Holistic findings
- `crates/gf2-core/benches/shifts.rs` labels its residual-production arm `bitvec-residual-scalar-*`; with `simd` on a BMI2 host that arm runs the kernel route. The arms belong to 00dd43c3, which derives the label from `residual_shift_route()`; changing it here would alter the measurement-output semantics behind 85fc5ff4's committed receipt.
- Both 2037941f producing-input closures were regenerated for the new kernel sources at merge.
