# Closure deck copy removal (@/issue/d0b1545e)

The archived deck of epic `@/issue/97bf0879` is the single copy of the
closure deck of `@/issue/f00fd873`; milestone `@/issue/b117c95e` links it.
Commit checked: `fb82f893f`, which
holds the removal and the manifest rows.

## Commands

```sh
git ls-tree -r fb82f893f^ dev/bench_results/b117c95e-gf2-core-perf | grep sota-closure-deck | awk '{n=split($4,a,"deck/"); print $3, a[2]}'
git ls-tree -r fb82f893f dev/archive/97bf0879-gf2-core-sota-performance/bench_results/2026-05-08-f00fd873-sota-closure-deck | awk '{n=split($4,a,"deck/"); print $3, a[2]}'
git ls-tree -r --name-only fb82f893f dev/bench_results/b117c95e-gf2-core-perf | grep -c sota-closure-deck
git ls-tree -r --name-only fb82f893f^ dev/bench_results/b117c95e-gf2-core-perf | grep sota-closure-deck | grep -v gruvbox.css | xargs -I{} git grep -n -e gruvbox fb82f893f^ -- {}
git ls-tree -r --name-only fb82f893f^ dev/bench_results/b117c95e-gf2-core-perf | grep gruvbox.css | xargs -I{} git grep -n -E 'url\(|@import|https?:' fb82f893f^ -- {}
git ls-tree -r --name-only fb82f893f^ dev/bench_results/b117c95e-gf2-core-perf | grep sota-closure-deck | grep -v gruvbox.css | xargs -I{} git grep -n -E 'href=|src=|url\(|@import' fb82f893f^ -- {}
git grep -n 'perf/2026-05-08-f00fd873-sota' fb82f893f -- . ':!.jit' ':!dev/active/fa787f85-documentation-overhaul/migration/manifest.toml'
git diff --name-status fb82f893f^ fb82f893f
jit doc list b117c95e
grep -l 'perf/2026-05-08-f00fd873-sota' .jit/issues/*.json
python3 dev/active/fa787f85-documentation-overhaul/migration/check.py
python3 -m unittest discover -s dev/active/fa787f85-documentation-overhaul/migration -p '*_test.py'
python3 contrib/gates/docs-mechanical.py
```

## Criteria

| Criterion | Command | Establishes | Does not establish |
| --- | --- | --- | --- |
| REQ-01 absence | 3 | Prints `0`: the commit tree holds no file of the copy | Absence in other commits |
| REQ-01 identity | 1, 2 | Blob ids of `base.css`, `talk.html` and `themes/rust.css` are equal in both listings | Identity of `themes/gruvbox.css`, which the archived deck lacks |
| REQ-01 `themes/gruvbox.css` | 4, 5, 6 | Command 4 prints nothing: no other file of the copy contains `gruvbox`. Command 5 prints nothing: the file has no `url(`, `@import` or `http` reference. Command 6 lists the `href` and `src` of `talk.html` and `url(` of the stylesheets: `themes/rust.css`, `base.css`, and remote URLs, no `themes/gruvbox.css` | Dynamic references built by script at run time |
| REQ-02 tracker | 9, 10 | Command 9 lists the archived `talk.html` for `@/issue/b117c95e` with type `presentation`; command 10 prints nothing. Run on the primary checkout, whose tracker holds the reference to the archived deck | Tracker state at other checkouts |
| REQ-03 citations | 7 | Prints nothing: outside tracker state and the manifest schema fields, no file at the commit names the copy path | Files that `contrib/gates/docs-mechanical.py` does not read; command 7 reads every tracked file |
| REQ-03 links | 8, 13 | Command 8 lists only deletions and `migration/manifest.toml`; the manifest carries no Markdown link. Command 13 reports no unresolved link in the Markdown footprint of `[docs-mechanical]`, which includes this record and excludes `dev/archive` | Links in `dev/archive` |
| REQ-04 manifest | 11, 12 | Four `deletion` rows with status `complete` for the removed files; the checker asserts `path` is absent for each; exit status 0 and self-tests pass | Rows for any other path |
| REQ-05 record | this file | States commit and commands | Output of commands run after the commit |

## Holders of the copy

| Holder | Disposition |
| --- | --- |
| Document reference of `@/issue/b117c95e` (no commit pin) | Names the archived `talk.html`, `presentation`, label `gf2-core SOTA closure deck (epic 97bf0879)` |
| Asset digests inside that reference | Follow the reference to the archived file; the archive holds the same bytes for `base.css` and `themes/rust.css` |
| Digest pins, code and script consumers | None names the copy path (command 7) |
| Manifest | One `deletion` row per removed file, status `complete` |
