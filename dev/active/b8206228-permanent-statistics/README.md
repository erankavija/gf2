# Epic b8206228 — empirical permanent statistics

Planning and execution materials for the epic *Empirical permanent statistics
of random matrices over small prime fields*. The authoritative work graph is the
`.jit/` issue store; the files here are its record.

| File | What it is |
|---|---|
| `plan.md` | The reviewed plan: outcome, criterion approach, and the reasoning behind the breakdown's shape. Live document. |
| `investigation.md` | The epic's own analysis narrative — a read-only verification of the tree against the claims the feasibility study carried into planning. Live document, kept current with the code. |
| `execution-batching.md` | How-to guidance for dispatching the breakdown: which serial runs one worker should take. Live document. |
| `breakdown.json` | The frozen pre-execution breakdown manifest. See below. |
| `progress.json` | Execution state: waves, per-issue status, rework counts, escalations, surfaced pitfalls. Maintained by the execution lead. |

## `breakdown.json` is a frozen snapshot

`breakdown.json` is the manifest this epic's issues were batch-created from. It
was applied on 2026-08-09 at `11:54:10Z`, from the manifest as it stood at
commit `89531d65`.

Its entries are verbatim copies of the issue descriptions as created, so each
one states the condition that motivated its issue at breakdown time rather than
the current state of the code. An entry whose issue has since been delivered
therefore describes a defect that no longer exists. That is what a snapshot
records; editing it to match the code would only make the copy disagree with
the issue it came from, which is the staleness defect `@/inv/single-source-prose`
names.

The live, authoritative text for any issue is the `.jit/` store:

```bash
jit issue show <id>
```

Two consequences follow.

- The file is excluded from live-prose audits, exactly as archived development
  history under [`dev/archive/`](../../archive/) is. The narrative audit
  recorded in [`crates/gf2-algebra/README.md`](../../../crates/gf2-algebra/README.md)
  excludes it by path.
- A correction to an issue's description belongs in the issue, through
  `jit issue update`. This file follows the store when it is resynchronised —
  commits `7901430a` and `95e16b81` mirrored one such pass for the protocol
  bracket back into it — and is never hand-edited into disagreeing with it.

To verify that fidelity, compare each entry against the issue of the same title:

```bash
python3 - <<'EOF'
import glob, json
manifest = json.load(open('dev/active/b8206228-permanent-statistics/breakdown.json'))
issues = {}
for path in glob.glob('.jit/issues/*.json'):
    record = json.load(open(path))
    issues[record.get('title')] = (record.get('description') or '').rstrip()
for entry in manifest:
    live = issues.get(entry['title'])
    if live is None:
        print('NO ISSUE ', entry['key'])
    elif live != entry['description'].rstrip():
        print('DIVERGED ', entry['key'])
EOF
```

It prints nothing while the manifest is faithful.
