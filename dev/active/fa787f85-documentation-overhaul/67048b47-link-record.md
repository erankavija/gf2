# Owned-artifact link record

Issue 67048b47 links manifest rows whose owner is established by identifier or
commit provenance to their owning issues. The generator is
`67048b47-doc-links.py`; the script it emits is `67048b47-doc-links.sh`.

## Selection rule

A (owner, path) pair is selected when the manifest row has evidence
`issue-id` or `commit`, lists the owner, and the artifact is a file the owner
does not yet link. Directory rows carry no reference; the files beneath them
have their own rows. The linked path is the row's `path` when it exists, else its
`destination` when that exists, else the target of the file's most recent git
rename. Terminal-state owners are linked. Every owner of a multi-owner row is
selected. Rows with evidence `doc-ref` already carry a reference and rows with
evidence `none` have no admissible evidence.

Binary files (`--binary` lists them) carry `--skip-scan` because the asset
scan rejects non-UTF-8 content. The script repeats a reference with
`--skip-scan` when `jit doc add` fails, which covers paths beyond the scan's
depth budget.
The document type is derived from the file suffix: `tool` for source and scripts,
`log` for `.log`, `notes` for Markdown, `figure` for images, `presentation` for
HTML and `data` otherwise. The label is the file name.

## Commands

Run from the repository root. The generator reads the manifest, the tracker and
the working tree of the checkout it runs in, and its output is sorted and
deterministic for a given state.

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

## Conformance findings

Tracker validation (`jit validate`) reports no new findings. New
`jit doc conformance` findings are limited to two kinds of selected file, each
listed by a generator mode:

- files with several owners: `--multi-owner`
- files outside their owner's canonical directory (`jit doc dir`): `--misplaced`

The findings on selected files are a subset of those two listings when this
prints nothing:

```sh
export LC_ALL=C
G=dev/active/fa787f85-documentation-overhaul/67048b47-doc-links.py
comm -23 <(comm -12 <(python3 $G --selected | sort -u) <(jit doc conformance | awk '/^  dev\//{print $1}' | sort -u)) <({ python3 $G --multi-owner; python3 $G --misplaced; } | sort -u)
```

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
