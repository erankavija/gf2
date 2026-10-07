# Permanent surface audit (`@/issue/fca578b1`)

| Fact | Value |
|---|---|
| Measured commit | `999228b25`: every committed output below is the script's output at this commit |
| Line numbers in the findings | commit `6a38cc8b6`, the state the audit read |
| Prior-source base | `791beb2b0`, the parent of `61c3f0a6a`, the first commit of the overhaul that removes a documentation file |

## Surface

`fca578b1-scans.py surface` derives the surface from the Git index with
`git ls-files -- README.md docs 'crates/*/README.md' AGENTS.md CLAUDE.md 'crates/*/AGENTS.md' 'crates/*/src/lib.rs'`;
[`fca578b1-surface.txt`](fca578b1-surface.txt) is its output. A crate-root
`lib.rs` contributes its `//!` lines. `CLAUDE.md` is a symbolic link to
`AGENTS.md` and is read through it. The pathspec `crates/*/AGENTS.md` matches no
file.

## Invariants applied

`@/inv/researcher-audience`, `@/inv/adapted-diataxis-placement`,
`@/inv/current-state-scope`, `@/inv/present-tense-prose`, `@/inv/no-marketing`,
`@/inv/single-source-prose`, `@/inv/benchmark-backed-performance`,
`@/inv/claims-trace-to-artifacts`, `@/inv/uncertainty-reported`,
`@/inv/external-claims-cited`, `@/inv/teaching-rustdoc-examples`,
`@/inv/non-obvious-comments`, `@/inv/agents-md-scope`,
`@/inv/convention-convergence`. The registry `.jit/invariants.toml` holds no
other invariant whose subject is prose, Rustdoc, examples or page placement,
apart from `@/inv/active-document-layout`, which governs this record's
directory.

## Method

| Step | Establishes | Does not establish |
|---|---|---|
| Full reading of each surface file | The findings and dispositions below | Absence of a defect the reader missed |
| Claim verification against code, manifests, scripts, configuration and the cited receipts, by reading | Each finding's evidence | The claim classes under [Limits of the verification](#limits-of-the-verification) |
| Pattern scans (`fca578b1-scans.py`) | Every match of the stated patterns is fixed or listed with a reason | Absence of a defect its patterns do not spell |
| Word-run comparison (`fca578b1-verbatim.py`) | No page shares an unlisted run of $N$ consecutive prose words with a prior source | Absence of paraphrase or of shared runs shorter than $N$ |
| `python3 contrib/gates/docs-mechanical.py` | Links, anchors, citations and item addresses resolve | Truth of the linked content |

## Findings

Every row is fixed in the named commit.

