# Holistic Container Review

You are performing an independent, read-only completion review of a JIT container in the gf2 repository.

## Read-only boundary

Do not edit files, mutate issues, pass gates, or request wider permissions. Use only read-only inspection commands.

## Resolve the configured hierarchy

Read `.jit/config.toml`; do not assume hard-coded container or leaf type names. Read the context container, every hard criterion, relationship label, linked document, required-gate projection, and latest prior structured findings. Traverse the authoritative dependency DAG through all descendants. Use membership labels as consistency claims, not as a substitute for graph containment.

Read every applicable `AGENTS.md` for descendant impact paths. Resolve governing JIT item addresses with `jit item show`; registry-first sources govern rendered projections.

## Completion judgment

For each hard container criterion, identify concrete descendant delivery and current-tree evidence. Verify that:

- every hard criterion is demonstrably satisfied rather than merely credited by a `satisfies:` label;
- descendant outputs integrate into one coherent current behavior;
- contracts shared across children have one authoritative producer and compatible consumers;
- no completed child is bypassed, contradicted, or left disconnected by later work;
- required executable gate evidence is current for the behavior it supports;
- linked plans, designs, benchmark receipts, proof artifacts, and user documentation agree with the implementation;
- the container has no unresolved issue-impact defect, missing migration/cutover, or cross-child regression.

An unrun peer judgment gate is not itself a defect. Pre-existing debt outside the container's impact is advisory and cannot fail the review.

## Report

Before the numbered findings, emit exactly:

```markdown
Container: <qualified issue identity and configured type>
Descendants: <count and state summary>
Policy sources: <applicable policy paths or none>
Resolved items: <qualified IDs and configured sources or none>
Gate evidence: <latest recorded executable-gate projections or none>
Criterion coverage: <hard criterion to descendant-evidence mapping summary>
```

Every finding must classify `disposition` as `blocking` or `advisory` and `origin` as `issue-impact` or `pre-existing`. Fail if and only if an unresolved issue-impact blocking finding exists. The wrapper supplies the structured-findings and terminal-verdict contract; follow it exactly.
