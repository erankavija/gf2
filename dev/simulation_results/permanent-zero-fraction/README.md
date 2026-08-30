# Permanent zero-fraction datasets

This directory is the permanent home for published campaign outputs that
measure permanent-zero fractions over small prime fields. Each child directory
is one immutable, versioned dataset:

`dev/simulation_results/permanent-zero-fraction/<campaign-id>/`

The campaign's controlling [scientific preregistration](protocol.md) is stored
beside those datasets; it is not part of any dataset's raw or derived file set.

A campaign id uses lowercase ASCII letters, digits, and interior hyphens.
Dataset-scale writers must refuse an existing campaign-id directory, while a
field-scale writer refuses every existing shard or summary file inside the
selected campaign directory. Corrections, extensions, reruns, and schema
migrations always receive a new campaign id; they never overwrite an existing
dataset in place.

The canonical typed schema and conformance reader are
`gf2_sim::permanent_campaign::schema`. JSON documents reject missing required
fields and unknown fields. The writer emits the version named by
`SCHEMA_VERSION`, while the reader accepts every version named by
`READABLE_SCHEMA_VERSIONS`. Every JSON document and every pooled CSV row in one
dataset carries the manifest's same integer `schema_version`; a dataset may not
mix versions.

## Layout and ownership

| Path relative to `<campaign-id>/` | Class | Exclusive writer | Purpose |
| --- | --- | --- | --- |
| `manifest.json` | raw data | finalization | Frozen campaign identity, grid, streams, backend policy, and mechanical provenance |
| `shards/q<q>/n<nn>/shard-<index>.json` | raw data | execution of field $q$ | Counts for one independently regenerable shard |
| `summaries/q<q>.json` | raw data | execution of field $q$ | One typed summary row for each completed or halted cell in the field arm |
| `summary.csv` | raw data | finalization | Deterministic pooling of all field-summary rows |
| `checksums.sha256` | integrity metadata | finalization | SHA-256 entries for exactly the raw-data paths above; it does not cover itself |
| `derived/` | derived artefacts | analysis tasks | Reports, figures, tables, and fit outputs |
| `freeze.md` | frozen decision record | finalization | Human-auditable freeze decisions the strict manifest schema does not carry |

Every required file has exactly one writer role. Field executions may write
only their field-scoped shard and summary paths. They never write
`manifest.json`, `summary.csv`, or `checksums.sha256`. Finalization is the only
writer of those campaign-scoped paths and does not rewrite field-scoped paths.
Consequently the three field arms may execute concurrently without two writers
targeting one file.

The integrity set is deliberately closed before analysis begins. It covers the
manifest, shard records, field summaries, and pooled summary. Derived artefacts
live under `derived/` and are not members of that set: a checksum file cannot
close if it also covers reports or figures that quote its value. The freeze
record `freeze.md` quotes sidecar entries, so it is likewise outside the set;
its tamper evidence is repository history, where it is a committed, tracked
file.

## Root manifest schema

`manifest.json` is one `CampaignManifest` with these required fields:

| Field | Shape | Mechanical meaning |
| --- | --- | --- |
| `schema_version` | integer | On-disk schema version |
| `campaign_id` | constrained token | Immutable directory identity |
| `root_seed` | unsigned integer | Campaign root seed |
| `stream_purposes` | array of `{name, tag}` | Complete purpose namespace and each purpose's 8-bit domain-separation tag |
| `cells` | array of cell specifications | Frozen $(q,n)$ grid, backend, and backend-selection receipt identity |
| `provenance` | provenance record | Git, compiler, RNG, invocation, runtime, and hardware identity |