| Page:line | Invariant | Finding | Fix |
|---|---|---|---|
| `AGENTS.md:9` | `@/inv/no-marketing` | "high-performance" in the mission | `81fbe6037` |
| `AGENTS.md:28` | `@/inv/benchmark-backed-performance` | An uncited percentage for the `ci-test` profile | `81fbe6037` |
| `AGENTS.md:118` | `@/inv/convention-convergence` | No statement of the issue reference form (REQ-05) | `81fbe6037` |
| `AGENTS.md:59` | `@/inv/current-state-scope` | `gf2-core` is stated to have no workspace dependency; `crates/gf2-core/Cargo.toml` depends on `gf2-kernels-simd` | `1ebd1248d` |
| `AGENTS.md:63` | `@/inv/convention-convergence` | Plain-text field notation in Markdown | `1ebd1248d` |
| `AGENTS.md:97` | `@/inv/current-state-scope` | The per-test kill is stated without the overrides `.config/nextest.toml` declares | `1ebd1248d` |
| `AGENTS.md:50` | `@/inv/current-state-scope` | The lock wrapper is stated to set `CARGO_CI_NO_LOCK=1`; its header makes that the caller's obligation | `a12757f45` |
| `AGENTS.md:100` | `@/inv/current-state-scope` | The command is attributed to the nightly run; `.github/workflows/ci-slow.yml` passes an explicit feature list | `1ebd1248d` |
| `README.md:28` | `@/inv/current-state-scope` | Kernels are stated as AVX2-only and feature-gated; `gf2m.rs` binds on PCLMULQDQ and SSE4.1, and `gf2-coding` detects BCH encode and demapper kernels in every build | `71a1eb545` |
| `docs/index.md:10`, `:39` | `@/inv/external-claims-cited` | Standards named without a registry key | `124392903` |
| `docs/index.md:34` | `@/inv/single-source-prose` | The section sentence copies the invariant's wording | `124392903` |
| `docs/concepts/acceleration-architecture.md:13` | `@/inv/current-state-scope` | "All `unsafe` code": test targets hold `unsafe impl GlobalAlloc` | `71e933b84` |
| `docs/concepts/acceleration-architecture.md:21`, `:93` | `@/inv/current-state-scope` | Every consumer is stated to apply `is_recoverable`; `gf2_algebra::gpu` and the GPU BCJR engine panic | `71e933b84` |
| `docs/concepts/acceleration-architecture.md:28`, `:31` | `@/inv/current-state-scope` | Universal detector statements; `modem` exports `detect_f32` and `detect_f64`, and `transpose::detect` returns a scalar bundle | `71e933b84` |
| `docs/concepts/acceleration-architecture.md:34`, `:37`, `:38` | `@/inv/current-state-scope` | Cache and accessor statements; the LLR bundle uses `once_cell`, the bipedal detectors cache in the kernel crate, the wide bundle has three accessors | `71e933b84` |
| `docs/concepts/acceleration-architecture.md:47` | `@/inv/current-state-scope` | "used only when the operand shape crosses a selector"; the LLR path has no selector | `71e933b84` |
| `docs/concepts/acceleration-architecture.md:141` | `@/inv/current-state-scope` | The resume check is attributed to tests without a resume case; `tests/determinism.rs` holds it | `71e933b84` |
| `docs/concepts/acceleration-architecture.md:31`, `:42` | `@/inv/convention-convergence` | Plain-text field notation | `124392903` |
| `docs/concepts/finite-field-arithmetic.md:83` | `@/inv/current-state-scope` | Batch kernel condition omits PCLMULQDQ and the `simd` feature; the scalar path is not Barrett (`gf2m/batch.rs`) | `71e933b84` |
| `docs/concepts/finite-field-arithmetic.md:181` | `@/inv/current-state-scope` | `trinomials(m)` is stated to list primitive trinomials; its degree-8 entry is reducible | `71e933b84` |
| `docs/how-to/formal-verification.md:4` | `@/inv/convention-convergence` | Citation addresses outside code spans | `124392903` |
| `docs/how-to/formal-verification.md:51` | `@/inv/current-state-scope` | `proofs/WORKAROUNDS.md` does not cover `fix-aeneas-sorrys.py` | `71e933b84` |
| `docs/how-to/reproduce-performance-evidence.md:3` | `@/inv/current-state-scope` | The receipt commit is equated with the built source; the worked example builds another revision | `71e933b84` |
| `docs/how-to/reproduce-performance-evidence.md:117` | `@/inv/current-state-scope` | The window requirement is attributed to the receipt's protocol; the wrapper imposes it | `71e933b84` |
| `docs/how-to/run-simulation-campaigns.md:6` | `@/inv/external-claims-cited` | Standard named without a registry key | `124392903` |
| `docs/how-to/run-simulation-campaigns.md:55`, `:62`, `:105` | `@/inv/convention-convergence` | Commands without the `cargo-budget.sh` prefix the other pages use | `124392903` |
| `docs/how-to/run-simulation-campaigns.md:88`, `:112` | `@/inv/current-state-scope` | SIGTERM is stated as handled; `ctrlc` is built without its `termination` feature | `71e933b84` |
| `docs/how-to/select-acceleration.md:50` | `@/inv/current-state-scope` | `matvec_route` is stated to report the dispatcher's arm; it reports the word-count comparison | `71e933b84` |
| `docs/how-to/select-acceleration.md:98` | `@/inv/current-state-scope` | "No committed receipt measures" the BCH encode families; a committed receipt times them | `71e933b84` |
| `docs/how-to/select-acceleration.md:137` | `@/inv/single-source-prose` | A copied value of `RAYON_NUM_THREADS` | `124392903` |
| `docs/how-to/select-acceleration.md:157` | `@/inv/external-claims-cited` | Standard named without a registry key | `124392903` |
| `docs/reference/performance-evidence.md:4` | `@/inv/claims-trace-to-artifacts` | The evidence commit is stated as the source identity of a claim whose Build line names another revision | `71e933b84` |
| `docs/reference/performance-evidence.md:7` | `@/inv/benchmark-backed-performance` | The introduction asserts that each receipt measures the shipped code path; no committed comparison establishes it (`@/issue/16fee10f`) | `999228b25` |
| `docs/reference/performance-evidence.md:76`, `:101`, `:122`, `:145` | `@/inv/benchmark-backed-performance` | The linked summaries omit host, build, workload, baseline and date; each receipt's `receipt.json` holds them | `a12757f45` |
| `docs/reference/performance-evidence.md:170` | `@/inv/benchmark-backed-performance` | "no RUSTFLAGS" without its source, the receipt's command | `a12757f45` |
| `docs/reference/performance-evidence.md:110` | `@/inv/claims-trace-to-artifacts` | The stated interval hull excludes two receipt bounds | `71e933b84` |
| `docs/reference/standards-conformance.md:20` | `@/inv/external-claims-cited` | Two section numbers without a source in the tree | `71e933b84` |
| `docs/reference/standards-conformance.md:51`, `:95` | `@/inv/current-state-scope` | Two test-tier universals with a counterexample each | `71e933b84` |
| `docs/reference/supported-configurations.md:11` | `@/inv/single-source-prose` | A tested ROCm version that neither cited source states | `124392903` |
| `docs/reference/supported-configurations.md:16` | `@/inv/current-state-scope` | "no operating-system-specific code": a `gf2-core` bench target is Unix-only | `71e933b84` |
| `docs/reference/supported-configurations.md:27`, `:31` | `@/inv/current-state-scope` | Same defect as `README.md:28` | `71e933b84` |
| `docs/reference/supported-configurations.md:44` | `@/inv/current-state-scope` | `DeviceUnavailable` is attributed to `gf2-sim`; the scheduler degrades to the CPU path | `71e933b84` |
| `docs/tutorials/coded-modulation-link-simulation.md:87` | `@/inv/current-state-scope` | `ber` is called a floating-point reduction; it is one integer ratio | `71e933b84` |
| `docs/tutorials/coded-modulation-link-simulation.md:119` | `@/inv/single-source-prose` | The checkpoint hash coverage restates the how-to | `124392903` |
| `docs/tutorials/field-linear-algebra-at-scale.md:42` | `@/inv/current-state-scope` | Montgomery storage is stated for $P = 2$ | `71e933b84` |
| `docs/tutorials/field-linear-algebra-at-scale.md:88` | `@/inv/current-state-scope` | The `gemm` path is stated without the field hooks that replace it | `71e933b84` |
| `crates/gf2-kernels-simd/README.md:10`, `:23`, `:32` | `@/inv/current-state-scope` | Same detector, target and feature defects as the architecture page; the family list reads as complete | `71a1eb545` |
| `crates/gf2-kernels-simd/src/lib.rs:2`, `:9` | `@/inv/current-state-scope` | Same defects in the crate-root doc | `71a1eb545` |
| `crates/gf2-core/src/lib.rs:1`, `:7` | `@/inv/non-obvious-comments` | A title line above the orientation paragraph; a second-level heading | `6baf3db24` |
| `crates/gf2-algebra/src/lib.rs:7` | `@/inv/non-obvious-comments` | The orientation restates the crate attribute | `6baf3db24` |
| `crates/gf2-sim/src/lib.rs:6` | `@/inv/external-claims-cited` | Standard named without a registry key | `71a1eb545` |
| `crates/gf2-sim/src/lib.rs:40` | `@/inv/single-source-prose` | A copied worker-count set | `6baf3db24` |

