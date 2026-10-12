## Review: Make caller-trusted safe kernel entry points checked or unchecked by name (bfc2ceab)

**Verdict:** PASS

### Gate status

cargo-ci and code-review passed on the merged tree of rework 3.

### Prior-findings regression table (Tier 1.5)

Rounds 1 to 3: every finding closed at HEAD; the worker's resolution table of
23 rows covers the reviewer findings and the audit rows, and code-review round 4
raised no finding.

### Success criteria

- [x] REQ-01: `bfc2ceab/inventory.json`, regenerated on the merged tree.
- [x] REQ-02: `crates/gf2-kernels-simd/src/entry_check.rs`.
- [x] REQ-03: `crates/gf2-kernels-simd/tests/entry_conditions.rs`.
- [x] REQ-04: `bfc2ceab/decisions.json` with the four committed receipts; the
  invoker accepted the exploratory cost.

### Deferred-items audit (Tier 2.75)

`confirmation-freeze-refusal-spmm-check.json` keeps its bytes and pins the
launcher digest of the pilot snapshot; `bfc2ceab/verification.md` states it.

### Holistic findings

The rework added `benchmark-acceptance --bootstrap` to the shared support
crate; the second-pilot sizing record reproduces byte for byte.
