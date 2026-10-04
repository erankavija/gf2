# docs-mechanical baseline findings

The `baseline` key of `[docs-mechanical]` in `.jit/config.toml` names this
record, and the check suppresses exactly its rows. Each row quotes an
unresolvable address on purpose (8f61d6de DEC-01).

| Location | Class | Target | Reason |
|---|---|---|---|
| `dev/active/6dc81018-field-capability-dispatch/handoff-4.md:31` | `unresolved-item` | `@/inv/canonical-abstraction` | Records the dangling citation a code review rejected. |
| `dev/active/6dc81018-field-capability-dispatch/handoff-4.md:54` | `unresolved-item` | `@/inv/canonical-abstraction` | Names the dangling citation as a trap to avoid. |
| `dev/active/1a379447-zen3-cpu-performance/9f0e385e/coverage-addressability.md:19` | `unresolved-item` | `@/issue/c04dd4ac/requirement/REQ-06` | Example of an address that does not resolve. |
