# Field-dispatch inventory notes (d1f6b2c9)

- Rows exist for the 477 files present at `7641561a9`; `dev/active/6dc81018-field-capability-dispatch/handoff-17.md` is later and has no row.
- Evidence is `doc-ref` when an issue links the file and `issue-id` when only the entry name identifies the owner; owners are the linking issues plus the entry-named issue.
- Rows whose owners sit under both top-level epics `6dc81018` and `86b9c719` list both epics in `owners`: `220cab0b/design.md` (`f35daec0`), `389aa4de/receipt-notes.md`, `3fa7c9d0/design.md`, and `classification.md`, `investigation.md`, `plan.md` and `breakdown.json` in the `6dc81018` directory (`265997f9`, `663965f6`). `active-layout` assigns them to `6dc81018`: most linked owners for `220cab0b/design.md` and `plan.md`, the entry name for the directory files.
- `3fa7c9d0` carries the single label `epic:field-capability-dispatch`, so the `active-layout` membership-label rule sends `3fa7c9d0/design.md` to `6dc81018`; `389aa4de` carries both epic labels, and the owner decision in the plan decision table `Active-layout ties` sends `389aa4de/receipt-notes.md` to `6dc81018`.
- Every entry except the `6dc81018` directory has destination `dev/active/6dc81018-field-capability-dispatch/<entry>/<relative path>`; the directory stays in place.
- Each entry is one bundle. Consumers that pin a directory or repository depth (`.cmd` command files, `ROOT=` depth computations, `OUTDIR`, cross-entry reads) sit on the bundle head; consumers of one file sit on that file.
- The campaign declaration locator is path-agnostic; the listed `a83583e0` and `dbd8787d` consumers read the located files and the declaration's `protocol` and `producing_manifest` paths.
- `digest_pinned` marks files whose SHA-256 a committed record keeps: the `50b47eae` session manifests, the `9162956b` pilot receipt, the `a83583e0` failed-attempt build records, and the campaign declarations, protocols and producing manifests of `a83583e0` and `dbd8787d`.
