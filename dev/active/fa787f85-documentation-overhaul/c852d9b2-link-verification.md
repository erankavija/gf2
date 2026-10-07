# Link verification (`@/issue/c852d9b2`)

Commit verified: `64b26f58f561d77abab39b8f7a0f90b408805ac4`. The scripts
read tracked files of the working tree and take no commit argument.

## Commands

Run from the repository root of a checkout of the commit verified.

```sh
python3 contrib/gates/docs-mechanical.py
grep -c '^| `' contrib/gates/docs-mechanical-baseline.md
for f in $(grep -o '^| `[^`:]*' contrib/gates/docs-mechanical-baseline.md | cut -c4- | sort -u); do
  echo "$(git grep -l "$(sha256sum "$f" | cut -d' ' -f1)" | wc -l) $f"
done
python3 dev/active/fa787f85-documentation-overhaul/c852d9b2-archive-links.py | diff - dev/active/fa787f85-documentation-overhaul/c852d9b2-archive-links.txt
r=$(git rev-parse --show-toplevel); (cd /tmp && python3 "$r/dev/active/fa787f85-documentation-overhaul/c852d9b2-archive-links.py" | diff - "$r/dev/active/fa787f85-documentation-overhaul/c852d9b2-archive-links.txt")
for e in $(grep -o '^| [0-9a-f]\{8\} ' dev/active/fa787f85-documentation-overhaul/archive-rerun-results.md | cut -c3-10 | sort -u); do
  grep -c "^container .*/$e-" dev/active/fa787f85-documentation-overhaul/c852d9b2-archive-links.txt
done
jit doc check-links --scope all --json | python3 dev/active/fa787f85-documentation-overhaul/c852d9b2-tracker-errors.py | diff - dev/active/fa787f85-documentation-overhaul/c852d9b2-tracker-errors.txt
python3 dev/active/fa787f85-documentation-overhaul/migration/check.py
python3 contrib/gates/tests/docs-mechanical-test.py
```

## REQ-01: mechanical documentation check

| Command | Result | Establishes | Does not establish |
|---|---|---|---|
| `python3 contrib/gates/docs-mechanical.py` | `PASS`, `0 finding(s)`; the baselined count equals the row count the `grep -c` command prints | each link, anchor, citation, item address and projection of the `[docs-mechanical]` footprint resolves or is a row of [the baseline](../../../contrib/gates/docs-mechanical-baseline.md), and each baseline row suppresses one finding | anything under the `exclude` prefix, which holds the archive root |
| the `sha256sum` loop | a count of one or more for each file except `coverage-addressability.md` | a tracked file holds the SHA-256 of each baseline file whose rows are links | which consumer verifies the digest |

The requirement of no unresolved reference holds over the footprint with the
baseline rows admitted; DEC-01 of `@/issue/c852d9b2` is their ground.

| Baseline table | Rows | Class of DEC-01 |
|---|---|---|
| first | the address quoted in `coverage-addressability.md` as an example that does not resolve | quoted example address |
| second to fifth | `broken-link` rows of addenda, protocols, comparator specifications and designs whose SHA-256 a tracked file holds | unresolved link of a byte-fixed document |

The third class of DEC-01, the `canonical-abstraction` invariant address
quoted in the archived `handoff-4.md` of the 6dc81018 container, lies under
the `exclude` prefix. The check reads no file there and the baseline holds no
row for it.

## REQ-02 and REQ-03: archive link scan

[`c852d9b2-archive-links.py`](c852d9b2-archive-links.py) prints
[`c852d9b2-archive-links.txt`](c852d9b2-archive-links.txt), which states per
root the files scanned, the internal links checked, the unresolved links and
the scheme-qualified targets.

| Root kind | Found by | Files read | Criterion |
|---|---|---|---|
| `container` | a tracked `.jit-container` marker under `archive_root` of `[documentation]` | tracked `*.md` under the root | REQ-02 |
| `legacy` | the directory `legacy` under `archive_root` | tracked `*.md` under the root | REQ-03 |
| `deck` | a directory under `archive_root` holding a tracked `*.html` and a tracked `*.css` | tracked `*.html` and `*.css` under the root, each assigned to its deepest deck root | REQ-03 |

| Command | Result | Establishes | Does not establish |
|---|---|---|---|
| the two `diff` commands | print nothing, from the root and from `/tmp` | the committed output is the output at the commit verified | — |
| last line of the output | `unresolved internal links: 0` | each internal link the scan reads in each root resolves to a tracked path; each `#fragment` of a Markdown target matches a heading or `<a>` anchor of the target; each `#L<n>` fragment lies within the target's line count | see the list below |
| the `archive-rerun-results.md` loop | prints `1` per epic | each container of [`archive-rerun-results.md`](archive-rerun-results.md) is one `container` root of the output | — |

The Markdown grammar is that of `contrib/gates/docs-mechanical.py`: inline
links, images and reference definitions outside fenced code and code spans.
For decks the script reads `href`, `src` and `url(` of HTML and `url(` of CSS.

The scan does not establish:

- reachability of a target with a scheme or a leading `//`; the output counts
  them per scheme;
- fragment-only targets of HTML files; the output counts them per deck;
- fragments of targets other than Markdown files;
- links in fenced code, in code spans, in files other than `*.md`, `*.html`
  and `*.css`, or built by a deck's scripts at run time;
- item addresses in the archives;
- links of a commit-pinned revision of an archived document.

## `jit doc check-links --scope all`

The command exits 1.
[`c852d9b2-tracker-errors.py`](c852d9b2-tracker-errors.py) groups its distinct
errors by type, area and file suffix in
[`c852d9b2-tracker-errors.txt`](c852d9b2-tracker-errors.txt), the output on
the primary checkout. The output follows tracker state, and its
`unreachable_url` rows depend on network reachability from the host. `linked` counts the errors whose target is a link of the tracked
file; `unresolved` counts those of them whose path is not tracked.

| Type | Area | Suffix | Read by | Finding |
|---|---|---|---|---|
| `unreachable_url` | archive | `html` | archive scan, deck roots | external font hosts; counted under `https:`, not fetched |
| `broken_link`, `missing_asset` | footprint | `md` | mechanical check | each `unresolved` link is a baseline row; each other error names a target that is not a link of the tracked file or that resolves |
| `broken_link`, `missing_asset` | other | `md` | neither scan | the files lie under `dev/studies` and `dev/bench_results`, outside the footprint and outside the archive root |
| `broken_link`, `missing_asset` | other | other suffixes | neither scan | the documents are not Markdown, HTML or CSS and lie outside the archive root |

## Repairs

| Root | File | Change |
|---|---|---|
| `legacy` | `dev/archive/legacy/crates/gf2-core/benches/field_matrix_fusion_results.md` | the target of reference definition `[alloc-test]` names the tracked `crates/gf2-core/src/field/expr.rs` |
| — | `contrib/gates/docs-mechanical-baseline.md` | holds no row under the `exclude` prefix |

## Other checks

| Command | Result | Establishes |
|---|---|---|
| `python3 dev/active/fa787f85-documentation-overhaul/migration/check.py` | exit 0 | the manifest assertions hold |
| `python3 contrib/gates/tests/docs-mechanical-test.py` | `OK` | the check's self-tests |
