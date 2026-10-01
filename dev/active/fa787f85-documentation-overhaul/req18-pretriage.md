# REQ-18 pre-triage (proposals only)

Read-only pre-analysis for `triage-doc-issues` (brief D-43). Snapshot 2026-10-01 at `66dc0910a`.
Class A: requirement overlaps the overhaul; preserve it in the named child, then reject as subsumed.
Class B: item-level API doc fix; stays in its epic and gets `epic:documentation-overhaul`.
Absorber keys are plan.md breakdown keys or existing overhaul issue IDs.

| ID | Epic | State | Scope | Class | Absorbing child (A) | Blocker / note |
|---|---|---|---|---|---|---|
| 2d65e37f | quality-documentation-tech-debt | ready | Fix unresolved, private-target and redundant intra-doc links workspace-wide | A | `34adff85` (rustdoc in CI gate) or f357b3dc REQ-05 sweep units | Workspace-wide, not item-level. Confirm 34adff85 denies rustdoc link warnings; if not, this is B |
| e2c649cd | quality-documentation-tech-debt | ready | Add rustdoc links to example file headers | A | `sweep-coding-tests-benches`, `entry-page-gf2-coding` | Criterion restates the title (2c668046); adds header prose against tersification |
| 9b3452e9 | quality-documentation-tech-debt | ready | Move six slow `no_run` LDPC doctests into integration tests | A | a0a29512 (example audit, doctest timing); `verify-rustdoc-examples` | Overlaps 3d34f504 REQ on non-compiled examples (`richardson_urbanke.rs:113`); dedupe at triage |
| 99c92597 | quality-documentation-tech-debt | ready | Document second Frobenius panic (exponent overflow) | B | — | — |
| 5deee377 | quality-documentation-tech-debt | ready | Feasibility backend doc falsely claims SIMD dispatch in `permanent_bipedal3` | B | — | Borderline A to 12907582 REQ-01 (implemented SIMD targets); single-item fix |
| 2c668046 | quality-documentation-tech-debt | ready | Meta: quality-epic issues carry title-restating criteria and drifted prose | A | `triage-doc-issues` itself | Moot once e2c649cd, 0056e853, 315f4de5, 35007c4c, 5f3d0ff9, be331e20 are dispositioned |
| 0056e853 | quality-documentation-tech-debt | ready | Deploy `README_NEW.md` as gf2-coding README | A | `entry-page-gf2-coding` | `crates/gf2-coding/README_NEW.md` no longer exists; learning-path design conflicts with entry-page contract |
| 315f4de5 | quality-documentation-tech-debt | ready | Add visual syndrome and decoding-trace examples | A | `tutorial-link-simulation` | D-45 caps tutorials at two; likely reject without carried requirement |
| 35007c4c | quality-documentation-tech-debt | ready | Standard headers (difficulty, reading time) on all gf2-coding examples | A | `sweep-coding-tests-benches` | Contrary to tersification contract |
| 5f3d0ff9 | quality-documentation-tech-debt | ready | Soft vs hard decoding comparison example | A | `tutorial-link-simulation` | Could alternatively stay as a coding feature task (B-like); triage decides |
| be331e20 | quality-documentation-tech-debt | ready | Split `hamming_7_4.rs` into basic and advanced | A | `sweep-coding-tests-benches` | `hamming_basic.rs` already exists (114 lines); mostly done |
| 68189a0a | quality-documentation-tech-debt | ready | README lacks gf2-sim row; decide `@/issue/<id>` reference convention | A | REQ-01 → 44c98235 (`readme-landing-page`, its REQ-02); REQ-02/03 → `3f29e945` (doc contract) + f357b3dc REQ-02/REQ-04 | `@/issue/` sites include `AGENTS.md:67`, `rref.rs:599`; preserve REQ-02 explicitly |
| 23f22f53 | tech-debt-2026-06-30 | ready | Missing cost/panic/example sections on gf2-core matrix and bit-vector APIs | B | — | Child of aabc528a |
| 3d34f504 | tech-debt-2026-06-30 | ready | gf2-coding doc sections; make four ignored examples compile | B | — | Example part overlaps a0a29512 and 9b3452e9 |
| 54278d0c | tech-debt-2026-06-30 | ready | Per-field docs for 15 `LogicalFns` pointers | B | — | Child of aabc528a |
| aabc528a | tech-debt-2026-06-30 | backlog | Container: public-API doc-completeness gaps | B | — | Depends on 23f22f53 and siblings |
| ad978596 | tech-debt-2026-06-30 | ready | `# Safety` docs on SIMD `pub unsafe fn`; remove `missing_safety_doc` allows | B | — | Includes lint config change |
| cdaf5da9 | tech-debt-2026-06-30 | ready | Document `Packed5Matrix` `PartialEq` and `Debug` | B | — | Child of aabc528a |
| 807ddab2 | zen3-cpu-performance | backlog | Trim comments/rustdoc in zen3-touched files to non-obvious-only | A | f357b3dc sweep units (`sweep-core-*`, `sweep-coding-*`, `sweep-simd-rest`, `sweep-sim-*`) + `sweep-completion-record` (its REQ-05 census) | Blocks `1362381c` (zen3 results publication); rejection releases it. Depends on `63bad95d`. Its REQ-02 grep (`Task [0-9]`, `session [0-9]`, `nee `, `Historically`) must carry into f357b3dc REQ-04 |
| 049a89af | gf2-algebra-permanent | ready | Permanent module rustdoc cites removed `dev/plans/` paths | A | `eliminate-dev-plans` ("no citation of its paths dangles"), `verify-final-links` | Epic container `ae82bd73` is archived; issue is orphaned in practice |
| 157c305c | none | ready | NR rate-match rustdoc says filler LLR +inf; code writes finite 15.0 | B | — | No epic label or parent; needs an epic before labelling |
| 835f34f0 | none | ready | `gf256()` rustdoc names AES polynomial; code builds 0x11D | B | — | No epic label or parent |
| c2663ce9 | none | ready | binary_v1 `is_systematic` doc vs delegated canonical answer | B | — | No epic label or parent; may need a code change, not only docs |

## Counts

- Class A: 12 (2d65e37f, e2c649cd, 9b3452e9, 2c668046, 0056e853, 315f4de5, 35007c4c, 5f3d0ff9, be331e20, 68189a0a, 807ddab2, 049a89af)
- Class B: 11 (99c92597, 5deee377, 23f22f53, 3d34f504, 54278d0c, aabc528a, ad978596, cdaf5da9, 157c305c, 835f34f0, c2663ce9)

## Extra search

Keyword scan (rustdoc, README, doctest, intra-doc, tautolog, examples/) of all 179 open issues outside fa787f85 found no further candidates whose main scope is documentation. Hits were code tasks or containers that mention rustdoc in passing: 6d2ab2bd, 90a88fa9, c9022d14, cdb951cc, dfd9e0c5, and containers b4b4b9ee, ae03bcd0, cffc15dd, 86b9c719.

## Open points for the triage task

- Three B items (157c305c, 835f34f0, c2663ce9) have no epic; D-43 "stays in its epic" needs a home first.
- 2d65e37f's class depends on whether 34adff85 fails on rustdoc link warnings.
- 5deee377 and 835f34f0 are factual drift; they are A only if 12907582 REQ-01 is read to cover item-level claims.
