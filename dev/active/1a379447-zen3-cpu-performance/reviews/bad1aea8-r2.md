## Review: Provide the AFF3CT benchmark arms of the batch decoder comparison (bad1aea8)

**Verdict:** PASS

### Gate status

cargo-ci, code-review and research-review pass on `ad461cb37`, after one
rework. research-review keeps one low advisory.

### Prior-findings regression table (Tier 1.5)

| Round | Finding | Status at HEAD | file |
|-------|---------|----------------|------|
| R1 | code-review F1: the safe decode methods pass undersized buffers to the AFF3CT shim | closed: the handle keeps the code dimensions and both methods check buffer lengths before the call; rejection tests | `dev/active/ed3d490e-zen3-ldpc-frame-simd/survey/arms/aff3ct-intra/src/lib.rs` (`checked_frames`) |
| R1 | research-review F2 (advisory): the smoke record pins a temporary plan, addendum and quality record | open, advisory; recorded in `progress.json` `surfaced_pitfalls` | `dev/active/ed3d490e-zen3-ldpc-frame-simd/bad1aea8/smoke-aff3ct-intra.json` |

### Success criteria

- [x] REQ-01 — `survey/arms.json` names seven arms; the three pinned AFF3CT
  modes are one executable named by the digest the `3be770d5` build record
  pins. The checkout holds no pinned executable; the catalogue says so.
- [x] REQ-02 — `survey/arms/aff3ct-intra/` builds `aff3ct-intra-arm`;
  `bad1aea8/smoke-aff3ct-intra.json` records the shared runner's smoke.
- [x] REQ-03 — `bad1aea8/aff3ct-arm-diagnostics.json` records one refusal of a
  differing quality record and one of a missing record.
- [x] REQ-04 — the INTRA arm writes decoder class, backend and wave size at run
  time; `bad1aea8/source-evidence.json` shows the pinned arms do.

### Stale-narrative sweep (Tier 2.5)

Zero matches.

### Deferred-items audit (Tier 2.75)

The linked records carry no deferral marker.

### Holistic findings

The INTRA crate holds its own AFF3CT handle beside the digest-pinned
`3be770d5` one. The arm executables' digests differ between checkouts with
identical sources. Both are recorded in `progress.json` `surfaced_pitfalls`.
