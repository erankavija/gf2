# 27f0f3b1 inventory notes

- Receipt directory: the topmost directory under `dev/bench_results` holding a `receipt.json` or an `inputs/` snapshot; one opaque, digest-pinned row each. Markdown elsewhere in an operational directory has its own row.
- `consumers` holds the first non-comment reference line of each consuming file.
- A Markdown file with a consumer or a content-digest pin stays in place; an unconsumed one goes to its epic's archive or active directory, or to the legacy mirror without evidence.
- `3eeb57f6` is a standalone task without an epic; it is its own `epic` and `dev/active/3eeb57f6/` its destination.
- `dev/plans/field_poly_module_overview.md` has four distinct `jit:` commit scopes, so it carries no unique provenance and goes to the legacy mirror.
- `dev/archive/packed_field_stub` is owned by `7f818151` through its archival commit; epic `6dc81018` is open, so it moves under that epic.
- `dev/archive/b7157be6-osd` lacks a container marker; its rows keep their paths and take the epic archive disposition in place.
- `dev/index.md` is `rewritten-topic`; `dev/authoring-conventions.md` stays as operational guidance.
