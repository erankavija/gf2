# docs-mechanical baseline findings

Temporary record: finalize-docs-policy (d6ebc4ec) removes it together with the
`baseline` key of `[docs-mechanical]` in `.jit/config.toml`. The check
suppresses exactly the rows below; each quotes an unresolvable address on
purpose (8f61d6de DEC-01).

| Location | Class | Target | Reason |
|---|---|---|---|
| `dev/active/6dc81018-field-capability-dispatch/handoff-4.md:31` | `unresolved-item` | `@/inv/canonical-abstraction` | Records the dangling citation a code review rejected. |
| `dev/active/6dc81018-field-capability-dispatch/handoff-4.md:54` | `unresolved-item` | `@/inv/canonical-abstraction` | Names the dangling citation as a trap to avoid. |
| `dev/active/9f0e385e/coverage-addressability.md:19` | `unresolved-item` | `@/issue/c04dd4ac/requirement/REQ-06` | Example of an address that does not resolve. |