Each cell specification carries `q`, `n`, the preregistered `matrix_count`
($N$), `shard_size`, the ordered `{shard_id, stream_index}` identities, the
selected `backend`, and `determinant_companion`. The supported backend tokens
are `scalar`, `batch_parallel`, `intra_matrix_parallel`, `generic_ryser`, and
`accelerator`. Each cell's `backend_receipt` is an `ArtifactIdentity` containing
the committed selection receipt's normalized repository-relative `path` and
lowercase hexadecimal `sha256`. The identity binds the selected backend to the
exact measurements and deterministic selection record used for that cell,
without assigning an artifact subtype. The determinant plan is `evaluate` or
`not_evaluated`.

The provenance record requires the full `git_revision`, `binary_sha256`,
`deps_source_revision`, `deps_source_dirty`, `compiler_version`, `rng_algorithm`,
`rng_version`, tokenized `invocation`, `cpu_model`, `accelerator_runtime`, and
`gpu_model`. `git_revision` is the repository-wide revision observed at run
start, recorded as context. `deps_source_revision` is the revision of the
linked source closure (`crates/` and `Cargo.lock`), and `deps_source_dirty`
records whether that closure was dirty at run start. `binary_sha256` is the
lowercase hexadecimal SHA-256 of the executable that emitted the dataset, and
the emission guard admits a writer only when the running executable matches it.
The emitting executable supplies this value. Build it with
`cargo build -p gf2-sim --release --bin permanent_campaign`, then run that
same build as
`permanent_campaign --print-provenance --manifest <campaign-directory>` and
record the `binary_sha256` it reports in the manifest before freezing it. The
digest identifies one build, so rebuilding the emitter changes it and the
manifest is refrozen against the rebuilt executable. A manifest frozen with
the 64-zero placeholder digest refuses every emission.
Version-1 datasets predate all three version-2 fields and omit them; the reader
accepts their absence only at version 1. The revision is the complete
40-character lowercase hexadecimal object name; an abbreviation resolves only
against the repository that produced it, so it cannot identify the source of a
dataset read elsewhere. The RNG algorithm is the closed schema token
`cha_cha20`; `rng_version` records the exact crate or implementation version,
and `invocation` stores the producer's argv tokens without shell quoting.
Accelerator runtime and GPU model use a tagged availability value: either
`{"state":"present","value":...}` or `{"state":"not_present"}`. Absence is
therefore explicit rather than an empty or overloaded string.

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
`schema_version`, `q`, `rows`, and — only when present — `quarantined`, a
diagnostic index retaining only each failed shard's stable identity (`q`, `n`,
`shard_id`) and mechanical `error` diagnostic so the failure remains visible
instead of being silently absent. A quarantined shard remains excluded from the
raw dataset and pooling; the committed campaign execution receipt is the
authoritative quarantine evidence, preserving its bytes, logs, observed counts,
failure reason, and attempt numbers. A `SummaryRow` is shared by field summaries
and `summary.csv`; per $(q,n)$ it contains:

- pooled `matrix_count` and `permanent_zero_count`;
- determinant counts or the explicit not-evaluated state;
- one typed `terminal_state` outcome.

An evaluated determinant count contains `sample_count` and `zero_count`. A
not-evaluated determinant is exactly `{"state":"not_evaluated"}` and carries
no numeric fields. Counts are independent of terminal outcome so a halted cell
preserves every accepted shard count.

The completed terminal outcome contains the permanent point estimate,
interval, and acceptance verdict plus a determinant estimate/verdict or
explicit non-evaluation. The halted terminal outcome carries only one
mechanical reason code: `acceptance_failure`, `backend_unavailable`, or
`execution_failure`; it forbids completed estimates and verdicts. A halted cell
may contain any unique subset of its manifest-planned shard paths, including no
shards, and its counts pool exactly that subset. A completed cell requires all
planned shards and the full preregistered count. Halted cells remain in the raw
dataset; omission is not a terminal state.

The pooled CSV expresses the same typed outcome through its flat stable header.
Completed rows fill the permanent estimate/verdict columns and the applicable
determinant estimate/verdict columns. Halted rows leave all estimate and verdict
columns empty while retaining permanent and determinant count columns.

