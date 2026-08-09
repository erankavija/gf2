# Permanent zero-fraction datasets

This directory is the permanent home for published campaign outputs that
measure permanent-zero fractions over small prime fields. Each child directory
is one immutable, versioned dataset:

`dev/simulation_results/permanent-zero-fraction/<campaign-id>/`

A campaign id uses lowercase ASCII letters, digits, and interior hyphens. A
writer must refuse an id whose directory already exists. Corrections,
extensions, reruns, and schema migrations always receive a new campaign id;
they never overwrite an existing dataset in place.

The canonical typed schema and conformance reader are
`gf2_sim::permanent_campaign::schema`. JSON documents reject missing required
fields and unknown fields. Every JSON document and every pooled CSV row carries
the same integer `schema_version`; the reader accepts only the version named by
that module's `SCHEMA_VERSION` constant.

## Layout and ownership

| Path relative to `<campaign-id>/` | Class | Exclusive writer | Purpose |
| --- | --- | --- | --- |
| `manifest.json` | raw data | finalization | Frozen campaign identity, grid, streams, backend policy, and mechanical provenance |
| `shards/q<q>/n<nn>/shard-<index>.json` | raw data | execution of field $q$ | Counts for one independently regenerable shard |
| `summaries/q<q>.json` | raw data | execution of field $q$ | One typed summary row for each completed or halted cell in the field arm |
| `summary.csv` | raw data | finalization | Deterministic pooling of all field-summary rows |
| `checksums.sha256` | integrity metadata | finalization | SHA-256 entries for exactly the raw-data paths above; it does not cover itself |
| `derived/` | derived artefacts | analysis tasks | Reports, figures, tables, and fit outputs |

Every required file has exactly one writer role. Field executions may write
only their field-scoped shard and summary paths. They never write
`manifest.json`, `summary.csv`, or `checksums.sha256`. Finalization is the only
writer of those campaign-scoped paths and does not rewrite field-scoped paths.
Consequently the three field arms may execute concurrently without two writers
targeting one file.

The integrity set is deliberately closed before analysis begins. It covers the
manifest, shard records, field summaries, and pooled summary. Derived artefacts
live under `derived/` and are not members of that set: a checksum file cannot
close if it also covers reports or figures that quote its value.

## Root manifest schema

`manifest.json` is one `CampaignManifest` with these required fields:

| Field | Shape | Mechanical meaning |
| --- | --- | --- |
| `schema_version` | integer | On-disk schema version |
| `campaign_id` | constrained token | Immutable directory identity |
| `root_seed` | unsigned integer | Campaign root seed |
| `stream_purposes` | array of `{name, tag}` | Complete purpose namespace and each purpose's 8-bit domain-separation tag |
| `cells` | array of cell specifications | Frozen $(q,n)$ grid |
| `provenance` | provenance record | Git, compiler, runtime, and hardware identity |

Each cell specification carries `q`, `n`, the preregistered `matrix_count`
($N$), `shard_size`, the ordered `{shard_id, stream_index}` identities, the
selected `backend`, and `determinant_companion`. The supported backend tokens
are `scalar`, `batch_parallel`, `intra_matrix_parallel`, `generic_ryser`, and
`accelerator`. The determinant plan is `evaluate` or `not_evaluated`.

The provenance record requires the full `git_revision`, `compiler_version`,
`cpu_model`, `accelerator_runtime`, and `gpu_model`. Accelerator runtime and GPU
model use a tagged availability value: either `{"state":"present","value":...}`
or `{"state":"not_present"}`. Absence is therefore explicit rather than an
empty or overloaded string.

Purpose names are constrained serialization-boundary labels. The tag is the
domain-separation identity consumed by the stream address. The statistical
sampler owns the executable purpose namespace; the manifest records the frozen
name-to-tag mapping without defining a second sampler-side enumeration.

## Shard record schema

Each shard path contains one `ShardRecord` JSON object with:

- `schema_version` and `shard_id`;
- `stream_address`, containing `root_seed`, `q`, `n`, `purpose_tag`, and the
  low-56-bit `stream_index`;
- `matrix_count` and `permanent_zero_count`;
- `permanent_histogram`, whose $q$ ordered bins count residues
  $0,\ldots,q-1$ and sum to `matrix_count`;
- `determinant`, in one of the two forms below.

```json
{"state":"not_evaluated"}
```

```json
{"state":"evaluated","sample_count":1000,"zero_count":438}
```

The `not_evaluated` form admits no `sample_count` or `zero_count`. Numeric zero
therefore always means an evaluated sample found zero singular matrices; it
never means that the companion did not run. Matrices themselves are omitted
because the complete stream address regenerates them, keeping storage
$O(\text{shards})$.

## Field and pooled summary schemas

Each `summaries/q<q>.json` file is a `FieldSummary` containing
`schema_version`, `q`, and `rows`. A `SummaryRow` is shared by field summaries
and `summary.csv`; per $(q,n)$ it contains:

- pooled `matrix_count` and `permanent_zero_count`;
- `permanent_estimate` with `point` and interval `lower`/`upper` endpoints;
- `permanent_verdict`, either `accepted` or `rejected`;
- the determinant summary state;
- `terminal_state`.

An evaluated determinant summary contains `sample_count`, `zero_count`, its own
point estimate and interval, and its acceptance verdict. A not-evaluated
determinant summary is exactly `{"state":"not_evaluated"}` and carries no
numeric fields. The pooled CSV expresses the same distinction with
`determinant_state=not_evaluated` and empty determinant count, estimate,
interval, and verdict columns.

A terminal state is either `completed` or `halted` with one mechanical reason
code: `acceptance_failure`, `backend_unavailable`, or `execution_failure`.
Halted cells remain in the raw dataset; omission is not a terminal state.

`summary.csv` uses the exact header exported as `SUMMARY_CSV_FIELDS`. Columns
are numeric values or closed-vocabulary tokens, so the format admits no
free-form interpretive prose. The JSON schemas likewise contain only counts,
states, identifiers, and mechanical provenance. Scientific meaning,
conclusions, novelty claims, and explanatory narrative belong only in derived
artefacts.

## Conformance

`conform_dataset(<campaign-id-directory>)` checks the complete raw shape. It
rejects an absent required path, malformed JSON or CSV, a missing or unknown
field, a wrong schema version, invalid count relationships, a shard address
that differs from the manifest, a field summary that differs from pooled
shards, and a pooled summary that differs from the field summaries.

Cryptographic checksum generation and verification are a separate integrity
layer. Schema conformance establishes the file shapes and cross-file count
relationships; it does not treat the placeholder existence of
`checksums.sha256` as proof of content integrity.
