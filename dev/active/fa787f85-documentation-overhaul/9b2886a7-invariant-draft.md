# Documentation invariant draft for 9b2886a7

Draft texts for user approval (REQ-01, REQ-02). Nothing here is registered.
Sources: brief D-06, D-07, D-08, D-17, D-18 and the Permanent Documentation
Design section; format follows `495807a3-invariant-draft.md`.

## Proposed invariants

| Subject | ID | Kind | Enforced by |
|---|---|---|---|
| audience | `researcher-audience` | enforced | `@/gate/doc-review` |
| placement | `adapted-diataxis-placement` | enforced | `@/gate/doc-review` |
| tone | `no-marketing` | enforced | `@/gate/doc-review` |

`docs-mechanical` (D-35) can take over `adapted-diataxis-placement` once it
checks that every page under `docs/` sits in one of the four directories, and
can add a lexicon check for `no-marketing`; the audience rule stays with
`doc-review`.

### `researcher-audience`

> Permanent documentation addresses researchers evaluating or adopting gf2: it assumes relevant technical competence, omits elementary finite-field and simplest-code primers, and limits tutorials to research-grade end-to-end workflows over advanced supported capabilities.

### `adapted-diataxis-placement`

> Every permanent documentation page under `docs/` lives in exactly one of `tutorials/` (reproducible research workflows), `how-to/` (focused adoption tasks), `concepts/` (current architecture and algorithmic choices), or `reference/` (supported configurations, limitations, evidence methodology, and stable contracts), and its content matches that directory's purpose.

### `no-marketing`

> Permanent documentation and rustdoc state capabilities and measured results in neutral technical terms; promotional adjectives, superlatives, and unquantified comparative claims are a defect.

## Superseded AGENTS.md prose and line budget

The projection renders one line per invariant (`style = "id-anchor"`), so three
invariants add three lines inside the region. Baseline is `AGENTS.md` at 193
lines after 495807a3 lands.

| Invariant | Superseded lines | Replacement outside region | Removed | Added |
|---|---|---|---|---|
| `researcher-audience` | none; no hand-written source | none | 0 | 0 |
| `adapted-diataxis-placement` | none; the 495807a3 replacement sentence "Keep permanent documentation under README.md, crate-level rustdoc, or docs/" names the roots, the invariant names the layout inside `docs/` | none | 0 | 0 |
| `no-marketing` | none; no hand-written source | none | 0 | 0 |
| Projection region | — | three new lines | 0 | 3 |

Net: 193 + 3 = **196 lines**. The budget holds; with the 194-line variant of
495807a3 the total is 197.

## Wording alternatives

- `researcher-audience`: ID `research-adoption-audience` instead.
- `researcher-audience`: short form: "Permanent documentation serves researchers evaluating or adopting gf2 and assumes relevant technical competence; tutorials are research-grade end-to-end workflows."
- `adapted-diataxis-placement`: ID `diataxis-placement` instead.
- `adapted-diataxis-placement`: short form without purposes: "Every permanent documentation page under `docs/` lives in exactly one of `tutorials/`, `how-to/`, `concepts/`, or `reference/` and matches that directory's purpose."
- `no-marketing`: ID `neutral-technical-tone` instead.
- `no-marketing`: state the rule positively: "Permanent documentation and rustdoc describe capabilities and measured results in neutral technical terms with every comparative claim quantified and evidence-backed."
