## Review: Build the gf2 batch decoder benchmark arms (feb60d0d)

**Verdict:** PASS

### Gate status

cargo-ci passes on `b867e6746`, code-review and research-review on `e153f56fe`,
after two reworks.

### Prior-findings regression table (Tier 1.5)

| Round | Finding | Status at HEAD | file |
|-------|---------|----------------|------|
| R1 | F1: arms manifest names dependencies by depth-dependent paths | closed: the manifest cites the named exception `harness-path-dependencies` | `dev/active/ed3d490e-zen3-ldpc-frame-simd/survey/arms/Cargo.toml` |
| R2 | F1: the build recorder decides from Git history | closed: the recorder reads no history and fails on content only | `dev/active/ed3d490e-zen3-ldpc-frame-simd/survey/record-gf2-arms-build.py` |
| R2 | F2 (advisory): selector test asserts construction literals | closed: asserts routes over the wave sizes | `dev/active/ed3d490e-zen3-ldpc-frame-simd/survey/arms/src/lib.rs` |
| R2 | F3 (advisory): restating comment | closed | same file |

### Success criteria

- [x] REQ-01 to REQ-04 — unchanged since round 1; no finding.
- [x] REQ-05 — `feb60d0d/gf2-arms-build.json` records each pinned executable
  with its record digest and the isolation of the arms build; the building
  checkout holds none of the pinned executables.

### Stale-narrative sweep (Tier 2.5)

Zero matches.

### Deferred-items audit (Tier 2.75)

The linked records carry no deferral marker.

### Holistic findings

The arm executables embed the build directory, so their digests in the build
and smoke records are those of the worktree that built them.
