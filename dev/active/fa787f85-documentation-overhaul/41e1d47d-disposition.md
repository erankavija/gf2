# 41e1d47d inherited documentation issue dispositions

Candidates: every open (backlog, ready, in progress) issue outside the `fa787f85`
subtree whose main scope is documentation, derived from `jit query all --json`
and `jit graph tree fa787f85` at `2829f5daa` (2026-10-02). Classes follow brief
D-43: **A** preserves the requirement in an overhaul item and rejects the issue
with `resolution:obsolete`; **B** keeps the issue in its epic and edges and adds
the `epic:documentation-overhaul` label. `fca578b1` REQ-05, `ffc35b8c` REQ-28 and
its sweep unit `50201002` carry requirements no overhaul item held before; the
owner approved them on 2026-10-02.

| ID | Epic | Disposition | Absorbing item and criterion, or label | Reason |
|---|---|---|---|---|
| 2d65e37f | quality-documentation-tech-debt | B | label | Item-level link repairs. The CI doc step denies warnings only for its fixed feature set; `cargo +1.95.0 doc --workspace --all-features --no-deps` still reports unresolved, private-target and redundant links. |
| e2c649cd | quality-documentation-tech-debt | A, no requirement | `13dfe1a5` REQ-01 | Example headers are not built by `cargo doc`, so Rustdoc links there neither render nor get checked; the comment contract governs those headers. |
| 9b3452e9 | quality-documentation-tech-debt | A | `a0a29512` REQ-01, REQ-02; `2596b143` REQ-04 | Each slow example is removed under the example policy or retained compiling in the Rust CI doctest step. |
| 99c92597 | quality-documentation-tech-debt | B | label | Single `# Panics` omission on `FiniteFieldExt::frobenius`. |
| 5deee377 | quality-documentation-tech-debt | B | label | Single false claim in a `dev/research` harness module doc, outside the sweep's `crates/` scope. |
| 0056e853 | quality-documentation-tech-debt | A, no requirement | `698fa793` REQ-02, REQ-03 | `README_NEW.md` is absent, and a learning-path README with example lists contradicts the entry-page contract. |
| 315f4de5 | quality-documentation-tech-debt | A, no requirement | `cdba4e71` (D-45) | Elementary teaching examples contradict D-06 and D-07; D-45 fixes the tutorial set. |
| 35007c4c | quality-documentation-tech-debt | A, no requirement | `13dfe1a5` REQ-01 | Difficulty, prerequisite and reading-time headers restate nothing the code needs and contradict the comment contract. |
| 5f3d0ff9 | quality-documentation-tech-debt | A, no requirement | `cdba4e71` (D-45) | An introductory soft- versus hard-decision example contradicts D-06; D-45 fixes the tutorial set. |
| be331e20 | quality-documentation-tech-debt | A, no requirement | `13dfe1a5` REQ-01 | `hamming_basic.rs` exists; elementary Hamming material is outside the D-06 audience. |
| 68189a0a | quality-documentation-tech-debt | A | `fdb998ec` REQ-02; `fca578b1` REQ-05 | `fdb998ec` REQ-02 requires `gf2-sim` in the capability map. The `@/issue/<short-id>` form resolves in the docs-mechanical check, every current site uses it, and `fca578b1` REQ-05 records it as the convention. |
| 2c668046 | quality-documentation-tech-debt | Unchanged | none | Tracker-hygiene scope; its predicate also selects code tasks `1b929ce5` and `3931ac6f`. |
| 23f22f53 | tech-debt-2026-06-30 | B | label | Item-level doc sections on gf2-core APIs. |
| 3d34f504 | tech-debt-2026-06-30 | B | label | Item-level doc sections and four uncompiled examples in gf2-coding. |
| 54278d0c | tech-debt-2026-06-30 | B | label | Per-field docs on `LogicalFns`. |
| aabc528a | tech-debt-2026-06-30 | B | label | Container of the four item-level fixes in this table; its accessor-shaped findings live in `153297cf`. |
| ad978596 | tech-debt-2026-06-30 | B | label | `# Safety` sections and clippy enforcement in gf2-kernels-simd. |
| cdaf5da9 | tech-debt-2026-06-30 | B | label | Item-level docs on `Packed5Matrix` trait impls. |
| 807ddab2 | zen3-cpu-performance | A | `ffc35b8c` REQ-21 to REQ-23, REQ-25 to REQ-27; REQ-28 with sweep unit `50201002` | The sweep covers every `crates/` file and the history-narration grep in substance (REQ-22); REQ-28 and `50201002` cover `dev/tools/`. Dependent `1362381c` carries no comment obligation; its other open blocker is `ed3d490e`. |
| 049a89af | gf2-algebra-permanent | A | `ec655592` REQ-03, REQ-04 | Every `dev/plans/` citation in Rustdoc points to the plan's archived path or is removed. No issue depends on it. |
| 157c305c | none | B | label | Single public-doc fact on the NR filler LLR. The issue has no epic and keeps its edges. |
| 835f34f0 | none | B | label | `gf256()` modulus doc and its test. The issue has no epic and keeps its edges. |
| c2663ce9 | none | B | label | `binary_v1::is_systematic` doc against adapter behavior. The issue has no epic and keeps its edges. |

The only dependent of each quality-epic A item is its container `86b9c719`,
which carries no obligation beyond its children.
