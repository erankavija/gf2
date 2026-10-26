You are a worker on JIT issue 1358d6ab, "Remeasure comparator gaps on the integrated tree", in the gf2 repository. Shell zsh. Never write `.jit/`.

## Issue (verbatim)

Remeasures, on the integrated release build, the integrated LDPC and BCH comparator gaps, so the integrated LDPC and BCH comparisons are current and every other primary cell is consumed by identity with its drift class.

## Background

Comparator families measured gf2 against external projects before later production changes. The production-drift record classifies every committed receipt as measuring changed code; the invoker ruled on 2026-10-25 that consumed cells keep their evidence by identity with that drift class, and only the criteria-named comparisons are remeasured. It also ruled that the canonical BCH evidence is consumed by identity as historical context and the affected integrated BCH cells are remeasured under protocol v4, and that the external BCH programs of the `4e732b56` survey, which time themselves, are wrapped as runner arms as the AFF3CT decoder arms are. Each family runs on a fresh ledger with at most six confirmatory and holdout cells (P-20). Design: `dev/active/1a379447-zen3-cpu-performance/1362381c/design.md` (families FI-3, FI-4; Decisions D-2, D-3, D-6).

## Success Criteria

- [hard] REQ-01: Family `1362381c-ldpc-aff3ct-integration` completes pilot, canonical freeze and confirmation of the integrated production LDPC route under the committed envelope against AFF3CT's matched flooding normalized min-sum decoder [Cassagne2019], on both codes at one worker with full and one-frame waves and at twelve physical cores.
- [hard] REQ-02: The AFF3CT and bchlib BCH encoders of the `4e732b56` survey run as runner arms whose codewords equal gf2's bit for bit on every workload they serve, and family `1362381c-bch-integration` completes pilot, canonical freeze and confirmation of gf2's production BCH batch encode against them on the code classes whose encoder changed.
- [hard] REQ-03: Every primary cell outside these two families is listed with the drift class the production-drift record gives its receipt and is consumed by identity, never presented as a measurement of the current tree.
- [hard] REQ-04: Each family's receipts, execution logs and acceptance verdicts are committed with every input they pin; negative and inconclusive outcomes are reported as recorded, and every external project cited has a citation-registry key.

## Notes

Timed runs run only in the overnight benchmark window.
## Read first

- `AGENTS.md` (project contract and invariants; especially single-source-prose, semantic-test-assertions, convention-convergence, library-first-generality, behavioral-evidence-validity, non-obvious-comments, present-tense-prose, no-deferred-defects).
- `dev/active/1a379447-zen3-cpu-performance/worker-brief.md` (epic worker conventions; "Evidence and prose" and "Repository mechanics" bind you).
- `dev/active/1a379447-zen3-cpu-performance/1362381c/design.md` (the scorecard design; your package is named in the Work packages table; Decisions D-1..D-6 are ruled as recommended).
- `progress.json` standing rule of this epic: provenance or protocol questions take the cheapest compliant resolution; add no new protocol, schema, checker or ledger version unless a success criterion names it.

## Constraints

- No timed run in session (see Window workflow).
- Nothing under `dev/bench_results/**`, no receipt, `inputs/` snapshot, ledger, addendum or frozen record changes. Production crates under `crates/**` stay untouched. Never write `.jit/`.
- Reuse existing modules by import (never copy their code). Extend a shared generator at its source.
- Python tools: follow the style of sibling generators (module docstring with usage, `--self-test` or a `test_*.py` beside the script, as siblings do).
- Report blockers instead of widening scope.

## Window workflow (binding)

- Timed runs happen only in the overnight benchmark window. You prepare everything in session: arm builds (Rust 1.95, release), untimed smokes through `benchmark-ab-runner smoke` (zero timing windows), pilot addenda, fresh ledger genesis files, holdout declarations committed before the pilot, plans, and one window launcher per family modelled on `dev/active/1a379447-zen3-cpu-performance/d06a99ed/measurement/window.sh` (pilot → canonical freeze with `dev/active/1a379447-zen3-cpu-performance/c7113c5a/survey/freeze-confirmation.py` using repository-relative `--pilot`, `--family-description`, `--purpose` and `--holdout-cells` where applicable → commit only the freeze outputs → confirmation).
- Validate each launcher and addendum with the untimed confirmation smoke (`--untimed`) before handing it over; it loads the addendum through the same session path.
- Pass repository-relative paths to the freezer and every recorder; the runner rejects absolute evidence paths only at timed-session start.
- Do not edit `dev/active/1a379447-zen3-cpu-performance/bench-window/queue.tsv`; give the queue lines (format of the existing lines) in your report; the lead appends them.
- The LDPC arms build only in the story build tree `.agents/worktrees/agent-ed3d490e-run` (pinned AFF3CT digests reproduce only at that path) and the live arm build records are `dev/bench_results/e5f8b3ae/{native,portable}/build-identity.json`. A new live record is written only by `record-gf2-arms-build.py --supersedes <previous>` followed by `record-arm-smoke.py` and `record-arm-validation.py` for the same directory.
- The family's ledger admits at most six confirmatory and holdout cells on its first attempt (P-20, see the design's bound arithmetic); check with `benchmark-acceptance --next-attempt <ledger> <family>`.
- Your issue completes after the window's results are committed; this dispatch covers the in-session preparation. Stop when the launchers are validated and committed-ready, and report.

## Pointers

- FI-3: `dev/active/ed3d490e-zen3-ldpc-frame-simd/survey/make-family.py`, declarations `survey/families/aff3ct-matched.json` (prior matched family; its outcome in `dev/active/1a379447-zen3-cpu-performance/6230e0b6/findings.md`). Production arm: `gf2-batch-tuned` (committed envelope, worker-aware routing); AFF3CT arm `aff3ct-flooding-nms-f32`.
- FI-4: production BCH batch encode harness of `8e53235b` (find it under `dev/active/1a379447-zen3-cpu-performance/8e53235b/`); external programs in `dev/active/ae03bcd0-general-bch/4e732b56/baseline-survey/` (they self-time; wrap them as runner arms speaking the arm wire contract in `dev/tools/tuning-campaign-support` `arm` module, as `survey/arms/aff3ct-*` wrap AFF3CT). Code classes: those whose encoder `8e53235b` changed (m14, DVB-T2 short and normal), batch sizes of the `fd9d5416` W1 table (`dev/bench_results/fd9d5416/2026-10-02-fd9d5416-bch-performance-receipt.md`, historical context only).
- bchlib needs a citation-registry key; add it through the citation registry's canonical path (find how existing keys such as `Cassagne2019` are registered).
- REQ-03 is satisfied by WP-6's scorecard reading `dev/active/1a379447-zen3-cpu-performance/1362381c/production-drift.json`; in this dispatch, only confirm that every primary cell of `workload-set.json` resolves to a drift row (report any that do not).

## Footprint

New files under `dev/active/1a379447-zen3-cpu-performance/1358d6ab/`, the FI-3 declaration under `dev/active/ed3d490e-zen3-ldpc-frame-simd/survey/families/`, BCH arm wrappers beside the `8e53235b` harness, the citation registry entry. No production crate edits.

## Report (under 3500 characters)

Changed files; per family: cells and roles, P-20 check output, equivalence and smoke results, launcher path; queue lines; open issues. Write `path<TAB>doc_type<TAB>label` lines into `target/1358d6ab-links.tsv` in your worktree.
