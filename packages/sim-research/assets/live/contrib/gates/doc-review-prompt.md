# Documentation Review

You are performing an issue-scoped, read-only documentation-impact review of the current JIT-managed gf2 repository.

## Read-only boundary

Do not edit files, mutate issues, pass gates, or request wider permissions. Use only read-only inspection commands.

## Attribution and policy

Read the context issue, its hard criteria, relationship labels, linked documents, required-gate projections, and latest prior structured findings. Build the attributable footprint from the union of commits whose messages contain `jit:<short-id>`; inspect each commit separately with rename and deletion detection. If no tagged commit exists, use the issue intent and linked documents without expanding into a repository-wide audit.

Read every applicable `AGENTS.md` from the repository root to each affected path. Resolve any governing JIT item address with `jit item show`; registry-first sources govern their rendered projections.

## User decisions

The issue description may record user decisions under a `## Decisions` heading (items `DEC-NN`). Decisions bind this review: treat the state a decision accepts as authoritative, and do not raise a blocking finding whose only remedy the decision forecloses. If evidence contradicts a decision's factual premise, surface that as an advisory finding citing the decision identifier.

## Documentation impact

Determine whether attributable behavior changes require updates to any of:

- the root or crate README files;
- crate-level and public API rustdoc;
- permanent material under `docs/`;
- issue-linked designs, benchmark protocols, proof notes, or operational documents;
- examples or commands that users rely on.

Check the current tree, not only the patch. Documentation is sufficient when it explains the observable contract at the narrowest authoritative location and does not duplicate another source of truth. A code-only change with genuinely no documentation impact may pass when the report explains why. Stale, contradictory, missing, or duplicated issue-attributable documentation is blocking. Useful unrelated pre-existing debt is advisory.

Verify every hard criterion that concerns documentation and consume the latest executable-gate evidence from the context. Do not fail merely because another judgment gate is pending.

## Report

Before the numbered findings, emit exactly:

```markdown
Attribution: <tagged commits, fallback, or none>
Policy sources: <applicable policy paths or none>
Resolved items: <qualified IDs and configured sources or none>
Gate evidence: <latest recorded required-gate projections or none>
Documentation impact: <affected surfaces and disposition>
```

Every finding must classify `disposition` as `blocking` or `advisory` and `origin` as `issue-impact` or `pre-existing`. Fail if and only if an unresolved issue-impact blocking finding exists. The wrapper supplies the structured-findings and terminal-verdict contract; follow it exactly.
