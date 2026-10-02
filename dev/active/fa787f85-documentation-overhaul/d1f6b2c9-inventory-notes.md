# Field-dispatch inventory notes (d1f6b2c9)

- Rows exist for the 477 files present at `7641561a9`; `dev/active/6dc81018-field-capability-dispatch/handoff-17.md` is later and has no row.
- Evidence is `doc-ref` when an issue links the file and `issue-id` when only the entry name identifies the owner; owners are the linking issues plus the entry-named issue.
- Epic `86b9c719` also contains `220cab0b`, `389aa4de`, `3fa7c9d0` and the `6dc81018` directory documents `classification.md`, `investigation.md`, `plan.md` and `breakdown.json` through shared owners. `6dc81018` holds the larger owner count for `220cab0b/design.md` and the directory files, and the entry-named tie-break picks it for the directory.
- `389aa4de/receipt-notes.md` and `3fa7c9d0/design.md` tie between both epics with no entry-named epic; `6dc81018` is assigned because its epic dir holds every sibling field-dispatch entry. Ownership of these two rows is uncertain.
- Every non-directory entry destination is `dev/active/6dc81018-field-capability-dispatch/<entry>/<relative path>`; the `6dc81018` directory stays in place.
- Each entry is one bundle. Consumers that pin a directory or repository depth (`.cmd` command files, `ROOT=` depth computations, `OUTDIR`, cross-entry reads) sit on the bundle head; consumers of one file sit on that file.
- The campaign declaration locator is path-agnostic; the listed `a83583e0` and `dbd8787d` consumers read the located files and the declaration's `protocol` and `producing_manifest` paths.
- `digest_pinned` marks files whose SHA-256 a committed record keeps: the `50b47eae` session manifests, the `9162956b` pilot receipt, the `a83583e0` failed-attempt build records, and the campaign declarations, protocols and producing manifests of `a83583e0` and `dbd8787d`.