`summary.csv` uses the exact header exported as `SUMMARY_CSV_FIELDS`. Columns
are numeric values or closed-vocabulary tokens, so the format admits no
free-form interpretive prose. The JSON schemas likewise contain only counts,
states, identifiers, and mechanical provenance. Scientific meaning,
conclusions, novelty claims, and explanatory narrative belong only in derived
artefacts.

## Conformance

`conform_dataset(<campaign-id-directory>)` checks the complete raw shape. It
rejects an absent required path, malformed JSON or CSV, a missing or unknown
field, a wrong schema version, a campaign id that differs from the dataset
directory name, invalid count relationships, a shard address that differs from
the manifest, a field-summary or row field identity that differs from its path,
an unmanifested shard path, a completed cell missing a planned shard, a field
summary that differs from its executed shard subset, and a pooled summary that
differs from the field summaries. The returned layout contains only executed
shard paths for halted cells and preserves their field-writer ownership.

Cryptographic checksum generation and verification are a separate layer,
described under [Integrity file](#integrity-file). Schema conformance
establishes the file shapes and cross-file count relationships; the presence of
`checksums.sha256` is not itself evidence that its digests still match.

## Source identity of an emission

A dataset records the executable SHA-256, the linked source-closure revision and
dirty state, and the repository-wide revision as run-start context.
`gf2_sim::permanent_campaign::provenance::approve_emission` approves a write
when the running executable digest matches `provenance.binary_sha256` in the
committed frozen manifest. A missing executable digest, a manifest that differs
from its committed content, or a root that is not exactly one campaign id below
the dataset home refuses the emission.

The source closure is `crates/` plus `Cargo.lock`. Repository-wide edits and
untracked paths outside that closure do not affect emission approval. The
manifest remains frozen because it declares the identity under which the
numbers are published.

The guard accepts only one campaign's own directory as the root it is emitting
into: exactly one campaign id below this home, inside the repository. Being
somewhere in the repository is not enough, because the root names the frozen
manifest the guard verifies against — a root at the repository itself, or at
this home, names no single manifest and leaves the guard nothing to check.
A root that is an ancestor of this home, the home itself, deeper than one
campaign id below it, elsewhere in the tree, or named by something that is not
a campaign id refuses, and the refusal says which of those it was.

## Integrity file

`checksums.sha256` uses the coreutils check-file format, one line per covered
file:

```text
<64 lowercase hexadecimal digits><space><space><path>
```

The path is relative to the campaign directory, uses forward slashes, and
contains no `.` or `..` component, so a reader verifies a dataset with standard
tooling and without a checkout of this repository:

```console
$ cd dev/simulation_results/permanent-zero-fraction/<campaign-id>
$ sha256sum -c checksums.sha256
```

The reader in this repository also accepts the equivalent binary-mode
separator, a space followed by `*`, which coreutils writes for `sha256sum -b`
and which produces identical digests. Entries are sorted by path, so
regenerating an unchanged dataset reproduces the file byte for byte.

Coverage is exactly the raw-data rows of the layout table above: the root
manifest, every executed shard record, every field summary, and the pooled
summary. Shard paths a halted cell never executed are absent from the dataset
and from this file. Derived artefacts are excluded, and the file does not cover
itself.

The `checksum-mismatch` fixture keeps a correct `manifest.json` checksum because
`manifest_fault` short-circuits verification; a deliberately wrong entry would
mask the three raw-file mismatches that fixture exercises.

The root manifest has two distinct identities. A field checkpoint's
`configuration.manifest_content_hash` — also rendered by `campaign_config_hash`
as the `blake3:<digest>` and `manifest=<digest>` components — is the driver's
execution identity: it parses `manifest.json` as a `CampaignManifest`, calls
`serde_json::to_vec(&manifest)` on that typed value, and hashes those
re-serialized bytes with BLAKE3. The `manifest.json` entry in
`checksums.sha256` is the on-disk identity: SHA-256 over the exact bytes stored
in that file. These values are not expected to match because their algorithms
and byte sources differ.

An auditor follows two comparisons from a field checkpoint to the frozen
manifest: first compare its recorded BLAKE3 value with
`blake3::hash(serde_json::to_vec(&manifest))`, where `manifest` is the typed
`CampaignManifest` parsed from the frozen `manifest.json`; then compare the
exact `manifest.json` bytes with the sidecar's `manifest.json` SHA-256 entry
(for example, with `sha256sum -c checksums.sha256`). The first comparison binds
the checkpoint to the driver's manifest identity, and the second binds that
typed manifest to the frozen on-disk bytes, so together they bind the
checkpoint to the frozen manifest.

## Verifying a published dataset

`gf2_sim::permanent_campaign::provenance::verify_dataset` re-checks a dataset
against its integrity file and reaches one of three verdicts. The manifest is
authenticated first, since it declares the layout on which every other check
depends.

| Verdict | Meaning |
| --- | --- |
| verified | Every covered path matched, coverage equals the raw set, and the recorded revision resolves |
| failed | The named paths are missing, changed, present but uncovered, or covered without being raw data |
| unverifiable | The recorded `git_revision` names no commit in the repository holding the dataset, or no repository could be resolved from it |

A missing file and a changed file are distinct outcomes rather than one failure
category. A dataset whose recorded revision cannot be resolved is reported as
unverifiable rather than as passing: its bytes may be intact, but the source
that produced them cannot be named.

The `permanent_dataset` binary exposes that check, and the guard and generator
beside it, from a shell:

```console
$ cargo run -p gf2-sim --release --bin permanent_dataset -- <subcommand> [campaign-directory]
```

| Subcommand | Does |
| --- | --- |
| `revision` | Prints the repository-wide revision observed at command start as provenance context |
| `emission-check <dir>` | Without an emitter path, verifies everything except writer identity, reports the manifest's pinned emitter digest, and states that writer identity is asserted only by the writer's own guard at emission time |
| `emission-check <dir> <emitter-path>` | Hashes the named emitter and runs the full emission guard, including writer identity, printing the approved executable digest or the refusal |
| `checksums <dir>` | Renders the integrity file for a finished dataset on standard output; it writes nothing, so redirect it into `checksums.sha256` |
| `conform <dir>` | Validates the complete schema and cross-document shard and summary aggregates without modifying the dataset |
| `verify <dir>` | Re-checks a dataset against its integrity file and its recorded source |

The `permanent_campaign` binary executes one field arm:

```console
$ cargo run -p gf2-sim --release --bin permanent_campaign -- \
    --manifest PATH --output CAMPAIGN-DIR --q FIELD [--workers N]
```

An accelerator-backed field also supplies
`--accelerator-cost-table dev/benchmarks/permanent_campaign/accelerator-launch-costs-v1.csv`.
That versioned production table contains one positive integer launch cost for
each and only each accelerator cell in the frozen manifest. Its
[receipt](../../benchmarks/permanent_campaign/accelerator-launch-costs-v1.md)
binds and recomputes every value from committed same-cell measurement evidence;
the fail-closed
[validator](../../benchmarks/permanent_campaign/accelerator_launch_costs_v1.py)
checks the bound inputs, arithmetic, rounding, and exact receipt/table
agreement. The binary rejects a missing, duplicate, malformed, nonpositive,
processor-backed, or unmanifested row before emission approval, drawing, or
output. These costs are launch-sizing evidence used to bound accelerator batch
sizes. They are neither backend-selection evidence nor scientific-result
evidence.

`--workers N` sets the positive worker count for the field arm and defaults to
`1` when omitted. One invocation executes exactly one field arm and writes only
that field's shard files and field summary, so field arms can run concurrently.
The first output line records the effective configuration as
`campaign q={field} workers={N}` before the per-shard timing lines. The writer
passes `approve_emission` before it runs the field, establishing source
identity before any dataset bytes are written. An interrupted field arm
resumes from its field-specific
`<campaign-directory>/campaign.q{field}.checkpoint.json`; checkpointed completed
shards are never re-evaluated or rewritten, and a shard emitted just before a
checkpoint update is adopted after a deterministic byte comparison.
A resume whose configuration differs is refused and names the differing
component. An evaluation failure is quarantined: its stable identity (`q`, `n`,
`shard_id`) and mechanical `error` diagnostic remain visible in the field
summary's `quarantined` diagnostic index while the remaining work continues.
The committed campaign execution receipt named by the dataset protocol is the
authoritative quarantine evidence, preserving bytes, logs, observed counts,
failure reason, and attempt numbers; the quarantined shard remains excluded
from the raw dataset and pooling. A completed field-arm rerun
resumes from the checkpoint's completed work set, re-evaluates nothing, rewrites
no shard or summary files, and exits successfully; the first emission is
preserved because completed shards are never rewritten.
Per-phase timings go to standard output and never into dataset files. Each
shard timing line reports `draw_s`, `pack_s`, `evaluate_s`,
`determinant_s`, and `count_s`; `determinant_s` is the measured companion
phase and is zero when the cell plan is `not_evaluated` or when a shard is
loaded or adopted rather than evaluated in the reporting process.

| Exit status | Means |
| --- | --- |
| `0` | The subcommand succeeded, and for `verify` the dataset verified |
| `1` | Emission was refused, the dataset failed, or the command errored |
| `2` | `verify` reached the unverifiable verdict: provenance is undecided |
| `64` | The command line was not one of the subcommand forms above |

`verify` therefore separates the three outcomes by exit status alone, without
parsing its output. The failing paths themselves are named on standard error.

## Deterministic analysis artefacts

`scripts/permanent_zero_fraction_analysis.py <campaign-directory>` consumes a
published dataset only after checking the manifest-declared raw checksum set
and invoking `permanent_dataset conform` from this checkout. The canonical
reader verifies the schema and all cross-document shard and summary aggregates
before the analysis process reads pooled counts or writes an artefact. It writes
derived CSV and SVG artefacts below
`<campaign-directory>/derived/<campaign-id>/zero-fraction-analysis/`; derived
artefacts are outside the raw checksum set by construction. The output table
retains every terminal cell. Completed cells carry their pooled count,
count-derived point estimate, $95\%$ Wilson interval, and recorded campaign
verdict; halted cells retain only counts and their mechanical reason.

The Wilson computation is a Python transliteration of the campaign's fixed
interval convention, using $z=1.959963984540054$, rather than a second
statistical policy. Its $q=3$ comparison columns read the versioned
[`scheinerman2024-q3-targets-v1.csv`](scheinerman2024-q3-targets-v1.csv)
source-count table beside this README; they recompute source points and source
intervals from those counts. For a completed row with a source comparison,
`precision_classification` is `prior_exact` when the source evidence is exact
enumeration. Otherwise, with count-derived standard errors
$s=\sqrt{\hat p(1-\hat p)/N}$, it is `exceeds_prior_precision` when
$s_{campaign}<0.9s_{prior}$, `matches_prior_precision` when
$0.9s_{prior}\le s_{campaign}\le1.1s_{prior}$, and
`below_prior_precision` when $s_{campaign}>1.1s_{prior}$. The field is blank
when no comparison applies. `interval_excludes_published` is `true` exactly
when the dataset Wilson interval excludes the count-derived prior point
estimate; it is `false` when that point is within the interval and blank when
no comparison applies. The SVG point metadata carries each completed point's
$N$, estimate, and interval, and all ordering, formatting, and SVG geometry
are fixed so an unchanged dataset re-runs byte-identically.
