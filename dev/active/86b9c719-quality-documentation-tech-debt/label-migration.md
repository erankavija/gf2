# Link-label qualification record (`f023b88b`, `0dc52df8`)

Planner: `dev/scripts/migrate-link-labels.py` (read-only over `.jit/`). Update
script: [`migrate-labels.sh`](migrate-labels.sh), generated at `95c60ccf`.
Coverage and validation results come from a scratch clone at `95c60ccf` with
`migrate-labels.sh` applied.

```sh
python3 dev/scripts/migrate-link-labels.py \
  --script dev/active/86b9c719-quality-documentation-tech-debt/migrate-labels.sh
bash dev/active/86b9c719-quality-documentation-tech-debt/migrate-labels.sh
python3 dev/scripts/migrate-link-labels.py --verify <migrated-copy>  # coverage tables
```

## Label counts

| Namespace | Labels | Rewritten | Ambiguous | Orphan / unregistered |
|---|---|---|---|---|
| satisfies | 736 | 646 | 1 | 89 |
| cites | 177 | 177 | 0 | 0 |

Owner distance of rewritten `satisfies` labels: 1 edge(s): 75, 2 edge(s): 236, 3 edge(s): 86, 4 edge(s): 84, 5 edge(s): 64, 6 edge(s): 60, 7 edge(s): 17, 8 edge(s): 20, 9 edge(s): 2, 10 edge(s): 1, 11 edge(s): 1.

## Unqualified satisfies labels

