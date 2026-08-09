# Permanent zero-fraction datasets

This directory is the permanent home for published campaign outputs that
measure permanent-zero fractions over small prime fields. Each child directory
is one immutable, versioned dataset:

`dev/simulation_results/permanent-zero-fraction/<campaign-id>/`

The campaign's controlling [scientific preregistration](protocol.md) is stored
beside those datasets; it is not part of any dataset's raw or derived file set.

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

The provenance record requires the full `git_revision`, `compiler_version`,
`rng_algorithm`, `rng_version`, tokenized `invocation`, `cpu_model`,
`accelerator_runtime`, and `gpu_model`. The revision is the complete
40-character lowercase hexadecimal object name; an abbreviation resolves only
against the repository that produced it, so it cannot identify the source of a
dataset read elsewhere. The RNG algorithm is the closed token
`chacha20`; `rng_version` records the exact crate or implementation version,
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
`schema_version`, `q`, and `rows`. A `SummaryRow` is shared by field summaries
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

A dataset is only as traceable as the build that wrote it, so an emitting
binary embeds the revision it was compiled from and checks that revision before
it writes. `gf2_sim::permanent_campaign::provenance::approve_emission` approves
a write only when the embedded revision equals the repository's `HEAD` and no
tracked file differs outside the campaign's own directory. A build that
recorded no revision — compiled outside a checkout, or on a host without `git` —
never emits.

The rule is deliberately narrower than "the working tree is clean". This
dataset lives inside the repository, so a clean-tree rule would refuse the
second shard of every campaign: the first shard already dirtied the tree. Files
under `<campaign-id>/`, raw and derived alike, are the campaign's expected
output and are exempt. `manifest.json` is the one exception inside that
directory, because it declares the identity the numbers are published under: a
tracked change to it refuses. A changed source file, build or dependency
manifest, `protocol.md`, or any other tracked file outside the campaign
directory refuses the emission, and the refusal names every path that differs.

Untracked paths never refuse. The source a binary was compiled from is fixed by
`HEAD`, and a file git does not track is not part of it.

The guard accepts only one campaign's own directory as the root it is emitting
into: exactly one campaign id below this home, inside the repository. Being
somewhere in the repository is not enough, because everything below that root
is exempt — a root at the repository itself would excuse the whole workspace,
and one at this home would excuse every campaign and the protocol beside them.
A root that is an ancestor of this home, the home itself, deeper than one
campaign id below it, elsewhere in the tree, or named by something that is not
a campaign id refuses, and the refusal says which of those it was.

A build follows `HEAD` only when asked to. By default the embedded revision is
fixed when `gf2-sim` is compiled and a later commit does not refresh it, so
landing a commit does not recompile the workspace's heaviest crate. The guard
then refuses with a revision mismatch until the crate is rebuilt, which is the
safe direction: a stale binary cannot publish under a source it was not built
from. A publisher exports `GF2_SIM_TRACK_HEAD=1` for the build that will emit,
and that build follows `HEAD` commit by commit. Setting or clearing the
variable re-runs the build script, so switching it on is itself enough to
refresh a stale binary.

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

The root manifest's content hash is consequently a sidecar value: it is this
file's `manifest.json` entry, and `manifest.json` stores no hash of itself. A
reader recomputes the hash from the manifest's bytes alone instead of having to
know it in advance, and an execution receipt that identifies a manifest quotes
the same value.

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
| `revision` | Prints the source revision this build embedded; it equals `git rev-parse HEAD` exactly when the binary is current with the checkout |
| `emission-check <dir>` | Runs the guard a campaign driver must pass before writing, printing the approved revision or naming every path that refuses it |
| `checksums <dir>` | Renders the integrity file for a finished dataset on standard output; it writes nothing, so redirect it into `checksums.sha256` |
| `verify <dir>` | Re-checks a dataset against its integrity file and its recorded source |

| Exit status | Means |
| --- | --- |
| `0` | The subcommand succeeded, and for `verify` the dataset verified |
| `1` | Emission was refused, the dataset failed, or the command errored |
| `2` | `verify` reached the unverifiable verdict: provenance is undecided |
| `64` | The command line was not one of the four forms above |

`verify` therefore separates the three outcomes by exit status alone, without
parsing its output. The failing paths themselves are named on standard error.
