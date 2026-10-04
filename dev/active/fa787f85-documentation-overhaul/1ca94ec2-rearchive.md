# Re-archive of epics e095a100, 806eb14e, 2928ccce and d4851c3d

Issue 1ca94ec2. Container archival of the four epics is executed in commit
`398ca9e0b`. `EA` is `dev/archive/e095a100-gfpm-arithmetic`, `HA` is
`dev/archive/806eb14e-hip-gpu-prototype`, `CA` is
`dev/archive/2928ccce-dvb-t2-awgn-campaign`, `MA` is
`dev/archive/d4851c3d-modem-framework`.

## Evidence files

| File | Content |
|---|---|
| `1ca94ec2-preexec.json` | Per epic, the `jit archive container <epic> --json` preview taken immediately before its execution, reduced to target, destination root, eligibility, blockers, action counts and per artifact source, action, destination, sha256 and deleted sources. |
| `1ca94ec2-verify.py` | Produces the report; `--reduce` produces `1ca94ec2-preexec.json` from the raw previews. |
| `1ca94ec2-verify.txt` | Report of `python3 dev/active/fa787f85-documentation-overhaul/1ca94ec2-verify.py`, exit 0, at the commit that adds it. The script takes the repository root from git, the archive roots from the live previews, the expected files and hashes from `1ca94ec2-preexec.json`, the scanned files from the commits tagged `jit:1ca94ec2`, and the manifest rows from the plan destinations and the epic identifiers of `1ca94ec2-preexec.json`. |

## Criteria

| Criterion | Evidence in `1ca94ec2-verify.txt` | The check establishes | The check does not establish |
|---|---|---|---|
| REQ-01 | `pre-execution preview` lines: four epics `eligible=True`, `blockers=0`, destination roots `EA`, `HA`, `CA`, `MA` | The preview before each execution is eligible, without plan-level or artifact-level blocker, into the existing directory. | That the reduced file equals the raw preview; the raw JSON is not committed. |
| REQ-02 | `marker` lines | Each of `EA`, `HA`, `CA`, `MA` holds a `.jit-container` naming the full identifier of its epic. | – |
| REQ-03 | `byte verification`: 23, 8, 12 and 8 destinations; 5 and 1 deleted sources; `tracker references`: 24/24, 10/10, 13/13, 9/9 | Each destination of the pre-execution plan has the sha256 the plan records, except one file that had it before a commit tagged `jit:1ca94ec2` changed one link target (section "Link target of the adopted Lean guide"); each source the plan deletes is absent; each document reference the fresh plan lists names its archive path in the live tracker. | Anything about `crates/gf2-sim/src/executor/scheduler.rs`, which the 2928ccce plan retains in place without a destination. |
| REQ-04 | Table "Citations" below | – | – |
| REQ-05 | `archive-rerun-results.md`, rows of epics e095a100, 806eb14e, 2928ccce and d4851c3d for issue 1ca94ec2; `fresh preview` lines: `eligible=True`, 0 blockers, 0 artifacts left to move, copy or delete; `link scan` lines: 0 unresolved; `manifest rows by epic`: 0 rows with status `pending` | The rerun table holds the result of a further `--execute` of each epic; a preview of each archived epic plans no publication and no deletion; no inline Markdown link with a local target is unresolved in a Markdown file of `EA`, `HA`, `CA` or `MA` or in a Markdown file the commits tagged `jit:1ca94ec2` touch; each manifest row of the four epics is complete. | Reference-style links, HTML links, anchors and links in non-Markdown files. |

## Files placed and deleted by the execution

| Epic | Moved into the archive | Adopted (bytes present) | Sources deleted |
|---|---|---|---|
| e095a100 | `EA/active/e095a100-presentation/themes/gruvbox.css`, `EA/plans/6fb4abad_breakdown.md`, `EA/plans/70972f06_audit.md`, `EA/plans/bdf95060_breakdown.md` | 2 | 5 |
| 806eb14e | – | 0 | 1 |
| 2928ccce | – | 1 | 0 |
| d4851c3d | – | 2 | 0 |

The fifth deleted source of e095a100 is `dev/research/rns_representation.md` and
the deleted source of 806eb14e is `dev/active/37e0b235/gpu-batch-ldpc-bp-plan.md`;
the pre-execution plan records for each the sha256 of its archive copy. Issues
0a7e2555 and 37e0b235 each hold two references that name the one archive file;
`jit validate` reports nothing about them.

## Blocker resolved before execution