| Issue | State | Label | Kind | Reason |
|---|---|---|---|---|
| 0064b0a2 | rejected | `satisfies:REQ-02` | orphan | no reaching container declares the id |
| 0064b0a2 | rejected | `satisfies:REQ-04` | orphan | no reaching container declares the id |
| 0064b0a2 | rejected | `satisfies:REQ-05` | orphan | no reaching container declares the id |
| 0064b0a2 | rejected | `satisfies:REQ-06` | orphan | no reaching container declares the id |
| 16560579 | done | `satisfies:REQ-02` | orphan | labelled issue is planning/breakdown; no coverage walk reaches it |
| 16560579 | done | `satisfies:REQ-06` | orphan | labelled issue is planning/breakdown; no coverage walk reaches it |
| 1956017f | rejected | `satisfies:REQ-02` | orphan | no reaching container declares the id |
| 1956017f | rejected | `satisfies:REQ-04` | orphan | no reaching container declares the id |
| 1956017f | rejected | `satisfies:REQ-05` | orphan | no reaching container declares the id |
| 1956017f | rejected | `satisfies:REQ-06` | orphan | no reaching container declares the id |
| 2094958d | rejected | `satisfies:REQ-01` | orphan | no reaching container declares the id |
| 2094958d | rejected | `satisfies:REQ-04` | orphan | no reaching container declares the id |
| 2094958d | rejected | `satisfies:REQ-05` | orphan | no reaching container declares the id |
| 2094958d | rejected | `satisfies:REQ-06` | orphan | no reaching container declares the id |
| 23a08297 | rejected | `satisfies:REQ-02` | orphan | no reaching container declares the id |
| 23a08297 | rejected | `satisfies:REQ-04` | orphan | no reaching container declares the id |
| 23a08297 | rejected | `satisfies:REQ-05` | orphan | no reaching container declares the id |
| 23a08297 | rejected | `satisfies:REQ-06` | orphan | no reaching container declares the id |
| 30c3aef1 | done | `satisfies:REQ-02` | orphan | labelled issue is planning/breakdown; no coverage walk reaches it |
| 30c3aef1 | done | `satisfies:REQ-06` | orphan | labelled issue is planning/breakdown; no coverage walk reaches it |
| 3f57a279 | rejected | `satisfies:REQ-02` | orphan | no reaching container declares the id |
| 3f57a279 | rejected | `satisfies:REQ-04` | orphan | no reaching container declares the id |
| 3f57a279 | rejected | `satisfies:REQ-06` | orphan | no reaching container declares the id |
| 42a22210 | rejected | `satisfies:REQ-02` | orphan | no reaching container declares the id |
| 42a22210 | rejected | `satisfies:REQ-04` | orphan | no reaching container declares the id |
| 42a22210 | rejected | `satisfies:REQ-05` | orphan | no reaching container declares the id |
| 42a22210 | rejected | `satisfies:REQ-06` | orphan | no reaching container declares the id |
| 4497e541 | rejected | `satisfies:REQ-02` | orphan | no reaching container declares the id |
| 4497e541 | rejected | `satisfies:REQ-04` | orphan | no reaching container declares the id |
| 4497e541 | rejected | `satisfies:REQ-05` | orphan | no reaching container declares the id |
| 4497e541 | rejected | `satisfies:REQ-06` | orphan | no reaching container declares the id |
| 44fcbdc2 | rejected | `satisfies:REQ-03` | orphan | no reaching container declares the id |
| 44fcbdc2 | rejected | `satisfies:REQ-05` | orphan | no reaching container declares the id |
| 44fcbdc2 | rejected | `satisfies:REQ-06` | orphan | no reaching container declares the id |
| 49b1611b | rejected | `satisfies:REQ-02` | orphan | no reaching container declares the id |
| 49b1611b | rejected | `satisfies:REQ-05` | orphan | no reaching container declares the id |
| 49b1611b | rejected | `satisfies:REQ-06` | orphan | no reaching container declares the id |
| 552fae3b | rejected | `satisfies:REQ-02` | orphan | no reaching container declares the id |
| 552fae3b | rejected | `satisfies:REQ-04` | orphan | no reaching container declares the id |
| 552fae3b | rejected | `satisfies:REQ-05` | orphan | no reaching container declares the id |
| 552fae3b | rejected | `satisfies:REQ-06` | orphan | no reaching container declares the id |
| 5db985b0 | rejected | `satisfies:REQ-02` | orphan | no reaching container declares the id |
| 5db985b0 | rejected | `satisfies:REQ-04` | orphan | no reaching container declares the id |
| 5db985b0 | rejected | `satisfies:REQ-05` | orphan | no reaching container declares the id |
| 5db985b0 | rejected | `satisfies:REQ-06` | orphan | no reaching container declares the id |
| 6ba2e919 | rejected | `satisfies:REQ-02` | orphan | no reaching container declares the id |
| 6ba2e919 | rejected | `satisfies:REQ-04` | orphan | no reaching container declares the id |
| 6ba2e919 | rejected | `satisfies:REQ-06` | orphan | no reaching container declares the id |
| 7d3ced35 | rejected | `satisfies:REQ-02` | orphan | no reaching container declares the id |
| 7d3ced35 | rejected | `satisfies:REQ-04` | orphan | no reaching container declares the id |
| 7d3ced35 | rejected | `satisfies:REQ-05` | orphan | no reaching container declares the id |
| 7d3ced35 | rejected | `satisfies:REQ-06` | orphan | no reaching container declares the id |
| 80ed6e9f | rejected | `satisfies:REQ-02` | orphan | no reaching container declares the id |
| 80ed6e9f | rejected | `satisfies:REQ-04` | orphan | no reaching container declares the id |
| 80ed6e9f | rejected | `satisfies:REQ-05` | orphan | no reaching container declares the id |
| 80ed6e9f | rejected | `satisfies:REQ-06` | orphan | no reaching container declares the id |
| 8197174d | rejected | `satisfies:REQ-01` | orphan | no reaching container declares the id |
| 8197174d | rejected | `satisfies:REQ-04` | orphan | no reaching container declares the id |
| 8197174d | rejected | `satisfies:REQ-05` | orphan | no reaching container declares the id |
| 8197174d | rejected | `satisfies:REQ-06` | orphan | no reaching container declares the id |
| 833dff1a | rejected | `satisfies:REQ-02` | orphan | no reaching container declares the id |
| 833dff1a | rejected | `satisfies:REQ-04` | orphan | no reaching container declares the id |
| 833dff1a | rejected | `satisfies:REQ-05` | orphan | no reaching container declares the id |
| 833dff1a | rejected | `satisfies:REQ-06` | orphan | no reaching container declares the id |
| 8d8be934 | done | `satisfies:REQ-02` | orphan | labelled issue is planning/breakdown; no coverage walk reaches it |
| 8d8be934 | done | `satisfies:REQ-06` | orphan | labelled issue is planning/breakdown; no coverage walk reaches it |
| 8ed3ac58 | done | `satisfies:REQ-02` | orphan | labelled issue is planning/breakdown; no coverage walk reaches it |
| 8ed3ac58 | done | `satisfies:REQ-06` | orphan | labelled issue is planning/breakdown; no coverage walk reaches it |
| a5f29ae5 | rejected | `satisfies:REQ-02` | orphan | no reaching container declares the id |
| a5f29ae5 | rejected | `satisfies:REQ-04` | orphan | no reaching container declares the id |
| a5f29ae5 | rejected | `satisfies:REQ-06` | orphan | no reaching container declares the id |
| b8ef6f13 | rejected | `satisfies:REQ-02` | orphan | no reaching container declares the id |
| b8ef6f13 | rejected | `satisfies:REQ-05` | orphan | no reaching container declares the id |
| b8ef6f13 | rejected | `satisfies:REQ-06` | orphan | no reaching container declares the id |
| c489745b | rejected | `satisfies:REQ-01` | orphan | no reaching container declares the id |
| c489745b | rejected | `satisfies:REQ-05` | orphan | no reaching container declares the id |
| c489745b | rejected | `satisfies:REQ-06` | orphan | no reaching container declares the id |
| c49e78bc | rejected | `satisfies:REQ-01` | orphan | no reaching container declares the id |
| c49e78bc | rejected | `satisfies:REQ-04` | orphan | no reaching container declares the id |
| c49e78bc | rejected | `satisfies:REQ-05` | orphan | no reaching container declares the id |
| c49e78bc | rejected | `satisfies:REQ-06` | orphan | no reaching container declares the id |
| c71becc5 | rejected | `satisfies:REQ-02` | orphan | no reaching container declares the id |
| c71becc5 | rejected | `satisfies:REQ-04` | orphan | no reaching container declares the id |
| c71becc5 | rejected | `satisfies:REQ-06` | orphan | no reaching container declares the id |
| c9cf13ee | rejected | `satisfies:REQ-02` | orphan | no reaching container declares the id |
| c9cf13ee | rejected | `satisfies:REQ-04` | orphan | no reaching container declares the id |
| c9cf13ee | rejected | `satisfies:REQ-05` | orphan | no reaching container declares the id |
| c9cf13ee | rejected | `satisfies:REQ-06` | orphan | no reaching container declares the id |
| d45aff82 | done | `satisfies:REQ-01` | ambiguous | tie at distance 2: 1a379447, ed3d490e |
| ef18c60b | done | `satisfies:REQ-03` | orphan | no reaching container declares the id |

