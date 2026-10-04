# Breakdown coverage addressability

The `satisfies:REQ-06` label on breakdown `16560579` refers to
`@/issue/1a379447/requirement/REQ-06`, the Zen 3 epic's correctness and
compatibility requirement. It is an inherited contribution annotation; the
configured coverage rules exclude breakdown nodes from coverage credit.

## Resolution

The canonical registry resolves the target with:

```sh
jit item show @/issue/1a379447/requirement/REQ-06 --json
```

The owning epic is established by the dependency path through story `c04dd4ac`
and the `epic:zen3-cpu-performance` membership label. The story itself carries
the same `satisfies:REQ-06` annotation. Its own requirement namespace is
separate: `jit item show @/issue/c04dd4ac/requirement/REQ-06 --json` reports no
such item. The planning node's local criteria are also not the target.

The scaffold record at commit `8295b69f1` adds the breakdown with the story's
contribution labels already present. This historical record explains the
inheritance; the current requirement registry and graph establish its target.

## Coverage effect

The namespace declaration in `.jit/config.toml` defines `satisfies` as a
contribution to a container criterion, not sole delivery. The `brackets`
namespace separately identifies the container whose decomposition is reviewed.
It does not redefine every inherited contribution label as a local requirement.

Both `@/rule/hard-criteria-covered` and `@/rule/coverage-preview` exclude
`planning` and `breakdown` node types in `.jit/rules.toml`. Consequently this
annotation gives no executable coverage credit, either to the epic or to the
bracketing story. The story preview checks implementation descendants against
the story's requirements; the epic completion rule checks completed
implementation descendants against the epic's requirements.

## Disposition

Retain the contribution annotation with the addressable epic target above.
It neither claims a missing story requirement nor substitutes for completed
implementation evidence. No label, criterion, gate or scope change is needed.

Reproduce the audit with the registry commands above, `jit issue show` for
`16560579`, `8ed3ac58`, `c04dd4ac` and `1a379447`, `jit graph deps c04dd4ac`,
and the two rule declarations. The final epic review still verifies the
correctness requirement against its delivered evidence.
