## Review: Build the gf2 batch decoder benchmark arms (feb60d0d)

**Verdict:** FAIL

### Gate status

research-review passed; code-review failed with one blocking finding;
cargo-ci failed in its gate run on
`gf2-coding::bch_matrix_allocation caller_buffer_materialization_reaches_no_allocator`,
a test that passed in the two runs before it on the same tree
(`@/issue/f21b25ac`). First review of the issue.

### Success criteria

- [x] REQ-01 to REQ-04 — the arms workspace under
  `dev/active/ed3d490e-zen3-ldpc-frame-simd/survey/arms/` and the records under
  `dev/active/ed3d490e-zen3-ldpc-frame-simd/feb60d0d/`; the reviews raised no
  finding against them.
- [ ] REQ-05 — `gf2-arms-build.json` records nine pinned executables with
  unchanged digests and the others as not found in the building checkout.

### Stale-narrative sweep (Tier 2.5)

Zero matches.

### Deferred-items audit (Tier 2.75)

The linked records carry no deferral marker.

### Holistic findings

code-review F1 (`@/invariant/no-dev-path-coupling`): the arms manifest
`survey/arms/Cargo.toml` names its two harness dependencies and the workspace
crates by depth-dependent relative paths.

### Required changes

1. The arms manifest declares its dependencies in a form the invariant admits;
   read the finding's full text and the record of the `harness-path-dependencies`
   exception before choosing it.
2. REQ-05 is evidenced for every pinned executable of the earlier LDPC
   campaigns, or the record states by which identity the absent ones are
   unaffected.
3. The cargo-ci gate is evaluated again.
