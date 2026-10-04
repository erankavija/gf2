# 12907582 drift record: findings classified, not patched

Findings of `12907582-drift-preaudit.md` and of the `CLAUDE.md §` citation
sweep whose only location is contributor prose or a permanent page that is
rewritten. Each file carries a row in `migration/manifest.toml`.

| Finding | File | Disposition |
|---|---|---|
| Archived audit reports four code TODOs, one in `bch/core.rs`; no such marker exists | `dev/archive/legacy/crates/gf2-coding/docs/archive/QUALITY_AUDIT_REPORT.md:191,496` | `legacy-archive` |
| Cites a `CLAUDE.md` policy on dev-dependencies that AGENTS.md does not state | `dev/archive/legacy/crates/gf2-core/benches/field_matrix_fusion_results.md:132` | `legacy-archive` |
| Bare `TODO: Phase 6 - Documentation` and `TODO: simd_vs_scalar.rs` | `dev/archive/legacy/crates/gf2-core/docs/KERNEL_OPTIMIZATION.md:459,552` | `legacy-archive` |
| `TODO` status markers for module documentation and README | `dev/archive/legacy/crates/gf2-core/docs/POLAR_IMPLEMENTATION_PLAN.md:162,167` | `legacy-archive` |
| `TODO: Validate with cargo +1.80 build` against a superseded MSRV | `dev/archive/legacy/crates/gf2-core/docs/QUALITY_AUDIT_REPORT.md:214` | `legacy-archive` |
| `TODO` status legend and rows | `dev/archive/legacy/crates/gf2-core/docs/archive/PHASE11_IMPLEMENTATION_PLAN.md:56,750-753` | `legacy-archive` |
| Cites `CLAUDE.md` for crate layout and an "apex dispatch constraint" that AGENTS.md does not state | `crates/gf2-kernels-simd/README.md:7` | `rewritten-topic` |

`dev/archive/legacy/crates/gf2-coding/docs/archive/QUALITY_AUDIT_PLAN.md:176` is an audit
checklist item naming the TODO grep, not a drift claim, and has no row.
