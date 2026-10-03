You are dispatched to JIT issue {ID}. Your worktree is at:
  /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-{ID}
on branch worktree-agent-{ID}, anchored to main at {SHA}.

Hard rules for path discipline (worktree-dispatch-protocol):
- Run every shell command from your worktree root. Prefix tool calls with 'cd /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-{ID} && ...' if your shell state has drifted.
- Use only paths relative to the worktree root in your tool calls. Checkout-specific absolute paths are forbidden — they leak files into main's checkout.
- Never run 'git checkout', 'git switch', 'git worktree add/remove'.
- Commit on your worktree branch only. Do not push.
- Keep every build/dependency cache inside the worktree (it may be pre-seeded warm). Never place a cache on tmpfs or outside the worktree.

You are a technical writer on the documentation overhaul (epic fa787f85) of gf2, a research-grade Rust finite-field and coding-theory workspace. This task is one unit of a workspace-wide source-comment sweep (story ffc35b8c). Read first, in this order: `AGENTS.md` at the worktree root (the engineering contract; the invariants `non-obvious-comments`, `present-tense-prose`, `current-state-scope`, `teaching-rustdoc-examples`, `no-marketing`, `benchmark-backed-performance`, `external-claims-cited`, `single-source-prose`, `researcher-audience` are the review standard), the issue (`jit issue show {ID}`, read-only), and the story (`jit issue show ffc35b8c`). Resolve any `@/inv/<id>` with `jit item show @/inv/<id>`. Never write tracker state.

## Assignment

**Issue:** {ID} — {TITLE}

**Scope:** {SCOPE}

Edit ONLY files in this scope. Other workers edit the other files of the same crate concurrently.

### Success criteria (verbatim from the issue)

- [hard] REQ-01: Each comment and Rustdoc block in scope meets the root `AGENTS.md` comment rule: nothing restates adjacent code, and each module-level doc is at most one short orientation paragraph plus the contract statements its public API needs.
- [hard] REQ-02: No comment in scope narrates history or process, or states planned, deferred, excluded or out-of-scope work; a current limitation is restated in present tense, and each removed planned-work statement is matched to or filed as a tracked issue named in the commit message.
- [hard] REQ-03: Each line in scope matching the sweep pattern below is removed or listed with a one-line justification in the commit message.

  ```text
  non-goal|out of scope|not in scope|future work|deferred to|follow-on|phase [a-e]\b|wave [a-z0-9]|(task|issue|story) `?[0-9a-f]{8}|previously|no longer|legacy|migrat
  ```

- [hard] REQ-04: Public API Rustdoc still states purpose, panics, safety conditions and non-obvious complexity; Rustdoc builds without new warnings and the Rust CI gate passes.
- [hard] REQ-05: The diff contains only comment, Rustdoc and whitespace edits, verified by an ignore-whitespace review of every hunk.

(If `jit issue show {ID}` prints criteria that differ from these, the issue text governs; say so in your return.)

Gates the lead runs after merge: cargo-ci, code-review, doc-review, docs-mechanical. Reviewers read every hunk against the invariants above.

## Method

Work file by file through the whole scope; do not sample. For each comment and Rustdoc block decide: keep, shorten, rewrite in present tense, or delete.

Delete:
- Comments that restate the adjacent code, a signature, a type, or another doc block.
- History and process: commit, task, story, phase, wave, session, amendment, round or review designators; dates of decisions; "previously / now / no longer / was / used to / new / old"; legacy and migration language; design-history provenance, including `@/issue/<id>` and bare 8-hex tracker ids and `dev/plans/...`, `dev/active/...`, `dev/sessions/...` paths cited as where a design came from; references to `CLAUDE.md §...` or `AGENTS.md §...` sections.
- Planned, deferred, excluded, out-of-scope or "future" work. Where such a statement hides a current limitation, restate the limitation as a present-tense fact (a panic condition, a supported domain, an unsupported input). For each removed planned-work statement that names real unfinished work, search the tracker (`jit search "<terms>"`, `jit query all` is large; prefer search) for a matching open issue and name it in the commit message as "planned work: <statement in five words> -> <short-id>"; when none matches, list it in your return under Needs decision as "unmatched planned work" with file:line and the removed text, so the lead files it. Do not invent ids.
- Uncited performance, timing, speedup, throughput or feasibility statements ("~5 s", "10% faster", "≈ 600 years", "infeasible", Gop/s figures) and benchmark narration. A performance statement stays only as a pointer to a committed receipt that exists at HEAD; verify the path exists.
- Marketing adjectives and superlatives ("high-performance", "blazing", "state-of-the-art", "efficient" without a stated complexity).
- Rustdoc examples that teach nothing (accessors, constants, constructors, predicates, lookups, direct field mappings, uncompilable or never-run examples, elementary tutorials): remove the whole example block. Never edit code lines inside an example you keep (REQ-05).

Keep, in the shortest form:
- Contracts: purpose of a public item, panics, errors, safety conditions (`# Safety`, `SAFETY:` comments in full substance), invariants, preconditions of `_unchecked` paths, non-obvious complexity.
- The reason for a non-obvious choice, stated as a fact about the code ("row-major so that ...").
- Evidence pointers: citations of external standards and papers (as `@/citation/<key>` when the key exists in `.jit/references.toml`; do not invent keys, keep the existing prose citation otherwise and list it in your return), and pointers to committed receipts.
- Provenance headers of generated data tables, unchanged.
- Mathematical statements. Notation keeps the form of the surrounding block (LaTeX where the block already uses it).
- A module-level doc: one short orientation paragraph plus the contract statements the module's public API needs. Move nothing to other files.
- Non-obvious complexity of public items, one verified line each.

