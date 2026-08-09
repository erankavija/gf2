# Epic b8206228 — empirical permanent statistics

Planning and execution materials for the epic *Empirical permanent statistics
of random matrices over small prime fields*. The authoritative work graph is the
`.jit/` issue store; the files here are its record.

| File | What it is |
|---|---|
| `plan.md` | The reviewed plan: outcome, criterion approach, and the reasoning behind the breakdown's shape. Live document. |
| `investigation.md` | The epic's own analysis narrative — a read-only verification of the tree against the claims the feasibility study carried into planning. Live document, kept current with the code. |
| `execution-batching.md` | How-to guidance for dispatching the breakdown: which serial runs one worker should take. Live document. |
| `breakdown.json` | The manifest this epic's issues were created from, excluded from live-prose audits. See below. |
| `progress.json` | Execution state: waves, per-issue status, rework counts, escalations, surfaced pitfalls. Maintained by the execution lead. |

## `breakdown.json` is excluded from live-prose audits

`breakdown.json` is the manifest this epic's issues were batch-created from. It
is excluded from live-prose audits by a decision recorded in issue `240b7618`,
under the heading *Decision — the breakdown manifest is excluded*:

```bash
jit issue show 240b7618
```

The exclusion is a decision, not a property this file has to demonstrate. It
does not depend on the file being frozen, on it matching any particular
revision, or on its entries agreeing with the `.jit/` store. Nothing here
should be read as a claim to be checked.

What the file is: the input that created this epic's issues. Its entries state
the conditions that motivated those issues at breakdown time, so an entry whose
issue has since been delivered describes a defect that no longer exists. A
correction to an issue belongs in the issue, through `jit issue update`, never
here.

The authoritative text for any issue is the `.jit/` store, `jit issue show
<id>`. Read the manifest beside it, not instead of it.

This project does not yet perform manual archival, so the file stays here. Its
location under `dev/active` reflects the epic it belongs to and carries no
implication that its contents are live prose.