Issue 806eb14e held two document references mapping to
`HA/plans/hip_gpu_prototype_wave.md`, which the preview reports as two
`destination-conflict` blockers. The reference to the byte-identical copy
`dev/plans/806eb14e-hip-gpu-prototype/hip_gpu_prototype_wave.md` is removed and
that copy is deleted with `git rm`; the reference of type `design` labelled
"HIP/ROCm GPU prototype wave plan" names the archive file. Both steps are part
of commit `398ca9e0b`.

The four pre-execution plans share no source, so the execution order is free.

## Link target of the adopted Lean guide

The execution adopts `EA/docs/lean4-verification-pipeline.md`. The planner
evaluates an adopted file at its mirrored source path
`dev/docs/lean4-verification-pipeline.md`. From that path the relative target
`../../../../proofs/WORKAROUNDS.md`, which the file has in commit `398ca9e0b`,
leaves the repository; the planner reports such a target as `repository-escape`
and reports `unpreservable-layout` for
`EA/plans/9509d8cc-9509d8cc/formal_verification.md`, which links the file. Line
10 carries the root-relative target `/proofs/WORKAROUNDS.md`, which the planner
resolves to `proofs/WORKAROUNDS.md`; the link scan of the verifier resolves a
target starting with `/` from the repository root.

## Citations

| File | Line | Target |
|---|---|---|
| `dev/active/6dc81018-field-capability-dispatch/investigation.md` | 515, 533 | `EA/plans/bdf95060_breakdown.md` |
| `dev/active/6dc81018-field-capability-dispatch/investigation.md` | 530 | `EA/research/rns_representation.md` |
| `dev/active/6dc81018-field-capability-dispatch/investigation.md` | 532 | `EA/plans/6fb4abad_breakdown.md`, `EA/plans/70972f06_audit.md` |
| `dev/active/ae03bcd0-general-bch/investigation.md` | 216, 371 | `EA/plans/70972f06_audit.md` |
| `dev/active/b8206228-permanent-statistics/0de41c82/investigation.md` | 400 | `HA/plans/hip_gpu_prototype_wave.md` |
| `036615b0-inventory-notes.md` | 5 | `HA/active/37e0b235/gpu-batch-ldpc-bp-plan.md`, `EA/active/e095a100-presentation/themes/gruvbox.css` |

`investigation.md` of this directory holds no citation of the theme stylesheet
or the GPU LDPC plan at a `dev/active` path: the list entry and the sentence
that placed them there are removed.

`git grep -E` for the seven source paths finds no citation in a README,
`AGENTS.md`, `docs/`, `crates/` or `scripts/` outside receipt `inputs/` trees.

Citations that keep their bytes:

| Location | Reason |
|---|---|
| `67048b47-linked-pairs.txt`, seven lines | Committed output of `67048b47-doc-links.py`. |
| `migration/manifest.toml`, `path` and `inbound` fields | `path` names the source by schema; `inbound` is inventory evidence. |
| `1ca94ec2-preexec.json`, `source` and `deletes` fields | Pre-execution plan. |
| `EA/active/e095a100-handoff-2.md:15`, `dev/archive/legacy/crates/gf2-core/docs/COMPUTE_BACKEND_DESIGN.md:144`, `dev/archive/6dc81018-field-capability-dispatch/active/6dc81018-field-capability-dispatch/investigation.md:515-533` | Historical artifacts under `dev/archive/`; code-span paths, no link. |
| `crates/gf2-core/src/field/poly.rs` copies in receipt `inputs/` trees | Receipt snapshot copies. |

## Manifest

| Rows | Disposition | Status | Evidence |
|---|---|---|---|
| The four files moved into `EA` | `jit-container-archive` | complete | source absent, destination carries the plan sha256 |
| `dev/research/rns_representation.md`, `dev/active/37e0b235/gpu-batch-ldpc-bp-plan.md`, `dev/plans/806eb14e-hip-gpu-prototype/hip_gpu_prototype_wave.md` | `jit-container-archive` | complete | source absent, archive copy carries the plan sha256 |

The report section `manifest rows by epic` counts 7, 4, 4 and 10 rows for
e095a100, 806eb14e, 2928ccce and d4851c3d, none with status `pending`.

## Verification

| Command | Result |
|---|---|
| `python3 dev/active/fa787f85-documentation-overhaul/1ca94ec2-verify.py` | exit 0 |
| `python3 contrib/gates/docs-mechanical.py` | PASS |
| `python3 dev/active/fa787f85-documentation-overhaul/migration/check.py` | no finding on the seven rows of this unit |