## Owners outside the labelled issue's membership

The DAG decides the owner; each row's labelled issue carries a different membership label.

| Issue | Label | Owner | Owner membership | Issue membership | Declaring containers @distance |
|---|---|---|---|---|---|
| 377a7a62 | `satisfies:REQ-01` | cce5da8c | epic:qldpc-decoding | epic:osd | cce5da8c@5, b7157be6@6 |
| 3f29e945 | `satisfies:REQ-01` | f357b3dc | story:doc-tersification-sweep | story:documentation-contract | f357b3dc@1, fa787f85@2, 2caf738d@3 |
| 583ec31c | `satisfies:REQ-03` | cce5da8c | epic:qldpc-decoding | epic:osd | cce5da8c@1, b7157be6@2 |
| 5dd3539f | `satisfies:REQ-01` | cce5da8c | epic:qldpc-decoding | epic:osd | cce5da8c@4, b7157be6@5 |
| 6beaf008 | `satisfies:REQ-01` | cce5da8c | epic:qldpc-decoding | epic:osd | cce5da8c@4, b7157be6@5 |
| abd48d99 | `satisfies:REQ-01` | cce5da8c | epic:qldpc-decoding | epic:osd | cce5da8c@3, b7157be6@4 |
| c077a88b | `satisfies:REQ-01` | d77176e5 | epic:competitive-benchmarking | epic:zen3-cpu-performance | d77176e5@2, 2caf738d@3, ed3d490e@4, 1a379447@5 |
| c97b2961 | `satisfies:REQ-01` | cce5da8c | epic:qldpc-decoding | epic:osd | cce5da8c@2, b7157be6@3 |
| f547c394 | `satisfies:REQ-01` | d77176e5 | epic:competitive-benchmarking | epic:zen3-cpu-performance | d77176e5@3, 1a379447@4, 2caf738d@4, ed3d490e@5 |

## Unregistered cites labels

None.

## Coverage before and after

| Measure | Before (unqualified) | After (qualified) |
|---|---|---|
| Hard requirements | 187 | 187 |
| Credited | 106 | 102 |
| Fully covered containers | 17 | 13 |

Rule-evaluated: an epic (`hard-criteria-covered`) or a container named by a `brackets:` label (`coverage-preview`).

### Container verdict changes

| Container | Rule-evaluated | Before | After |
|---|---|---|---|
| 2caf738d | no | covered | uncovered |
| 3931ac6f | no | covered | uncovered |
| 86b9c719 | yes | covered | uncovered |
| b7157be6 | yes | covered | uncovered |

### Requirement credit changes

Each row lists where the labels that credited the requirement before now resolve (owner short id or unresolved kind, with label count).

| Container | Rule-evaluated | Requirement | Before | After | Former crediting labels |
|---|---|---|---|---|---|
| 2caf738d | no | REQ-01 | credited | uncredited | 1a379447 ×12, 2037941f ×9, 6dc81018 ×1, ae03bcd0 ×4, ambiguous ×1, b4b4b9ee ×14, c04dd4ac ×4, d77176e5 ×3, ed3d490e ×1, f357b3dc ×1, fa787f85 ×3 |
| 3931ac6f | no | REQ-01 | credited | uncredited | ae03bcd0 ×4, b4b4b9ee ×1 |
| 86b9c719 | yes | REQ-01 | credited | uncredited | 6dc81018 ×1, ae03bcd0 ×4, b4b4b9ee ×14 |
| b7157be6 | yes | REQ-01 | credited | uncredited | cce5da8c ×5 |

## Validation after migration

| Rule | Errors | Subject |
|---|---|---|
| `dangling-item-link` | 90 | The unqualified `satisfies` labels listed above; no `cites` label |
| `hard-criteria-covered` | 1 | `b7157be6` REQ-01 |
| `coverage-preview` | 1 | `b7157be6` REQ-01 |
| derived-state drift | 1 | `.jit/.gitignore` absent |
