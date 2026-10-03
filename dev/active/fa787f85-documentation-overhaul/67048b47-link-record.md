# Owned-artifact link record

Issue 67048b47 links manifest rows whose owner is established by identifier or
commit provenance to their owning issues. The generator is
`67048b47-doc-links.py`; the script it emits is `67048b47-doc-links.sh`.

## Selection rule

A (owner, path) pair is selected when the manifest row has evidence
`issue-id` or `commit`, lists the owner, and the artifact is a file the owner
does not yet link. The linked path is the row's `path` when it exists, else its
`destination` when that exists, else the target of the file's most recent git
rename. Terminal-state owners are linked. Every owner of a multi-owner row is
selected. Rows with evidence `doc-ref` already carry a reference and rows with
evidence `none` have no admissible evidence.

`jit doc add` records each reference with `--skip-scan` (an artifact reference
needs no asset scan, and binary artifacts are rejected by the scan). The
document type is derived from the file suffix: `tool` for source and scripts,
`log` for `.log`, `notes` for Markdown, `figure` for images, `presentation` for
HTML and `data` otherwise. The label is the file name.

## Commands

Run from the repository root.

```sh
python3 dev/active/fa787f85-documentation-overhaul/67048b47-doc-links.py > dev/active/fa787f85-documentation-overhaul/67048b47-doc-links.sh
bash dev/active/fa787f85-documentation-overhaul/67048b47-doc-links.sh
```

The script is idempotent: `jit doc add` updates an existing reference in place.
It stops at the first failure and prints each command before running it.

REQ-01 holds when the check lists nothing and exits 0:

```sh
python3 dev/active/fa787f85-documentation-overhaul/67048b47-doc-links.py --check
```

`67048b47-missing-before.txt` is that check's output before the script ran.

## Rows deliberately not linked

Each category is listed by a generator mode, one `<owner> <path>` (or `<path>`)
per line.

| Category | Command suffix | Reason |
|---|---|---|
| No admissible evidence | `--unassociated` | Evidence `none`; the row has no owner. |
| Artifact absent | `--absent` | The path, destination and rename history locate no existing artifact; these are archived directories whose files are linked at their renamed paths. |
| Owner missing | `--owner-missing` | The owner issue is absent from the tracker. |
| Directory | `--directory` | Tracker validation rejects a document reference to a directory; the files beneath carry their own rows. |

```sh
python3 dev/active/fa787f85-documentation-overhaul/67048b47-doc-links.py --directory
```
