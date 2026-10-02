# Documentation invariants for 9b2886a7

The three texts below are approved (DEC-01 on issue 9b2886a7) and registered in
`.jit/invariants.toml`; `AGENTS.md` projects them. Sources: brief D-06, D-07,
D-08, D-17, D-18 and the Permanent Documentation Design section.

## Registered invariants

| Subject | ID | Kind | Enforced by |
|---|---|---|---|
| audience | `researcher-audience` | enforced | `@/gate/doc-review` |
| placement | `adapted-diataxis-placement` | enforced | `@/gate/doc-review` |
| tone | `no-marketing` | enforced | `@/gate/doc-review` |

### `researcher-audience`

> Permanent documentation addresses researchers evaluating or adopting gf2: it assumes relevant technical competence, omits elementary finite-field and simplest-code primers, and limits tutorials to research-grade end-to-end workflows over advanced supported capabilities.

### `adapted-diataxis-placement`

> Every permanent documentation page under `docs/` lives in exactly one of `tutorials/` (reproducible research workflows), `how-to/` (focused adoption tasks), `concepts/` (current architecture and algorithmic choices), or `reference/` (supported configurations, limitations, evidence methodology, and stable contracts), and its content matches that directory's purpose.

### `no-marketing`

> Permanent documentation and rustdoc state capabilities and measured results in neutral technical terms; promotional adjectives, superlatives, and unquantified comparative claims are a defect.

## Rejected alternatives

- `researcher-audience`: ID `research-adoption-audience` instead.
- `researcher-audience`: short form: "Permanent documentation serves researchers evaluating or adopting gf2 and assumes relevant technical competence; tutorials are research-grade end-to-end workflows."
- `adapted-diataxis-placement`: ID `diataxis-placement` instead.
- `adapted-diataxis-placement`: short form without purposes: "Every permanent documentation page under `docs/` lives in exactly one of `tutorials/`, `how-to/`, `concepts/`, or `reference/` and matches that directory's purpose."
- `no-marketing`: ID `neutral-technical-tone` instead.
- `no-marketing`: state the rule positively: "Permanent documentation and rustdoc describe capabilities and measured results in neutral technical terms with every comparative claim quantified and evidence-backed."
