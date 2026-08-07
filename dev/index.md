# gf2 Development Documentation

Development documentation for gf2, written for contributors working on the
library itself.

## Documentation Domains

**Permanent documentation** — `README.md`, crate-level rustdoc, and `docs/`.
What gf2 is and how to use it.

**Development documentation** — this directory (`dev/`). How gf2 is built: the
designs, plans, studies, and benchmark receipts that working issues produce.

[`AGENTS.md`](../AGENTS.md) is the engineering contract both domains answer to.

---

## Areas Inside the Development Root

`dev/` is this repository's development root. Which areas it holds, which of
them organize their artifacts one directory per issue, and what archival does to
each are declared in the `[documentation]` table of
[`.jit/config.toml`](../.jit/config.toml) — this project's own policy rather
than a shipped default. `jit config get documentation` prints the table in
force.

An area is **managed** or **permanent**. A managed area's artifacts are
relocated into the archive when the container that owns them is archived; a
permanent area's artifacts stay where they are. An **issue-scoped** area
additionally gives each issue its own directory.

| Area | Class | Issue-scoped | Holds |
| --- | --- | --- | --- |
| `active/` | managed | yes | Working artifacts of open issues: designs, handoffs, progress files, completion reports |
| `bench_results/` | managed | yes | Benchmark receipts and the scorecards that read them |
| `plans/` | managed | yes | Plans and breakdowns |
| `presentations/` | managed | yes | Slide decks |
| `studies/` | managed | yes | Feasibility studies and their measurement receipts |
| `benchmarks/` | managed | no | Benchmark output directories and criterion captures |
| `sessions/` | managed | no | Session handoffs, which span issues rather than belonging to one |
| `archive/` | permanent | no | Archived containers, one directory per container |
| `campaigns/` | permanent | no | Simulation campaign definitions |
| `reference_data/` | permanent | no | Reference curves and external comparison data |
| `research/` | permanent | no | Standalone prototype crates |
| `scripts/` | permanent | no | Benchmark and comparison tooling |
| `simulation_results/` | permanent | no | Campaign outputs |

Tooling lives in `scripts/` rather than `benchmarks/` on purpose: `benchmarks/`
is managed, so anything inside it travels into the archive with its container,
and a live harness must not.

## Adding a Document

1. Pick the area from the table above.
2. In an issue-scoped area, let the tool name the issue's directory instead of
   composing it:

   ```bash
   mkdir -p "$(jit doc dir <issue-id> <area>)"
   ```

3. Link the document to its issue, which is what puts it in reach of archival:

   ```bash
   jit doc add <issue-id> <path>
   ```

   Attach documents this way rather than naming `dev/...` paths inside an issue
   description, so the link survives a rename and `jit doc list` can find it.

4. Keep assets and links portable — [authoring-conventions.md](authoring-conventions.md)
   gives the patterns, and `jit doc check-links` validates them.

`jit doc conformance` reports artifacts sitting outside the directory their
owning issue owns. It is advice: it writes nothing, blocks no transition, and a
listed artifact stays resolvable where it is.

## Archival

A document is archived with the container that owns it. `jit archive container
<id>` plans every artifact linked to a container and its hierarchy descendants;
`jit archive document <path>` targets one document and the bundle it reaches.
Both are read-only previews until `--execute`, which recomputes the plan under
the repository write guard and refuses an ineligible one. A container must be
effectively terminal, and a successful execution retires it into the `Archived`
state.

A container's artifacts land in one directory under `dev/archive/`, named from
the container's short id and the slug its membership label resolves to — so
`epic:gf2-algebra-permanent` on `ae82bd73` gives
`dev/archive/ae82bd73-gf2-algebra-permanent/`. An epic whose label is missing or
is its own short id produces a directory named for nothing; give every container
a descriptive slug before archiving it.

Beneath that directory each artifact keeps its repository-relative source path,
so a document archived out of an area is found again under the same area name
inside the container's directory.

Artifacts outside the development root — crate READMEs, examples, test data —
are retained where they are rather than relocated, even when the container owns
them.

---

## Key Documents

- [../AGENTS.md](../AGENTS.md) — the engineering, testing, architecture, proof, and JIT workflow contract
- [authoring-conventions.md](authoring-conventions.md) — asset and link patterns that survive archival
- [../.jit/reference/rules-and-gates.md](../.jit/reference/rules-and-gates.md) — the rules and quality gates in force
- [../.jit/reference/content-standards.md](../.jit/reference/content-standards.md) — content standards for tracked items

## For Contributors

1. Read [../AGENTS.md](../AGENTS.md).
2. Use `jit query available` to find work.
3. File what the work produces under the area that matches it, attached to its
   issue.
