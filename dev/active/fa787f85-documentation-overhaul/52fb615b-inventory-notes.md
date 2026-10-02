# Permanent-source inventory notes (52fb615b)

- Evidence: a tagged-commit history counts as provenance only when every commit touching the file carries one `jit:<id>`; no in-scope file qualifies, so rows without a `jit doc` reference or an id in the path carry evidence `none`.
- Deck bundles: `epic` and destination come from the `jit doc` reference of the deck or its recorded asset; `gruvbox.css` is unrecorded and takes `issue-id` from its deck directory name. The deck directory is the own directory of every bundle member.
- `generate_grand_comparison_plots.py` has no document reference; it travels with figures 4-6 under bundle head `docs/presentations/6efb756b-grand-sogrand/talk.html`.
- Destinations mirror the source path beneath the owning archive directory, so the decks' `../figures/` links keep their depth.
- In-place rewrites (root and crate READMEs, `proofs/README.md`) carry their own path as destination.
- `docs/lean4-verification-pipeline.md` reaches `e095a100` through task `886af072`, whose ancestors also include `bb85c68a`; the plan names `e095a100`.
- Root `AGENTS.md` and `CLAUDE.md` inbound lists name references outside `dev/` and `.agents/`; `README.md` inbound lists its three document references.
- Consumers of the root files are the literal entries of `.jit/config.toml`; the `crates/*/README.md` and `docs/**/*.md` globs pin no file.
- `table_interpretation.md` is digest-pinned by receipt snapshots; consumers are the `producing-inputs*.json` closure lists and inbound holds the receipt and log references.
- `crates/gf2-core/benches/*_results.md` inbound lists include the bench headers beside them, which the legacy move repoints.
- `CONTRIBUTING.md` is already retired and has no row.
