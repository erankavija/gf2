# Development Documentation Authoring Conventions

## Overview

This guide gives the conventions that keep a development document archivable:
assets and links laid out so the document still resolves after the archive
planner relocates it.

The conventions apply to documents under this repository's development root,
`dev/`. What archival does with an artifact follows from the class of the area
holding it; the [development documentation index](index.md) lists the areas and
their classes, and covers where a new document goes. Permanent documentation
under `README.md`, rustdoc, and `docs/` lies outside the reach of a plan, so it
is retained where it is rather than relocated, even when a development document
links it.

## Link Validation

`jit doc check-links` validates documents before archival:

```bash
jit doc check-links --scope all              # every registered document
jit doc check-links --scope issue:b488f02c   # one issue's documents
jit doc check-links --json                   # machine-readable
```

Exit codes: **0** all valid, **1** errors (missing assets, broken links) — do
not archive, **2** warnings only — review first.

Each reference is checked at the version it names: a commit-pinned reference at
its commit, an unpinned one at the current version. Archival pins references, so
a pinned document's file, assets, and links keep resolving once the working tree
has moved past them.

A document's asset inventory is recorded when `jit doc add` attaches it. Repair
a link in the file and the stored inventory still describes the old text, so the
checker keeps reporting the fixed link. Re-run `jit doc add` on the same path to
re-derive it — the command is idempotent on path and updates the reference in
place.

Two limitations are worth recognizing rather than working around. A document is
scanned as markdown, so source files attached as documents can produce findings
from their own syntax — a C++ `std::chrono::nanoseconds` scans as an autolink
and then fails to resolve. Binary artifacts attached as documents, such as
figures, report a scan error because they do not decode as text. Neither blocks
a plan; `jit archive container` reports its own eligibility.

## Asset Management Patterns

### Pattern 1: Per-Document Assets (Recommended)

Store assets in a directory named after the document so they move with it:

```
<area>/
  <issue-directory>/
    encoder-timing-closure.md
    encoder-timing-closure/
      latency-histogram.png
      throughput-vs-rate.png
```

**Markdown reference:**

```markdown
![Latency histogram](encoder-timing-closure/latency-histogram.png)
```

The planner discovers the assets a document references and carries them in the
same plan, preserving their arrangement relative to the document, so the links
resolve at the destination. Preview, then execute:

```bash
jit archive document dev/active/<issue>/encoder-timing-closure.md
jit archive document dev/active/<issue>/encoder-timing-closure.md --execute
```

### Pattern 2: Shared Assets (Use Sparingly)

Only when several documents genuinely need the same asset. Link it
root-relative:

```markdown
![Reference curve](/dev/reference_data/fig_ber_prediction.csv)
```

Shared assets need coordination during archival, so reserve them for reference
curves and comparison data that outlive any one issue — which is why
`reference_data/` and `simulation_results/` are permanent areas.

### Pattern 3: External Assets

```markdown
[Ryser's formula](https://en.wikipedia.org/wiki/Computing_the_permanent)
```

External URLs are preserved and reported as warnings; they are not bundled.

## Link Safety Guidelines

### Safe

**Same directory:**

```markdown
See [the completion report](completion-report.md).
```

**Per-document assets:**

```markdown
![Crossover](gpu-crossover-decision/crossover.png)
```

**Single-level parent, to a sibling issue directory:**

```markdown
See [the design](../<sibling-issue-directory>/design.md).
```

**Root-relative, for shared assets and cross-area references:**

```markdown
See [the engineering contract](/AGENTS.md).
![Reference](/dev/reference_data/fig_ber_prediction.csv)
```

### Risky

**Deep relative traversal (two or more parent levels):**

```markdown
[Kernel](../../crates/gf2-core/examples/m4rm_multiply_perfstat.rs)
```

Relocating the document breaks the link, and the validator warns about it. Use a
root-relative path for anything outside the issue's own directory.

**Linking a file that is not itself a registered document** resolves today and
warns, because nothing schedules that file to move alongside. Attach it with
`jit doc add`, or link it root-relative if it belongs to a permanent area.

### Avoid

**Absolute filesystem paths** — `/home/user/gf2/dev/plan.md` resolves on one
machine.

**Paths escaping the repository** — `../../../other-repo/doc.md`.

**Tracker internals by path** — `.jit/issues/<id>.json`. Name the issue by its
short id in prose and attach the document to it with `jit doc add`; the
attachment carries the association, and the path does not survive a move.

## Receipts and Evidence

A performance, crossover, or scalability claim cites a committed receipt, per
[`AGENTS.md`](../AGENTS.md). Receipts belong in `bench_results/`, which is
issue-scoped, so a receipt lands in the directory of the issue whose claim it
supports and archives with that issue's container.

Keep a receipt's provenance header intact. When a re-run supersedes a receipt,
replace the file rather than leaving the study citing a name that no longer
exists, and re-run the whole set the document cites if the narrative compares
them — receipts from different harness commits do not compare cleanly.
