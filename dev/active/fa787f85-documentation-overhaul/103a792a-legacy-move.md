# gf2-core guide move to the legacy mirror (103a792a)

## Moved files

29 files, `git mv`, paths below `dev/archive/legacy/` repeat the
repository-relative path of each file.

| Rule | Files |
| --- | --- |
| Every file under `crates/gf2-core/docs/` | 26 (13 under `archive/`) |
| Every Markdown file directly under `crates/gf2-core/benches/` | 3 |

`crates/gf2-core/docs/` is absent. `crates/gf2-core/benches/` keeps its Rust
sources and `common/`.

## Inbound citations outside the mirror

Choice codes: P = permanent page, M = mirror path, R = removed provenance.
Every citation sits in a development document and no permanent page covers
the cited artifact, so each is M. Line numbers are those of the committed files.

| File:line | Choice | Target |
| --- | --- | --- |
| `dev/active/6dc81018-field-capability-dispatch/investigation.md:513,523` | M | `dev/archive/legacy/crates/gf2-core/{docs/GF2M.md,benches/strassen_threshold_results.md}` |
| `dev/active/7d7c647c/design.md:347` | M | `.../benches/strassen_threshold_results.md` |
| `dev/active/ae03bcd0-general-bch/bch-api-design.md:1044` | M | `.../docs/PRIMITIVE_POLYNOMIALS.md` |
| `dev/active/ae03bcd0-general-bch/investigation.md:264,369,370` | M | `.../docs/PRIMITIVE_POLYNOMIALS.md`, `.../docs/archive/GF2M_POLY_UTILITIES_REQUIREMENTS.md` |
| `dev/active/fa787f85-documentation-overhaul/12907582-drift-preaudit.md:158-161` | M | `.../docs/` files |
| `dev/active/fa787f85-documentation-overhaul/12907582-drift-record.md:10-14` | M | `.../benches/field_matrix_fusion_results.md`, `.../docs/` files |
| `dev/active/fa787f85-documentation-overhaul/60652fe4-roadmap-map.md:11` | M | `.../docs/POLY_UTILITIES_PERFORMANCE.md`, `.../benches` note |
| `dev/active/fa787f85-documentation-overhaul/dev-path-code-consumers.md:248` | M | `.../docs` |
| `dev/active/fa787f85-documentation-overhaul/e85b8edf-roadmap-map.md:27` | M | `.../docs/` file |
| `dev/active/fa787f85-documentation-overhaul/fa787f85-planning-brief.md:83` | M | `.../docs` |
| `dev/active/fa787f85-documentation-overhaul/investigation.md:68,114,138,166,206,264,277` | M | `.../docs/`, `.../benches/` notes |
| `dev/active/fa787f85-documentation-overhaul/perf-evidence-catalog.md:38,164,177,195-197,221` | M | `.../docs/`, `.../benches/` notes |
| `dev/active/fa787f85-documentation-overhaul/roadmap-premap.md:48,120,128` | M | `.../docs/` files |

Also repointed: `dev/active/eaae1b56/premeasurement-protocol.md:472`,
`dev/active/53c5a8c0/survey/source-evidence.json:215`,
`dev/active/ae03bcd0-general-bch/breakdown.json:1804` and
`dev/active/fa787f85-documentation-overhaul/52fb615b-inventory-notes.md:12`
(M). A search for the SHA-256 and the git blob id of each of the first three
over the tracked tree finds no occurrence, and no script or tool reads them.

Rust sources, crate READMEs and `docs/` hold no citation of a moved path.

Remaining hits of a moved path, outside the mirror and receipt `inputs/`
trees:

| File:line | Reason |
| --- | --- |
| `dev/active/a83583e0/premeasurement-protocol.md:1031` | the campaign driver hashes the file (`dev/tools/tuning-campaign-support/src/bin/tuning-extent-campaign-driver.rs:84-89`) and rejects a changed digest (`:682-684`); `dev/active/a83583e0/campaign-declaration.json:4` names the file |
| `dev/active/fa787f85-documentation-overhaul/breakdown.json:4396,4484,5089,5133` | lines 4396, 4484 and 5089 copy the descriptions of tracker issues, and 5133 is the footprint of issue 103a792a, whose contract names the pre-move location |
| `dev/active/fa787f85-documentation-overhaul/migration/manifest.toml` (rows 23-51 and `inbound` fields) | rows key on the `crates/gf2-core/` paths |
| `dev/active/fa787f85-documentation-overhaul/103a792a-legacy-move.md` | this record names the moved paths |

No tracker document reference names a moved file.

## Links inside moved files

15 local links resolve from the mirror location: 4 in `KERNEL_OPTIMIZATION.md` and `QUALITY_AUDIT_REPORT.md`
(`dev/benchmarks/tuning_profiles/`), 6 in `README.md` (crate README, examples,
audit pair under `archive/`), 5 in `archive/README.old.md`.

## Verification

| Command | Result |
| --- | --- |
| `python3 dev/active/fa787f85-documentation-overhaul/103a792a-link-scan.py` | 0 unresolved local links over the Markdown files the commits tagged `jit:103a792a` touch; output in `103a792a-link-scan.txt` |
| `python3 dev/active/fa787f85-documentation-overhaul/migration/check.py` | exit 0; the 29 rows are `complete` |
| `python3 contrib/gates/docs-mechanical.py` | PASS |

The scan covers inline links outside fenced code. It does not check anchors.