Hard limits:
- Comment, Rustdoc (`//`, `///`, `//!`, `/* */`, `#[doc = ...]` text) and whitespace edits only. No change to code, attributes, string literals, `#[ignore = "..."]` reasons, test names, or code inside doc-test fences. A `#[ignore]` reason or log string that carries narration is out of scope: list it in your return.
- NET-NEGATIVE LINE COUNT IS A HARD REQUIREMENT. The unit's `git diff --shortstat` must delete more lines than it adds. Every added line that is not a shortened replacement of removed text must be a missing panic, safety, error or contract statement required by REQ-04, and each such addition is listed in the commit message under "added contracts:" with file:line. Never add an explanatory comment, a section header, or a reworded restatement.
- Every line you write must be true of the code at HEAD: verify each rewritten claim against the code it describes (bounds, dispatch conditions, feature names, panics). A rewritten sentence that is false fails review harder than the narration it replaced. When you cannot verify a claim quickly, delete it rather than rephrase it.
- Do not "fix" what is merely terse and correct. Do not reflow untouched comments. Keep hunks minimal so the reviewer can read them.
- Intra-doc links must still resolve; do not remove a doc line that another item links to by anchor without fixing the link (comment-only).

## Verification before commit

1. Pattern: this command must print only lines you justify (one line each) in the commit message; aim for none:
   `rg -n -i 'non-goal|out of scope|not in scope|future work|deferred to|follow-on|phase [a-e]\b|wave [a-z0-9]|(task|issue|story) `?[0-9a-f]{8}|previously|no longer|legacy|migrat' {RG_PATHS} | rg '^\S+:\d+:\s*(//|/\*|\*)'`
   (A match where the word is domain vocabulary, e.g. "legacy" as part of a standard's term or "migrat" inside an identifier, is justified as such.)
2. Also sweep for what the pattern misses: `rg -n -i '@/issue/|\b[0-9a-f]{8}\b.*(task|issue|story|epic)|CLAUDE\.md|AGENTS\.md §|dev/(plans|sessions|active)/|TODO|FIXME|XXX|for now|currently|temporar|will be|planned|in a future|upcoming|soon|not yet|placeholder|session \d|round \d|amendment|high-performance|state-of-the-art' {RG_PATHS}` and resolve every comment hit (delete, restate as a present fact, or list with a reason in your return). `TODO`/`FIXME` without a tracked issue is a defect: match it to an issue or list it under Needs decision.
3. Comment-only proof (REQ-05): `git diff -w --stat` and a mechanical check that no non-comment token changed. Use this check and include its result: strip comments from the old and new version of each changed file and compare. A sufficient method: for each changed file, `git show HEAD:<file>` and the working file both pass through `./scripts/cargo-budget.sh cargo fmt -- --check` unchanged in code, AND the crate's compiled artifacts are equivalent, shown by: `./scripts/cargo-budget.sh cargo check -p {CRATE} --all-targets --all-features` succeeding plus a token-level comparison with a small script that removes `//...` line comments and `/* */` block comments outside string literals and diffs the remainder ignoring whitespace. Write that script once at `target/strip-comments.py` (git-ignored, do not commit it) and report "token diff empty" or the first differing line. If the token diff is not empty, you changed code: revert that hunk.
4. `./scripts/cargo-budget.sh cargo fmt --all -- --check`.
5. `./scripts/cargo-budget.sh cargo doc -p {CRATE} --all-features --no-deps` : no new warnings compared with the count at your anchor commit (measure the baseline first, before editing). {DOC_NOTE}
6. Doc tests compile (you did not edit code in them, but removed lines can break hidden setup): `./scripts/cargo-budget.sh --test cargo test --doc -p {CRATE} --all-features --profile ci-test` restricted with a module filter where the crate is large; report the command used. Do not run ignored tests. Do not run the full workspace suite; the lead runs the CI gate.
7. If the seeded `target/` gives stale absolute-path errors, run `./scripts/cargo-budget.sh cargo clean -p {CRATE}`, remove `target/doc`, and set `CARGO_CI_NO_SCCACHE=1`. Check `df -h /` before building; if less than 30 GB is free, stop and report.

## Commit

Commit in several commits if the scope is large (one per directory or file group is fine), so a crash loses little. Each commit subject: `docs(jit:{ID}): <summary>`, whole subject at most 71 characters (count it). The LAST commit's message body carries, in this order: `git diff --shortstat <anchor>..HEAD` totals for the unit; "justified pattern lines:" (file:line — reason, or "none"); "planned work:" (`<statement> -> <short-id> (jit issue show <short-id>)`, or "none"); "removed examples:" (item names, or "none"); "added contracts:" (file:line, or "none"); "token diff: empty". Mention no other issue id in the message except those matched under "planned work:". Every message ends with `Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>`. Never commit `__pycache__`, build output or scratch scripts. Never transition issue state, pass gates, or write `.jit/`.

If a criterion cannot be met literally, or you find a comment that documents behavior the code does not have (a real defect, not a comment problem), report it under Needs decision with file:line instead of documenting around it.


## Lessons from the first two sweep waves (binding)

- Prefix every cargo invocation with `CARGO_CI_NO_SCCACHE=1`. Never run `./scripts/cargo-ci.sh` or the workspace test suite; other workers and the lead's gates share the host.
- When a comment describes code that does not exist or states a wrong value, delete or correct it against the code and list it in your return under Deviations; when the CODE looks wrong (an unchecked overflow, an ignored parameter, an unsound check, a vacuous test), do not document around it and add no contract lines for it: report it under Needs decision with file:line so the lead files a defect.
- A removed "planned work" statement that is only conditional speculation needs no issue; say so in your return. One that names real unfinished work needs a tracked issue id in the commit body: search the tracker first (`jit search "<terms>"`), list unmatched ones in your return with the removed text, and the lead files them and asks you to amend the message.
- Narration inside string literals, test names and `#[ignore]` reasons is out of scope; list file:line in your return (tracked by an existing issue).
- A doc that only says an item delegates to, forwards to or wraps another call restates code: replace it with the item's semantic purpose in one line, or delete it when the item is private and its name says it. A comment directly above a loop or iterator chain that paraphrases it is deleted.
- A numerical-equivalence, bit-exactness or determinism claim in Rustdoc names the test that validates it (verify the test asserts exactly that) or is deleted. A claim wider than the test (for example "independent of worker count" when the test uses one worker) is narrowed to what the test shows.
- A statement that work "belongs to", "is tracked by" or "is deferred to" an issue is planned work in source: delete it and name the issue in the commit body instead. No `@/issue/` address stays in a comment.
- No account of how the code was written ("implemented from", "clean-room", "no source consulted", "ported from"); a pure attribution through a citation key stays.
- You may split the scope across parallel helper passes by file group, but you personally review every hunk and run every check once on the combined result.

The review standard below is part of this brief. Where it and the text above differ, the standard wins.

# Sweep review standard, version 2 (binding; supersedes the brief where they differ)

The independent reviewers judged the first five units and failed all of them. Every rule below is taken from a blocking finding. They judge the WHOLE scope against the criteria, not only the lines you changed: a pre-existing violation left in a file your unit owns fails REQ-01. Work through each rule over the entire scope.

## 1. Navigational comments: delete all of them

Delete every comment whose only job is to name, number, separate or inventory the code or tests that follow, whether you wrote it or it was already there: section banners and separators (`// ----`, `// ====`, `// ── Tests ──`, `// --- Construction ---`, `// Section 2 — ...`), test-group labels ("Property tests", "Inherent wrappers agree with the trait methods"), test inventories and indexes, and comments that narrate the next assertion or restate a test's name or purpose ("check that add is commutative" above `assert_eq!(a + b, b + a)`). Reviewer wording: "recurring navigational and assertion-narrating test comments that restate immediately visible test structure or code".

Keep a comment in tests only when it states something the test code does not show: why a constant has its value, which boundary case a literal encodes, a mathematical fact the assertion relies on.

## 2. Rustdoc examples: remove the non-teaching ones

`@/inv/teaching-rustdoc-examples`: "A rustdoc example teaches a workflow or clarifies a material contract that prose and focused tests leave unclear; examples for accessors, constants, constructors, predicates, and direct field mappings are a defect."

Removing a whole Rustdoc example block (the `# Examples` heading and its fenced code) is a Rustdoc edit and is IN scope; the earlier instruction not to remove examples is withdrawn. For every Rustdoc example in scope decide:
- Remove: examples for accessors, getters, constants, constructors (`zeros`, `new`, `from_*` followed by a `len`/`is_empty`/`all_zero` assertion), predicates, direct lookups and table accessors, direct representation or field mappings (`raw_words`), examples that only call the function and assert its obvious result, `ignore`d examples that cannot compile, `no_run` examples that can never execute, and elementary tutorials (line-by-line arithmetic in a small field, "what is a finite field" walk-throughs; `@/inv/researcher-audience`).
- Keep: an example that shows a multi-step workflow a researcher would otherwise have to assemble from several items, or that pins down a contract prose leaves ambiguous (an ordering convention, a layout, an error path).
- Never edit code lines inside an example you keep. When you remove one, remove the whole block; do not leave a dangling heading.
After removals, the doc tests of the crate must still pass, and the token-diff check must treat doc-comment lines as comments (it already does).
List removed examples by item name in the commit body under "removed examples:".

## 3. Module-level docs

Each `//!` module doc is ONE short orientation paragraph plus the contract statements the module's public API needs. Reviewer wording: "multi-paragraph explanations that restate nearby constants and generated-table structure, rather than one short orientation paragraph. The generated-data exception preserves provenance headers, not these module orientations." Cut algorithm walk-throughs, inventories of items, layout tables that restate constants, tutorials and "when to use" guidance. A statement that is a contract of one item moves nowhere: it is deleted here if that item's own doc already states it. A module doc may keep additional short sections only where each is a contract no single item owns (a module-wide safety contract, a shared layout or ordering convention, a determinism contract).

## 4. Non-obvious complexity is a contract: keep it

REQ-04 keeps "purpose, panics, safety conditions and non-obvious complexity". Reviewer wording: "The sweep removed non-obvious complexity contracts from public APIs ... Their cost depends on set-bit count, supplied multiplication implementation, field degree, or batch length and SIMD availability". For every public item whose cost is not obvious from its signature, the doc states the complexity in one line, verified against the code. If your unit removed such a line, restore it in shortened form. Do not add complexity lines for trivially O(1) or obviously linear items.

## 5. Performance statements

A source comment carries no performance, speedup, throughput, timing or feasibility statement, with or without a `dev/` path. Reviewer wording: "The new unconditional batch-performance claim cites a mutable development-tree projection rather than an identity-based receipt/protocol, and omits the receipt's host/build/path scope." Delete such statements, including ones your unit shortened and kept. Measured results live in `docs/reference/performance-evidence.md`. The one exception: a tuning constant's doc may say that its value comes from a measurement and name the committed receipt by a path that exists at HEAD, without quoting figures; delete even that if in doubt.

Also delete remaining `dev/active/...`, `dev/plans/...`, `dev/sessions/...` pointers used as design provenance. A pointer to a committed, digest-pinned protocol or preregistration that the code implements stays, as one line.

## 6. External works: cite by registry key

`@/inv/external-claims-cited`: "a work referenced only by prose title is a staleness defect". Reviewer wording: "retains an external Seroussi claim without a resolving citation key".

The citation registry is `.jit/references.toml` on main, and a mapping of cited works to keys is `dev/active/fa787f85-documentation-overhaul/554c2935-citation-map.md` with `554c2935-citation-map.txt` (both on main; if your worktree's anchor predates them, read them through the one allowed read-only relative path `../../../dev/active/fa787f85-documentation-overhaul/` and `../../../.jit/references.toml`). For every comment in scope that names an external paper, book, report, standard, sequence database entry or library as the source of a method, a constant or a claim: replace the prose citation with `@/citation/<Key>` in the form already used in the repository's Rustdoc (the key in a code span, e.g. `` `@/citation/Seroussi1998` ``), keeping a section, algorithm or table number as plain text after it. Use only keys that exist in the registry on main. If a cited work has no key, do NOT invent one and do NOT keep the bare prose claim on a line you touched: list the work with file:line in your return under Needs decision. For standards, use the key of the edition the code's provenance names; if the mapping gives no key for that edition, list it.

## 7. Planned work, limits and wrong code

Unchanged from the brief, with one addition: when your commit body names tracked issues under "planned work:", write each id so that it can be looked up, in the form `<statement> -> <short-id> (jit issue show <short-id>)`. A reviewer failed a unit because it could not resolve bare short ids.

## 8. Files in scope that are not Rust

If your scope directory contains hand-written notes inside non-Rust text artefacts that sit beside the sources (for example `src/x86/asm/*.asm.txt` headers), the same rules apply to the hand-written note lines: delete history, process, planned-work and host narration; keep a one-line statement of what the artefact is. Do not touch generated content (assembly listings, data tables) or provenance headers of generated data.

## 9. Self-review before you commit (do it; the reviewers will)

Run these over your scope and resolve every hit or justify it in the commit body:
- labels: `rg -n '^\s*//+!?\s*([-─═=*#~]{2,}.*|Section [0-9].*|(Unit |Property |Integration |Regression )?[Tt]ests?( for .*)?:?|Helpers?:?|Constants?:?|Imports?:?)\s*$' <scope>` and then read every remaining comment line of at most six words;
- examples: `rg -n '# Examples?' <scope>` and judge each against rule 2;
- module docs: for each file, count the `//!` lines and re-read the block against rule 3;
- performance: `rg -n -i 'faster|slower|speed.?up|throughput|\bGop|ops/s|latency|\b[0-9.]+ ?(ms|µs|us|ns|x|×|%)\b|benchmark|measured|dev/(bench_results|benchmarks|active|plans|sessions)' <scope>` on comment lines;
- citations: `rg -n -i 'et al|[A-Z][a-z]+ (and|&) [A-Z][a-z]+|\((19|20)[0-9]{2}\)|ePrint|arXiv|HPL-|OEIS|TS 38\.|ETSI|§ ?[0-9]|Alg(orithm)?\.? [0-9]|Hacker|Knuth|Handbook' <scope>` on comment lines, each resolved to a key or listed;
- the two sweep patterns of the brief; the token diff; `cargo fmt --check`; `cargo doc` warning count; doc tests.

The net line count stays negative. Report in your return, per rule 1-6, the number of items changed and anything you deliberately kept with a one-line reason.

## 10. Citation details (the registry and mapping are on main)

- Section 1 of `554c2935-citation-map.txt` lists `key | regex | files:lines`. Use it as the index for your scope, then read each hit: the regexes are broad (for example a bare "paper"), so confirm that the comment really cites that work before replacing.
- Eponyms are not citations: a method named after a person with no year, title, section or report number (Barrett reduction, Montgomery form, Karatsuba, Strassen, Wilson interval, Clopper-Pearson, Keller-Gehrig) stays as plain text and needs no key.
- Works the registry worker could not identify have no key. Do not delete them and do not reword them: leave those lines byte-identical and list them in your return. Known cases: "Condo et al. (2022), Fixed Complexity Soft-Output GRAND" (`grand/`), the Knuth/MMIX LCG constant, "Shoup §12.4" (`charpoly.rs`), TS 38.214 without an edition, "IEEE AES standard" (`primitive_polys.rs`), "NASA/CCSDS K=7 standard", and unnamed "the paper" references in `fading.rs` and `ldpc_bler_check.rs`.
- Known wrong citation data in source; correct it when it is in your scope, using the registry entry as the authority: `field/ple.rs` cites arXiv:1703.02438 where the work is arXiv:1601.01798 (`DumasPernetSultan2017`), and gives a wrong title and year for Dumas-Pernet (`DumasPernet2012`); `grand/mod.rs` gives ISIT as the venue of Solomon et al., which is ICC 2020 (`Solomon2020`); `dvb_t2_awgn_campaign.rs` writes "TR 102 831" for ETSI TS 102 831 (`Etsi2012`). With a key in place, drop the duplicated bibliographic prose (title, venue, year): the registry owns it (`@/inv/single-source-prose`).
- TS 38.212 maps to `ThreeGpp2017` except clause 5.4.2.2, which maps to `ThreeGpp2020`.


## Return

Your final message is the dispatcher's only input; write it for the next action.
Use these sections in order and write `None.` for an empty one:

1. **Outcome** — `done`, `partial`, or `blocked`, then the deliverables your
   assignment names (artifact paths, commit SHAs, counts, verdict).
2. **Needs decision** — each choice, escalation, or conflict the dispatcher must
   resolve, with the options and your recommendation.
3. **Deviations** — each departure from the assignment, each unmet criterion, and
   each failed or skipped check with its command and first failing line.

Omit passing checks, step narration, restated instructions, and anything the
written artifact or commit already records. Stay within 300 words unless a
deviation needs more evidence.

Outcome deliverables: per rule 1-6 of the review standard the number of items changed, examples kept with a one-line reason each, unkeyed citations left untouched (file:line), commit SHAs, the unit's `--shortstat` (insertions and deletions), remaining pattern-line count with justifications, the token-diff result, and the rustdoc warning count before and after.
