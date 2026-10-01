# Documentation invariant draft for 495807a3

Draft texts for user approval (REQ-03, DEC-03). Nothing here is registered.

## REQ-01: resolution and `enforces` namespace

Invariant addresses resolve through both the alias and the canonical kind:

```console
$ jit item show @/inv/present-tense-prose
Qualified id: @/invariant/present-tense-prose
Kind:         invariant
Self id:      present-tense-prose
Scope:        @ (project)
Text:         Permanent artifacts describe the current state of the system; change-relative narration — old/new contrasts, previously/now framing, legacy markers, migration history — lives in commit messages and issue history.
$ echo $?
0
```

The `enforces` namespace is declared and bound to the invariant kind in
`.jit/config.toml`:

```console
$ grep -n -A3 '\[namespaces.enforces\]' .jit/config.toml
61:[namespaces.enforces]
62-description = "Enforcement link: names an invariant, rule, or gate item that the labeled issue enforces."
63-examples = ["enforces:@/invariant/<self-id>", "enforces:@/rule/label-format", "enforces:@/gate/<self-id>"]
64-unique = false
$ grep -n -A4 '\[item_kinds.invariant\]' .jit/config.toml
122:[item_kinds.invariant]
123-aliases = ["inv"]
124-id-pattern = "[a-z][a-z0-9-]*"
125-link-namespaces = ["enforces"]
```

Live `enforces` labels already resolve to invariants:

```console
$ jit label values enforces
Values in namespace 'enforces':
  @/invariant/behavioral-evidence-validity
  @/invariant/caller-trusted-fast-paths
  @/invariant/campaign-resumability
  @/invariant/convention-convergence
  @/invariant/runtime-observed-provenance
Total: 5
```

## Proposed invariants

Group (c) is split into three invariants: rustdoc examples, active-document
layout and `AGENTS.md` scope share no subject, and one sentence carrying all
three reads as a list. Five invariants result.

| Group | ID | Kind | Enforced by |
|---|---|---|---|
| a | `non-obvious-comments` | enforced | `@/gate/code-review` |
| b | `current-state-scope` | enforced | `@/gate/doc-review` |
| c1 | `teaching-rustdoc-examples` | enforced | `@/gate/code-review` |
| c2 | `active-document-layout` | enforced | `@/gate/doc-review` |
| c3 | `agents-md-scope` | enforced | `@/gate/doc-review` |

`docs-mechanical` (8f61d6de) can take over `agents-md-scope` and
`active-document-layout` enforcement once it exists.

### (a) `non-obvious-comments`

> Comments and rustdoc carry only what the code leaves non-obvious — a contract, an invariant, the reason for a choice, or a pointer to evidence — in the shortest form that conveys it and without restating code, signatures, or other documentation; a module-level orientation is at most one short paragraph.

### (b) `current-state-scope`

> Documentation states the current behavior and limits of the system only; speculation, future promises, non-goals, and deferrals are omitted, and planned work lives in JIT issues.

### (c1) `teaching-rustdoc-examples`

> A rustdoc example teaches a workflow or clarifies a material contract that prose and focused tests leave unclear; examples for accessors, constants, constructors, predicates, and direct field mappings are a defect.

### (c2) `active-document-layout`

> Active development documents live under `dev/active/<epic-short-id>-<slug>/` and are linked to their owning issues with `jit doc`.

### (c3) `agents-md-scope`

> Every `AGENTS.md` governs its directory subtree, overrides less specific guidance only within that subtree, states nothing that belongs to a broader or narrower scope, and stays at or below 200 lines.

## Superseded AGENTS.md prose and line budget

The projection renders one line per invariant (`style = "id-anchor"`), so five
invariants add five lines inside the region. Line numbers refer to `AGENTS.md`
at 199 lines.

| Invariant | Superseded lines | Replacement outside region | Removed | Added |
|---|---|---|---|---|
| `non-obvious-comments` | 101–104 (comment bullet) | none | 4 | 0 |
| `current-state-scope` | none; the rule has no hand-written source | none | 0 | 0 |
| `teaching-rustdoc-examples` | 95–100 (rustdoc bullet) | `- Public APIs need rustdoc stating purpose, panics, safety conditions, and`<br>`  non-obvious complexity.` | 6 | 2 |
| `active-document-layout` | 127–131 (placement paragraph) | `Keep permanent documentation under README.md, crate-level rustdoc, or`<br>`docs/. Prefer citations or generated projections over copied facts. See`<br>`@/inv/single-source-prose.` | 5 | 3 |
| `agents-md-scope` | 3–6 (last sentence, lines 5–6) | lines 3–5 keep the first two sentences | 4 | 3 |
| Projection region | — | five new lines | 0 | 5 |

Net: 199 − 19 + 13 = **193 lines**.

If the owner keeps lines 5–6 so the hand-written preamble still defines
recursive scope for REQ-34 of 3f29e945, the total is 194 lines. The budget
holds in both variants.

Unchanged: the `dev/active` area itself stays registered in `.jit/config.toml`
(`managed_paths`, `issue_scoped_areas`); the invariant states the layout
inside it.

## Wording alternatives

- (a) ID `minimal-comments` or `comment-economy` instead of `non-obvious-comments`.
- (a) Drop the "without restating …" clause and rely on `single-source-prose` for duplication.
- (a) Split the module-orientation limit into its own invariant `short-module-orientation`.
- (b) ID `current-behavior-only` or `present-scope-prose` instead of `current-state-scope`.
- (b) Name the forbidden forms positively: "Documentation states current behavior and limits; planned, speculative, or excluded work is recorded only in JIT issues."
- (b) Merge into `present-tense-prose` as one forward- and backward-looking rule; it changes an approved text, so it is not the default.
- (c1) ID `rustdoc-example-value` instead of `teaching-rustdoc-examples`.
- (c1) Short form: "Rustdoc examples are non-trivial: each teaches a workflow or a material contract."
- (c2) Add "and leave it when the owning issue completes" to state the lifecycle.
- (c3) Short form: "Every `AGENTS.md` governs only its subtree and stays at or below 200 lines."
- (c) Single sentence for all three: "Rustdoc examples teach a workflow or material contract, active development documents live under `dev/active/<epic-short-id>-<slug>/` linked to their owning issues, and every `AGENTS.md` governs only its subtree within 200 lines."
- Enforcement: `advisory` kind for `teaching-rustdoc-examples` if the owner wants existing trivial examples to stay non-blocking until the rustdoc audit lands.
