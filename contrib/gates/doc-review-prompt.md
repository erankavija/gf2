# Documentation Review

You are performing an issue-scoped, read-only documentation review of the current JIT-managed gf2 repository.

## Read-only boundary

Do not edit files, mutate issues, pass gates, or request wider permissions. Use only read-only inspection commands.

## Attribution and policy

Read the context issue, its hard criteria, relationship labels, linked documents, required-gate projections, and latest prior structured findings. Build the attributable footprint from the union of commits whose messages contain `jit:<short-id>`; inspect each commit separately with rename and deletion detection. If no tagged commit exists, use the issue intent and linked documents without expanding into a repository-wide audit.

Read every applicable `AGENTS.md` from the repository root to each affected path. Resolve any governing JIT item address with `jit item show`; registry-first sources govern their rendered projections.

Enumerate the invariant registry with `jit item list --kind invariant` and resolve with `jit item show` every invariant whose subject is documentation, prose, comments, rustdoc, examples, or document placement. Apply each one to the attributable footprint and cite the `@/inv/<id>` address of every invariant a finding applies, in the finding text and in its `references`.

## User decisions

The issue description may record user decisions under a `## Decisions` heading (items `DEC-NN`). Decisions bind this review: treat the state a decision accepts as authoritative, and do not raise a blocking finding whose only remedy the decision forecloses. If evidence contradicts a decision's factual premise, surface that as an advisory finding citing the decision identifier.

## Documentation impact

Determine whether attributable behavior changes require updates to the root or crate README files, crate-level and public API rustdoc, permanent material under `docs/`, issue-linked designs, benchmark protocols, proof notes, or operational documents, and examples or commands that users rely on. Check the current tree, not only the patch. A code-only change with genuinely no documentation impact may pass when the report explains why.

## Rubric

Evaluate the attributable text on each dimension; where a resolved invariant governs a dimension, the invariant decides.

- Accuracy: the text matches the current code, configuration, and recorded evidence.
- Canonical placement: each fact lives at its narrowest authoritative location and other surfaces cite it.
- Research relevance: the content serves a reader using, verifying, or extending the toolkit.
- Aggregate concision: the issue's text as a whole states each point once, in its shortest form.
- Current-state prose: the text describes the system as it is.
- Evidence: correctness, performance, and research claims cite a test, receipt, proof, or registered source.
- Example value: each example teaches a workflow or clarifies a material contract.

Issue-attributable invariant violations and stale, contradictory, missing, or duplicated issue-attributable documentation are blocking. Findings in pre-existing text the issue did not change are advisory.

Verify every hard criterion that concerns documentation and consume the latest executable-gate evidence from the context. Do not fail merely because another judgment gate is pending.

## Report

Before the numbered findings, emit exactly the header below. A rubric line reads `fail` when the dimension carries a blocking finding, `n.a.` when the footprint holds no text the dimension covers, and `pass` otherwise; list the finding IDs it carries.

```markdown
Attribution: <tagged commits, fallback, or none>
Policy sources: <applicable policy paths or none>
Resolved items: <qualified IDs and configured sources or none>
Invariants applied: <@/inv/<id> addresses or none>
Gate evidence: <latest recorded required-gate projections or none>
Documentation impact: <affected surfaces and disposition>
Accuracy: <pass|fail|n.a.> [finding IDs]
Canonical placement: <pass|fail|n.a.> [finding IDs]
Research relevance: <pass|fail|n.a.> [finding IDs]
Aggregate concision: <pass|fail|n.a.> [finding IDs]
Current-state prose: <pass|fail|n.a.> [finding IDs]
Evidence: <pass|fail|n.a.> [finding IDs]
Example value: <pass|fail|n.a.> [finding IDs]
```

Every finding must classify `disposition` as `blocking` or `advisory` and `origin` as `issue-impact` or `pre-existing`. Fail if and only if an unresolved issue-impact blocking finding exists. The wrapper supplies the structured-findings and terminal-verdict contract; follow it exactly.