### Judged dispositions

| Page:line | Question | Disposition |
|---|---|---|
| `README.md:6` | "aims to pair" | A purpose statement, which the landing page carries; the sentence makes no behavioral claim |
| Crate READMEs, last line | Link into `target/doc/` | The owner-approved form, `@/issue/b16d013a/decision/DEC-01` |
| `docs/reference/performance-evidence.md:17` | The linked protocol text differs in digest from the receipts' snapshots | Not a defect: the link names the protocol, and the page states that each receipt carries the snapshot that evaluates it |
| `docs/reference/standards-conformance.md` | Standard symbols in code spans | Not a defect: they name the standard's parameters as the code does |
| Eponymous terms (Montgomery form, Rabin's criterion, Clopper–Pearson and the like) | No registry key cited | Vocabulary assumed under `@/inv/researcher-audience`; standards, libraries and data sets carry keys |

Placement (`@/inv/adapted-diataxis-placement`), audience
(`@/inv/researcher-audience`), the crate-root example
(`@/inv/teaching-rustdoc-examples`, in `crates/gf2-sim/src/lib.rs`, a typestate
build and sweep) and the Monte Carlo table of the link tutorial
(`@/inv/uncertainty-reported`: frame counts and intervals present, run record
linked at a fixed commit) carry no finding. `wc -l AGENTS.md` prints the
limit `@/inv/agents-md-scope` sets.

### Policy statements the tree does not meet

`AGENTS.md` is the owner's contract. Each row is a policy sentence the tree
does not meet or enforce. This audit changes neither the sentence nor the
tree; the issue of each row tracks the reconciliation.

| `AGENTS.md` line | Invariant | Policy | Tree | Tracked by |
|---|---|---|---|---|
| `:21`, `:24`, `:31`, `:32` | `@/inv/current-state-scope` | Commands pass `--all-features` | `hip` pulls in `gf2-kernels-hip`, whose `build.rs` needs `hipcc`; `scripts/cargo-ci.sh:222` passes a feature list without ROCm | `@/issue/51f299d2` |
| `:19` | `@/inv/single-source-prose` | MSRV 1.95 | `crates/gf2-kernels-simd/Cargo.toml` declares no `rust-version` | `@/issue/578adb63` |
| `:42` | `@/inv/convention-convergence` | Test execution passes `--test` | `scripts/cargo-ci.sh:283`, `:304`, `:309`–`:312` run `cargo test` through the wrapper without it | `@/issue/20f5343b` |
| `:69` | `@/inv/current-state-scope` | Other production crates deny unsafe code | The binary roots under `crates/gf2-coding/src/bin/` and `crates/gf2-sim/src/bin/` carry no `deny(unsafe_code)`; none contains `unsafe` | `@/issue/75a71fab` |
| `:97`, `:98` | `@/inv/current-state-scope` | A per-test kill and a sixty-second suite budget | `.config/nextest.toml` declares per-test limits and overrides and no suite limit | `@/issue/774f2de4` |
| `:98` | `@/inv/convention-convergence` | Ignored tests carry a `slow:` or `sim:` reason | Bare `#[ignore]` at `crates/gf2-coding/src/bcjr/mod.rs:743`, `crates/gf2-coding/src/ldpc/core.rs:2013` and `crates/gf2-coding/tests/nr5g_regression.rs:91`; `bench:` and `external:` reasons also occur | `@/issue/18db6e3d` |
| `:98` | `@/inv/convention-convergence` | Same policy | Bare `#[ignore]` at `crates/gf2-core/src/field/axiom_tests.rs:762` | `@/issue/0407aca8` |

### Limits of the verification

Claim verification is by reading. Each row names a claim class the reading
does not settle and what settles it.

| Claim class | Checked | Settled by |
|---|---|---|
| Commands and Rust snippets build and run | Signatures and argument parsers | The `cargo-ci` gate of this issue builds the workspace and its doctests; the field tutorial command runs at `999228b25` and prints the page's sample lines with `<elapsed>` filled |
| Sample output of the link tutorial | Internal consistency; the intervals recomputed | The run record the page links at a fixed commit |
| A receipt measures the code path the library ships | The page states an admission rule in place of the assertion (`999228b25`) | `@/issue/16fee10f` tracks the per-receipt source comparison |
| Charon `--opaque` and `taskset` behavior | The page's statements against the scripts that invoke the tools | Outside the tree: the tools' own documentation |
| GitHub URLs are published | Each pinned commit and path exists in the local object store | Outside the tree: the state of the remote |
| "not on crates.io"; gfx1030 as the only exercised architecture | No contrary evidence in the tree | Outside the tree: the registry and the hardware at hand |
| Section and table numbers of the cited standards | Against the in-tree citations; the two numbers without one are removed (`71e933b84`) | Outside the tree: the standards' text; the one suspected mismatch is `@/issue/3a5894ed` |

## Scans

Command per row: `python3 fca578b1-scans.py <scan> > fca578b1-scan-<scan>.txt`,
run from this directory. The patterns are the constants at the head of the
script.

| Scan | Output | Remaining matches |
|---|---|---|
| `narration` | [`fca578b1-scan-narration.txt`](fca578b1-scan-narration.txt) | See below |
| `marketing` | [`fca578b1-scan-marketing.txt`](fca578b1-scan-marketing.txt) | See below |
| `figures` | [`fca578b1-scan-figures.txt`](fca578b1-scan-figures.txt) | See below |
| `issue-refs` | [`fca578b1-scan-issue-refs.txt`](fca578b1-scan-issue-refs.txt) | See [Issue references](#issue-references) |
| `dev-paths` | [`fca578b1-scan-dev-paths.txt`](fca578b1-scan-dev-paths.txt) | See below |
| `plain-math` | [`fca578b1-scan-plain-math.txt`](fca578b1-scan-plain-math.txt) | See below |
| `standards` | [`fca578b1-scan-standards.txt`](fca578b1-scan-standards.txt) | Zero matches |

| Scan | Matches | Reason they are not findings |
|---|---|---|
| `narration` | `AGENTS.md`, between the projection markers | Generated from `.jit/invariants.toml`; the invariants name the vocabulary they forbid |
| `narration` | `AGENTS.md` "expected to", "intended" | A rule about tests that exceed the budget; the intrinsic a design names |
| `narration` | `README.md` "aims" | Judged disposition above |
| `narration` | "rewrites", "rewritten", "replace", "replaces", "remove", "removing", "new file", "later window", "later run", "intended points", "was measured", "sweep plan" in `docs/` | Each describes an action of a tool or of the reader, a file the reader creates, a subsequent run, or a recorded measurement; none contrasts states of the system |
| `marketing` | `AGENTS.md`, between the projection markers | Generated invariant text |
| `marketing` | "fast tier", "fast test tier" | The test tier's name |
| `marketing` | `AGENTS.md` "research-grade" | The mission's audience term, shared with `@/inv/researcher-audience` |
| `marketing` | "constant coefficient fastest" | Enumeration order |
| `marketing` | "best-effort" | The build script's handling of optional targets |
| `figures` | `AGENTS.md` "wall-clock", "crossover", "scalability", and the projection lines | Policy on claims; no figure |
| `figures` | `$256 \times 256$` | Table dimensions |
| `figures` | `$250$ ms` | A harness parameter the receipt's command passes |
| `figures` | "300 frames", "95%" | A frame budget and a confidence level |
| `figures` | "wall-clock" in the tutorials | Sentences that label the printed time an unbenchmarked observation |
| `dev-paths` | `AGENTS.md` `dev/scripts/…` | Operational scripts at their current paths |
| `dev-paths` | `AGENTS.md`, projection lines | Generated invariant text |
| `dev-paths` | URLs on `github.com` | Evidence pinned to a commit |
| `dev-paths` | `reproduce-performance-evidence.md` commands and `ccx1-bench-flock.sh` | The benchmark-window tools the procedure runs, at their current paths |
| `plain-math` | Crate-root `lib.rs` docs | Rustdoc renders no LaTeX |
| `plain-math` | `mod` in `acceleration-architecture.md` | Part of a file name |

## Performance figures

The `figures` scan lists every match outside
`docs/reference/performance-evidence.md` and prints the count of matches on that
page. No listed match is a performance figure. On the evidence page each
figure matches its linked receipt at the stated precision; the `:110` finding
records the mismatch the comparison found.

## Issue references

`AGENTS.md` states the form in its documentation paragraph. The `issue-refs`
scan matches `@/issue/` addresses, `jit:` tokens, `issue <id>` phrases, `#<n>`
and bare eight-digit hexadecimal identifiers.

| Match | Disposition |
|---|---|
| `@/issue/ec530af9` | The required form; the issue is done and links the pipeline design document |
| `@/issue/<short-id>` | The statement of the form |
| `jit:<short-id>`, `jit:<id>` | Commit-subject scope syntax |
| `jit:…:begin`, `jit:…:end` | Projection markers |

## Prior sources and verbatim comparison

A permanent page here is a file that `AGENTS.md` names as permanent
documentation: the root README, a crate README, a page under `docs/`, or a
crate-root Rustdoc block. `AGENTS.md` is the operational guide and its text
continues from its earlier revisions.

[`fca578b1-verbatim-sources.tsv`](fca578b1-verbatim-sources.tsv) gives every
page the same source sets; `fca578b1-verbatim.py` prints the number of
files they resolve to.

| Revision | Pathspec | Content |
|---|---|---|
| `791beb2b0` | `**/*.md` | Every Markdown file before the overhaul: READMEs, crate guides, roadmaps, plans, reports |
| `791beb2b0` | `crates/**/*.rs` | Every Rustdoc comment before the overhaul |
| `6a38cc8b6` | `dev/active/fa787f85-documentation-overhaul/**/*.md` | The epic's records, among them `e3ff339a-fact-map.md` |

The table is derived with

```sh
for p in $(git ls-files README.md 'docs/**/*.md' 'docs/*.md' 'crates/*/README.md' 'crates/*/src/lib.rs' | sort -u); do
  printf '%s\t%s\t%s\n' "$p" 791beb2b091b65f375929cc16a0a8c1b042b17fc '**/*.md'
  printf '%s\t%s\t%s\n' "$p" 791beb2b091b65f375929cc16a0a8c1b042b17fc 'crates/**/*.rs'
  printf '%s\t%s\t%s\n' "$p" 6a38cc8b6170d2c91ea5bf03397dd7963196ab57 'dev/active/fa787f85-documentation-overhaul/**/*.md'
done > fca578b1-verbatim-sources.tsv
```

The authoring issues, from `git log --format=%s 791beb2b0..6a38cc8b6 -- <page>`,
name no narrower source list, so every page is compared with the whole
corpus.

| Page | At the base | Authoring issues |
|---|---|---|
| `README.md` | yes | `@/issue/a82d2347`, `@/issue/da120b98`, `@/issue/fdb998ec` |
| `crates/gf2-algebra/README.md` | yes | `@/issue/4f752758`, `@/issue/e3ff339a` |
| `crates/gf2-coding/README.md` | yes | `@/issue/698fa793`, `@/issue/a82d2347`, `@/issue/e3ff339a` |
| `crates/gf2-core/README.md` | yes | `@/issue/8b42d23b`, `@/issue/e3ff339a` |
| `crates/gf2-kernels-simd/README.md` | yes | `@/issue/8f61d6de`, `@/issue/ee50e270` |
| `crates/gf2-sim/README.md` | no | `@/issue/b16d013a` |
| `crates/gf2-stats/README.md` | yes | `@/issue/ee50e270` |
| `crates/gf2-algebra/src/lib.rs` | yes | `@/issue/12907582`, `@/issue/545b113f`, `@/issue/57436474`, `@/issue/8f61d6de` |
| `crates/gf2-coding/src/lib.rs` | yes | `@/issue/12907582`, `@/issue/4ad869d6`, `@/issue/545b113f`, `@/issue/aa0558c1` |
| `crates/gf2-core/src/lib.rs` | yes | `@/issue/12907582`, `@/issue/4997a3ed`, `@/issue/545b113f`, `@/issue/63bad95d` |
| `crates/gf2-kernels-hip/src/lib.rs` | yes | `@/issue/12907582`, `@/issue/93aed46a` |
| `crates/gf2-kernels-simd/src/lib.rs` | yes | `@/issue/12907582`, `@/issue/1b034786`, `@/issue/290b8716`, `@/issue/34adff85`, `@/issue/545b113f` |
| `crates/gf2-sim/src/lib.rs` | yes | `@/issue/12907582`, `@/issue/545b113f`, `@/issue/f22ec590` |
| `crates/gf2-stats/src/lib.rs` | yes | `@/issue/545b113f` |
| `docs/index.md` | no | `@/issue/69aa6896` and the issue of each page it lists |
| `docs/concepts/acceleration-architecture.md` | no | `@/issue/deda0962`, `@/issue/ee50e270` |
| `docs/concepts/finite-field-arithmetic.md` | no | `@/issue/44f4586d` |
| `docs/how-to/formal-verification.md` | no | `@/issue/da120b98` |
| `docs/how-to/reproduce-performance-evidence.md` | no | `@/issue/01c20b61` |
| `docs/how-to/run-simulation-campaigns.md` | no | `@/issue/9dcc0cae`, `@/issue/cdba4e71` |
| `docs/how-to/select-acceleration.md` | no | `@/issue/8a9156fc` |
| `docs/reference/performance-evidence.md` | no | `@/issue/1fca3739` |
| `docs/reference/standards-conformance.md` | no | `@/issue/5625a423` |
| `docs/reference/supported-configurations.md` | no | `@/issue/2ff9953a` |
| `docs/tutorials/coded-modulation-link-simulation.md` | no | `@/issue/cdba4e71` |
| `docs/tutorials/field-linear-algebra-at-scale.md` | no | `@/issue/6c314026` |

`python3 fca578b1-verbatim.py > fca578b1-verbatim.txt` compares at $N = 7$
([`fca578b1-verbatim.txt`](fca578b1-verbatim.txt)). At $N = 5$ and $N = 6$ the
report fills with set phrases such as "for the life of the process"; at
$N = 7$ every reported run is a carried sentence, a standard's table
reference or a proper name. The script's header states what counts as prose.

Runs the comparison reported at `6a38cc8b6`, each reworded:

| Page | Shared with | Fix |
|---|---|---|
| `docs/how-to/formal-verification.md` | `docs/lean4-verification-pipeline.md` | `124392903` |
| `docs/how-to/reproduce-performance-evidence.md` | `18858de1-root-resolution.md` | `124392903` |
| `docs/how-to/run-simulation-campaigns.md` | `crates/gf2-sim/src/executor/drain.rs` | `124392903` |
| `docs/index.md` | `9b2886a7-invariant-draft.md` | `124392903` |
| `docs/reference/standards-conformance.md` | `crates/gf2-coding/src/bch/dvb_t2/mod.rs` | `124392903` |
| Crate-root docs of `gf2-algebra`, `gf2-coding`, `gf2-core`, `gf2-kernels-hip`, `gf2-kernels-simd`, `gf2-sim`, `gf2-stats` | Their own text at the base | `6baf3db24` |

Runs that remain at the measured commit, in
`docs/reference/standards-conformance.md`:

| Run | Allowed class |
|---|---|
| "EN 302 755 Tables 6a and 6b" | Identifier: the standard's table numbers |
| "the DVB-T2 Verification and Validation reference streams" | Citation: the name of the cited vector source |

No other page shares a run of seven prose words with a prior source.

## Code observations

Found while verifying claims; outside the audited surface and unchanged here.
The pages state what the code does.

| Location | Observation | Tracked by |
|---|---|---|
| `crates/gf2-sim/src/snr_checkpoint.rs:300`, `crates/gf2-coding/src/simulation.rs:45`, `crates/gf2-coding/Cargo.toml:68` | `ctrlc` is built without its `termination` feature, so SIGTERM ends the process without a checkpoint flush; the manifest comment states a SIGTERM flush | `@/issue/37235fd6` |
| `crates/gf2-core/src/primitive_polys.rs:108` | `trinomials(8)` returns a reducible polynomial | `@/issue/0657c9fa` |
| `crates/gf2-coding/src/bch/dvb_t2/params.rs:40`, `:50` | Comments assign Table 6a to short frames and 6b to normal frames | `@/issue/3a5894ed` |
| `crates/gf2-kernels-hip` | No file states the ROCm version the crate is tested with | `@/issue/c7c11e7e` |
| `dev/scripts/ccx1-bench-flock.sh:85` | A `dev/active/` path literal in an error message (`@/inv/no-dev-path-coupling`) | `@/issue/84b67272` |

## Reproduction

From this directory, at the measured commit:

```sh
python3 fca578b1-scans.py surface | diff - fca578b1-surface.txt
for s in narration marketing figures issue-refs dev-paths plain-math standards; do
  python3 fca578b1-scans.py "$s" | diff - "fca578b1-scan-$s.txt"
done
python3 fca578b1-verbatim.py | diff - fca578b1-verbatim.txt
```

Each `diff` prints nothing. The scans read the surface files; the comparison
reads the pages and the pinned revisions.
