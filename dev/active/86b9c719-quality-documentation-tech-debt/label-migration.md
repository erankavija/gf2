# Link-label qualification record (`f023b88b`, `0dc52df8`)

Planner: `dev/scripts/migrate-link-labels.py` (read-only over `.jit/`).
[`migrate-labels.sh`](migrate-labels.sh) is generated at `95c60ccf`;
[`migrate-labels-2.sh`](migrate-labels-2.sh) is generated at `358b19f0`, after
the first script, with `--drop-orphans`. Tables below cover both scripts
together, planned from the `95c60ccf` labels; coverage and validation come from
a scratch clone of `358b19f0` with `migrate-labels-2.sh` applied.

```sh
python3 dev/scripts/migrate-link-labels.py --drop-orphans \
  --script dev/active/86b9c719-quality-documentation-tech-debt/migrate-labels-2.sh
bash dev/active/86b9c719-quality-documentation-tech-debt/migrate-labels-2.sh
python3 dev/scripts/migrate-link-labels.py --root <pre-migration-copy> \
  --items-from <migrated-copy> --verify <migrated-copy>  # coverage tables
```

## Ownership rules

| Case | Owner | Decided by |
|---|---|---|
| Containers reaching the labelled issue declare the id | Nearest (fewest dependency edges) | `f023b88b` REQ-01 |
| A declaring container is named by a membership label of the labelled issue | Nearest such member, outranking nearer non-members | Owner decision |
| Tie at the nearest distance | Label unchanged and listed | `f023b88b` REQ-02 |
| No reaching container declares the id | Label removed | Owner decision |

Containers are `milestone`, `epic` and `story` issues; the coverage walk never
enters `planning` or `breakdown` issues.

## Label counts

| Namespace | Unqualified | Rewritten | Requalified | Ambiguous | Orphan / unregistered |
|---|---|---|---|---|---|
| satisfies | 736 | 647 | 0 | 0 | 89 |
| cites | 177 | 177 | 0 | 0 | 0 |

Owner distance of rewritten `satisfies` labels: 1 edge(s): 73, 2 edge(s): 237, 3 edge(s): 85, 4 edge(s): 84, 5 edge(s): 66, 6 edge(s): 61, 7 edge(s): 17, 8 edge(s): 20, 9 edge(s): 2, 10 edge(s): 1, 11 edge(s): 1.

## Removed satisfies labels

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
| ef18c60b | done | `satisfies:REQ-03` | orphan | no reaching container declares the id |

## Owners outside the labelled issue's membership

Rewritten labels whose owner carries no membership label of the labelled issue while the issue is a member of another container of that type.

None.

## Unregistered cites labels

None.

## Labels requalified by `migrate-labels-2.sh`

Qualified by `migrate-labels.sh` to the plain nearest owner; moved to the membership owner. The same script qualifies `d45aff82 satisfies:REQ-01`, tied at distance 2 between `1a379447` and `ed3d490e`, to its membership owner `1a379447`, and removes the 89 labels listed above.

| Issue | Label | Membership owner | Declaring containers @distance |
|---|---|---|---|
| 377a7a62 | `satisfies:cce5da8c/REQ-01` | b7157be6 | cce5da8c@5, b7157be6@6 |
| 3f29e945 | `satisfies:f357b3dc/REQ-01` | fa787f85 | f357b3dc@1, fa787f85@2, 2caf738d@3 |
| 583ec31c | `satisfies:cce5da8c/REQ-03` | b7157be6 | cce5da8c@1, b7157be6@2 |
| 5dd3539f | `satisfies:cce5da8c/REQ-01` | b7157be6 | cce5da8c@4, b7157be6@5 |
| 6beaf008 | `satisfies:cce5da8c/REQ-01` | b7157be6 | cce5da8c@4, b7157be6@5 |
| abd48d99 | `satisfies:cce5da8c/REQ-01` | b7157be6 | cce5da8c@3, b7157be6@4 |
| c077a88b | `satisfies:d77176e5/REQ-01` | 1a379447 | d77176e5@2, 2caf738d@3, ed3d490e@4, 1a379447@5 |
| c97b2961 | `satisfies:cce5da8c/REQ-01` | b7157be6 | cce5da8c@2, b7157be6@3 |
| f547c394 | `satisfies:d77176e5/REQ-01` | 1a379447 | d77176e5@3, 1a379447@4, 2caf738d@4, ed3d490e@5 |

## Coverage before and after

| Measure | Before (unqualified) | After (qualified) |
|---|---|---|
| Hard requirements | 187 | 187 |
| Credited | 106 | 100 |
| Fully covered containers | 17 | 14 |

Rule-evaluated: an epic (`hard-criteria-covered`) or a container named by a `brackets:` label (`coverage-preview`).

### Container verdict changes

| Container | Rule-evaluated | Before | After |
|---|---|---|---|
| 2caf738d | no | covered | uncovered |
| 3931ac6f | no | covered | uncovered |
| 86b9c719 | yes | covered | uncovered |

### Requirement credit changes

Each row lists where the labels that credited the requirement before now resolve (owner short id or unresolved kind, with label count).

| Container | Rule-evaluated | Requirement | Before | After | Former crediting labels |
|---|---|---|---|---|---|
| 2caf738d | no | REQ-01 | credited | uncredited | 1a379447 ×15, 2037941f ×9, 6dc81018 ×1, ae03bcd0 ×4, b4b4b9ee ×14, c04dd4ac ×4, d77176e5 ×1, ed3d490e ×1, fa787f85 ×4 |
| 3931ac6f | no | REQ-01 | credited | uncredited | ae03bcd0 ×4, b4b4b9ee ×1 |
| 86b9c719 | yes | REQ-01 | credited | uncredited | 6dc81018 ×1, ae03bcd0 ×4, b4b4b9ee ×14 |
| cce5da8c | yes | REQ-01 | credited | uncredited | b7157be6 ×5 |
| cce5da8c | yes | REQ-03 | credited | uncredited | b7157be6 ×1 |
| f357b3dc | no | REQ-01 | credited | uncredited | fa787f85 ×1 |

## Validation after migration

| Rule | Errors |
|---|---|
| `dangling-item-link` | 0 |
| `hard-criteria-covered` | 0 |
| `coverage-preview` | 0 |
| derived-state drift | 1 (`.jit/.gitignore` absent) |
