# Stray and ownerless dev entries (0a0ece44)

`L` is `dev/archive/legacy`. `M` is `migration/manifest.toml`; its rows key on
the source path.

## Entries

| Entry | Files at the committed state | Collected by | `M` rows |
| --- | --- | --- | --- |
| `dev/active/37e0b235/` | `dev/archive/806eb14e-hip-gpu-prototype/active/37e0b235/gpu-batch-ldpc-bp-plan.md` | `398ca9e0b` (issue 1ca94ec2) | `jit-container-archive`, `complete` |
| `dev/active/babcf05e-gf2-core-ppc-spiral/` | `dev/archive/babcf05e-gf2-core-ppc-spiral/active/babcf05e-handoff-5.md` | `aac49881e` (issue 3759f995) | `jit-container-archive`, `complete` |
| `dev/active/e095a100-gfpm-arithmetic/` | `dev/archive/e095a100-gfpm-arithmetic/active/e095a100-presentation/themes/gruvbox.css` | `398ca9e0b` (issue 1ca94ec2) | `jit-container-archive`, `complete` |
| `dev/active/DOCUMENTATION_AUDIT.md` | `L/dev/active/DOCUMENTATION_AUDIT.md` | commit tagged `jit:0a0ece44`, `git mv` | `legacy-archive`, `complete` |
| `dev/active/charon-patch-backup-2026-05-15/` | `L/dev/active/charon-patch-backup-2026-05-15/` | same | `legacy-archive`, `complete` |
| `dev/archive/packed_field_stub/` | `L/dev/archive/packed_field_stub/` | same | `legacy-archive`, `complete` |

Each of the three archive directories in the first three rows holds a
`.jit-container` marker. The tracked tree holds no file under the three source
directories.

## Byte identity of the collected strays (REQ-01)

For each of the first three rows, with `<commit>` and the paths of that row:

```sh
git show <commit>^:<source file> | sha256sum
sha256sum <archive file>
```

| Archive file | SHA-256 of both |
| --- | --- |
| `.../37e0b235/gpu-batch-ldpc-bp-plan.md` | `c2c7882ea9bba53b0d29f36b7b6b940c32232237405d608191d2fb4d3cf1a846` |
| `.../active/babcf05e-handoff-5.md` | `036d0cffac8fe40d7ab192048e4859cebec517089e3ea75e604aec1ef9c113e6` |
| `.../themes/gruvbox.css` | `b835feba6c82b645f3a4eb98bfc49622501b1b8f4b1a0d28bf4241bb5b147401` |

The comparison covers the last blob of each source path before its deleting
commit. It does not cover the files of the two epic directories that commit
`195f8254f` archived.

## The stub

| Fact | Evidence |
| --- | --- |
| No Cargo workspace, script, test or CI file reads the stub | `git grep -nF packed_field_stub` outside the stub finds development documents, tracker records and committed tool outputs only |
| Its manifest declares `gf2-core` by a path relative to the stub directory | `Cargo.toml` of the stub; the move rewrites the path text to the depth of the mirror location, and the path resolves to `crates/gf2-core` |
| Task 7f818151 of the archived epic 6dc81018 holds a tracker document reference to each of its files, without commit pin | `jit doc list 7f818151` |
| The `M` rows of the stub change `disposition` and `destination` besides `status` | REQ-02 of the issue names the mirror; epic 6dc81018 is archived |

The stub is not built at its mirror location by this unit; `cargo-ci` does not
build it either, since it is outside the workspace.

## Citations

Repointed, path text only, except the one restated sentence:

| File | Cites |
| --- | --- |
| `scripts/verify-lean.sh` (comment), `proofs/WORKAROUNDS.md` | the implied-clause patch of the Charon backup |
| `dev/active/86b9c719-quality-documentation-tech-debt/150d7d79/150d7d79-toolchain-upgrade.md` | the Charon backup directory and its rebased diff |
| `dev/active/6dc81018-field-capability-dispatch/investigation.md` | the stub source and one Charon backup file |
| `dev/active/6dc81018-field-capability-dispatch/classification.md` | the stub source |
| `dev/active/ae03bcd0-general-bch/investigation.md` | the audit |
| `investigation.md` (prior-art row and CONTRIBUTING consumers paragraph) | the audit |
| `27f0f3b1-inventory-notes.md` | the stub; the sentence states that epic 6dc81018 is archived |
| `dev/active/6dc81018-field-capability-dispatch/breakdown.json`, `breakdown.json` (JSON strings; both files parse) | the stub directory |

Hits of a pre-move path that keep their bytes:

| Location | Ground |
| --- | --- |
| `dev/archive/6dc81018-field-capability-dispatch/active/6dc81018-field-capability-dispatch/{investigation.md,classification.md,breakdown.json}` | Files of a marker-backed archive; code-span citations, no inline link |
| `dev/archive/ae82bd73-gf2-algebra-permanent/active/{0606186a/0606186a-impl-handoff.md,ae82bd73-handoff-13.md}`, `dev/archive/e095a100-gfpm-arithmetic/docs/lean4-verification-pipeline.md` | Same |
| `616e1d7c-preexec.json` | Committed tool output: the archive preview taken before execution commit `50de04a6b` |
| `67048b47-linked-pairs.txt` | Committed tool output of `67048b47-doc-links.py`, committed at `d5b3d8b4b` |
| `eb2a833d-link-scan.txt` | Committed tool output of `eb2a833d-link-scan.py`, committed at `1e92d5273` |
| `M`, `path`, `bundle` and `inbound` fields | The fields name the source by schema |
| This record | It names the moved paths |
| `.jit/issues/` descriptions and the document references of 7f818151 | Tracker state; `target/0a0ece44-relink.sh` repoints the references |

`investigation.md` Appendices A and B name the three entries by bare name in
census rows of the tree at `fca6df53d`, the commit its header states.
`036615b0-inventory-notes.md` names two of them by bare name with their commit
scopes. Neither file holds a pre-move path of the three entries.

`git grep -nF <pre-move path>` for each of the three paths, outside `.jit/`,
the marker-backed archives and `L`, finds the files of the table above only.

No moved file names its own directory by path literal (REQ-05): `rg` for the
three pre-move paths under the three mirror locations finds nothing.

## Verification

| Command | Result | Establishes | Does not establish |
| --- | --- | --- | --- |
| `python3 contrib/gates/docs-mechanical.py` | PASS | Every inline link of the touched Markdown files under `dev/active/` resolves | Links of files under `dev/archive/` and `proofs/` |
| `rg -n '\]\(' proofs/WORKAROUNDS.md dev/archive/legacy/dev/active/DOCUMENTATION_AUDIT.md dev/archive/legacy/dev/archive/packed_field_stub/README.md` | one line, in `proofs/WORKAROUNDS.md`; its target `docs/how-to/formal-verification.md` exists | The touched and moved Markdown files outside the gate's scope hold no unresolved inline link | Anchors, code-span paths, reference-style links |
| `python3 dev/active/fa787f85-documentation-overhaul/migration/check.py` | no finding names a row of this unit | Each row of the six entries is `complete`, its destination exists and its source is absent | Rows of other units |
