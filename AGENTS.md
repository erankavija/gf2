# Engineering contract for gf2

This is the canonical guidance for human and automated contributors. Its rules
apply recursively under `@/inv/document-placement`, and `CLAUDE.md` is a symlink to
this file.

## Mission

Build a fast, ergonomic research toolkit for finite fields and coding theory:
composable APIs with sensible defaults, standards-backed correctness, throughput
competitive with the strongest reference implementations, and reproducible
results. Optimization never relaxes mathematical or standards conformance.

## Supported toolchain and commands

- The Rust MSRV is 1.95. Use that toolchain for compatibility-sensitive work.
- Run the repository CI contract with `./scripts/cargo-ci.sh`.
- Workspace commands take their features from `./scripts/cargo-features.sh`,
  as the CI contract does: `--all-features` where ROCm `hipcc` exists, a set
  without the `hip` features elsewhere.
- Build all ordinary workspace crates with
  `cargo build --workspace $(./scripts/cargo-features.sh)`.
- Run the fast test tier with
  `cargo nextest run --workspace $(./scripts/cargo-features.sh) --cargo-profile ci-test --profile ci`.
- For focused work, keep the `ci-test` profile and select a package or test
  expression, for example
  `cargo nextest run -p gf2-core --cargo-profile ci-test --profile ci`.
- Build benchmarks under `--release`; `ci-test` drops LTO.
- Check formatting with `cargo fmt --all -- --check` and lint with
  `cargo clippy --workspace --all-targets $(./scripts/cargo-features.sh) --features tuning-profile -- -D warnings`.
- Build API documentation with `cargo doc --workspace $(./scripts/cargo-features.sh doc) --no-deps`;
  `doc` selects the one host-independent set both CI entry points document.
- Run Lean regeneration with `./scripts/verify-lean.sh`; build committed proof
  sources with `(cd proofs && lake build)`.
- Build the ROCm-only crate explicitly with
  `cargo build --manifest-path crates/gf2-kernels-hip/Cargo.toml` on a suitable
  host. It is intentionally outside the default Cargo workspace.

Tests, examples, simulations, and benchmarks that do substantial work must use
release mode.

- The host is never locked during a working session. An indication is a short
  run on a few pinned cores, allowed at any time and labelled as an indication.
  A stated result is a queue line for the benchmark window (`GF2_BENCH_WINDOW=1`,
  exported by the window runner); outside it `dev/scripts/ccx1-bench-flock.sh`
  refuses to run.
- `CARGO_CI_STEPS=<pcre>` makes `./scripts/cargo-ci.sh` run matching steps
  only, no CI verdict; `^$` lists them.
- `RAYON_NUM_THREADS` in `.cargo/config.toml` pairs with `threads-required` in
  `.config/nextest.toml`; change one and change the other. A binary run directly
  from `target/` inherits neither.

## Architecture boundaries

- `gf2-core` owns low-level bit storage, dense and sparse linear algebra,
  finite-field abstractions and arithmetic, and safe runtime dispatch. Its only
  production dependency inside the workspace is `gf2-kernels-simd`.
- `gf2-coding` (on `gf2-core`) owns codes, modems, channels, domain composition.
- `gf2-algebra` owns packed $\mathbb{F}_3$, $\mathbb{F}_5$ and $\mathbb{F}_7$
  arithmetic and permanent algorithms; it may dispatch to isolated kernels
  without becoming a kernel layer itself.
- `gf2-sim` owns the generic CPU/GPU simulation pipeline and orchestration
  primitives. JIT item `@/issue/ec530af9` links its design source of truth.
  Experiment code stays outside it under `@/inv/library-first-generality`.
- `gf2-kernels-simd` and `gf2-kernels-hip` own the machine-specific kernels.
- `proofs/` owns Lean verification of selected arithmetic and algorithm paths.
- `experiments/<name>/` holds one experiment and `experiments/common` the
  mechanics several share. No `crates/*/Cargo.toml` depends on `experiments/`.

## Correctness and test policy

Work test-first: establish failing behavioral evidence, implement the smallest
coherent change, then add shared property or conformance coverage where the
contract is mathematical or implemented by several backends.

- Exercise word-boundary cases 0, 1, 63, 64, and 65 when bit-packed behavior is
  involved. Preserve zero tail padding and canonical little-endian bit indexing.
- Public APIs need rustdoc stating purpose, panics, safety conditions, and
  non-obvious complexity.

The ordinary fast tier has a default fifteen-second per-test kill (sixty under
`BUSY_HOST_OVERRIDE=1`) and a sixty-second suite budget. Tests expected to exceed it use a descriptive `#[ignore = "slow: ..."]`
or `#[ignore = "sim: ..."]`; normal agent work never opts into ignored tests.
The slow tier uses
`cargo nextest run --workspace $(./scripts/cargo-features.sh) --release --profile slow --run-ignored ignored-only`
and has a 600-second budget. Tests too heavy for that tier must self-gate on
required host data or benchmark mode, or use the configured `slow-serial` group.
Every nextest profile in the workspace and standalone `gf2-kernels-hip` crate
inherits the `gpu-serial` group and its `scripts/nextest-gpu-lock.sh` wrapper.
Device-opening workspace tests, ignored tests included, join that group; all
standalone HIP crate tests join it. Each holds a host-wide lock while running: the wrapper runs
flock `/tmp/gf2-gpu.lock` unless `GF2_GPU_LOCK` names another file. `cargo test` takes no lock.
Do not widen a wall-clock assertion to accommodate a busy runner.

## Performance and research evidence

Profile and read the assembly before optimizing; check each change with an
indication and state results from the benchmark window, under
`@/inv/benchmark-backed-performance`. Compare against the reference's fastest
configuration, under `@/inv/performance-deliverable`. Machine-dependent
assertions are benchmark gates, not ordinary tests; `GF2_BENCH=1` runs use the
lock wrapper under `dev/scripts/`.

Permanent documentation lives in `README.md`, crate-level rustdoc, or `docs/`.
Prose and Rustdoc reference a JIT issue as `@/issue/<short-id>`, the form the
mechanical documentation check resolves.

## Planning, proof, and change discipline

- Read `.jit/reference/content-standards.md` before authoring JIT content.
  Active criteria use `## Success Criteria` and zero-padded identifiers such as
  `[hard] REQ-01:` or `[aspirational] REQ-02:`. Correctness is always hard;
  aspirational is limited to explicitly provisional empirical targets.
- Work mentioning specific intrinsics or toolchain-sensitive behavior verifies
  the design with Rust 1.95 before breakdown. Preserve a tested scalar fallback
  when the intended intrinsic is unavailable at the MSRV.
- Formal-proof or model-checking work needs an approved sketch before proof code:
  lemma statements, proof strategy, exact production path, and, for Kani,
  unwind bounds and dispatched paths.
- Commit subjects are `<type>(<scope>): <summary>`, with type one of `feat`,
  `fix`, `docs`, `test`, `refactor`, `perf`, `chore`; the list is closed. The
  whole subject line is at most 71 characters. A commit that implements or
  serves an issue carries `jit:<short-id>` as its scope. Work with no issue
  carries `jit:none`.
- The rule binds every commit that enters `main`: merge commits
  (`chore(jit:<id>): merge ...`), tracker state commits and salvage commits
  included. Reword a private branch before it merges; the history of `main`
  is immutable.
- A pull request or handoff names the JIT item, behavior change, verification run,
  and evidence links. Contributors follow the
  [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct).

<!-- jit:profile-sim-research-guidance:begin -->
## gf2 Engineering Invariants

<!-- jit:invariants:begin -->
- **canonical-bit-indexing** — Canonical bit index i maps to word i >> 6 and mask 1u64 << (i & 63); conversions preserve this little-endian numbering.
- **finite-field-laws** — Every finite-field implementation satisfies the shared field-law conformance suite for its supported domain.
- **standards-vector-conformance** — Standards-based coding and modem behavior is checked against the authoritative standard or a named, versioned vector source; generated fixtures do not replace external conformance evidence.
- **backend-behavioral-equivalence** — Scalar, SIMD, parallel CPU, and GPU implementations expose equivalent observable results within their declared numerical contract.
- **single-source-prose** — Every fact with a single source of truth reaches prose by projection or citation; volatile facts (counts, enumerations, registry contents) are stated structurally or derived, and a hand-maintained copy is a staleness defect.
- **shared-test-contracts** — Tests assert observable semantics; literal inventories live in one canonical suite per stable external contract. Every implementation of a shared field, codec, modem, pipeline, or kernel interface runs the same behavioral conformance suite.
- **accelerator-safe-fallback** — Unsupported SIMD or GPU capabilities and recoverable accelerator resource failures select a tested safe fallback; fatal kernel or driver failures remain explicit.
- **semantic-types** — Every identity, constrained token, closed vocabulary, and protocol sentinel has one canonical semantic type; raw strings exist only at parsing and serialization boundaries.
- **canonical-cutover** — Superseded aliases, fields, and representations are removed after cutover; compatibility code is allowed only in a named, versioned migration boundary with a tracked removal condition.
- **unsafe-kernel-isolation** — Production unsafe code is isolated to gf2-kernels-simd and gf2-kernels-hip, with explicit safety contracts at every unsafe boundary. The one exception is `crates/gf2-core/src/gf2m/gf256_bytes.rs`, the byte views of `Gf256` slices.
- **crate-dependency-direction** — Crate dependencies point from domain orchestration toward mathematical primitives and isolated kernels; mathematical, coding-domain, simulation, and kernel layers do not acquire reverse production dependencies.
- **deterministic-seeded-execution** — A fixed seed and configuration produce identical observable results across supported worker counts, scheduling paths, checkpoint/resume boundaries, and accelerator fallbacks.
- **benchmark-backed-performance** — Measurements have two tiers. An indication is a short run on a few pinned cores at any time; it guides the next change, is labelled an indication, and supports no stated result. A result comes from the benchmark window and names the harness, configuration and host that reproduce it. Estimates are identified as estimates.
- **performance-deliverable** — Throughput of field arithmetic, kernels and decoders is a deliverable beside correctness. A comparison against another implementation uses that implementation's fastest configuration at matched error rate; a result against a weaker configuration is labelled as such.
- **structural-performance-defaults** — Default routes, thresholds and block sizes derive from the code's structure and the selected backend's vector width and cache sizes. A host measurement motivates a default and is cited as an example; a default that names a host, a benchmark cell or a measured constant without its structural form is a defect. Every such default is caller-overridable.
- **test-tier-budgets** — Every test respects its tier's configured time budget: release-mode fast-tier tests within the per-test and suite budgets, ignored slow and simulation tests within the nightly budget or through an explicitly serialized or host-gated execution path; slower work carries a descriptive ignore tier.
- **convention-convergence** — A shared convention or abstraction has one form: work that finds it harmful or ill-fitting changes it at its source, or reports the mismatch as a blocking concern before proceeding. A local parallel variant, a private helper duplicating a shared mechanism, or a bypass around an abstraction is a defect unless it is a named, cited exception with a tracked convergence condition.
- **present-tense-prose** — Permanent artifacts state the system's current behavior and limits. Change history lives in commit messages and issue history; plans, deferrals and non-goals live in JIT issues.
- **library-first-generality** — Generic behavior lands as a reusable API in the layer-appropriate library crate, and binaries, simulations, and orchestration stay thin consumers. An experiment's protocol, constants, records, validation and orchestration live with that experiment outside the library crates and consume library APIs.
- **committed-evidence-budget** — Measurements, receipts, execution logs, result tables, copied source and inputs, and third-party archives are never committed; the repository holds harnesses and their configuration, and a run writes outside the tracked tree. A decision record may state the measured values its pre-registered rule decides on, labelled with the host; receipts, logs and result tables stay outside the tree. `scripts/check-tracked-budget.py` enforces this and the file, commit and tree limits, which only fall.
- **no-deferred-defects** — Quality defects surfaced while implementing a change are fixed in that change or filed as a tracked issue before completion; a TODO or FIXME marker without a tracked issue is a defect.
- **resolvable-references** — Tracked text names only what a fresh clone resolves, alone or with the tracker checkout mounted at `.jit/`: no host-local path and no repository path that does not exist. `scripts/check-references.py` enforces this.
- **falsification-preserved** — Data that contradicts a criterion, hypothesis, or cited claim is recorded together with the contradiction; silent rework of the falsified statement is a defect.
- **campaign-evidence** — A campaign is a measurement whose result a decision record or publication cites, or that spans several benchmark windows. It is a resumable sequence of bounded runs with one append-only execution log, and its record pins the executable digest, the configuration, the seed and the host facts the tool observes. Evidence is invalidated only by a change to those or to the tool's measurement behavior; Git commit IDs, clean/dirty status and documentation changes decide nothing. A ledger, schema, checker or protocol version beyond this is a defect unless a success criterion names it. Any other measurement carries a harness, a configuration and a host name.
- **caller-trusted-fast-paths** — API contracts trust the caller: validation exists to catch mistakes and to amortize cost, never to defend against an adversary. A path that skips validation for performance is a distinct method under the _unchecked naming convention with its precondition documented, and violating that precondition is caller error with unspecified mathematical results — never grounds for sealing types, policing evidence provenance, or other adversarial hardening in a library that does HPC, not security.
- **ergonomic-defaults** — Constructing a standard's code and decoding with defaults selects the fastest conforming path the host supports. Defaults follow the field's standard conventions, deviations are optional parameters, and where the algorithm permits the library adapts to its input instead of asking the caller.
- **non-obvious-comments** — Comments and rustdoc carry only what the code leaves non-obvious, in the shortest form that conveys it. A rustdoc example shows a workflow a researcher would copy, including constructing a standard's code and its decoder; examples for accessors and direct field mappings are a defect.
- **researcher-audience** — Permanent documentation addresses researchers evaluating or adopting gf2: it assumes relevant technical competence and states capabilities and measured results in neutral technical terms.
- **document-placement** — Every permanent page under `docs/` other than `docs/index.md` lives in exactly one of `tutorials/`, `how-to/`, `concepts/`, or `reference/`. Active development documents live under `dev/active/<epic-short-id>-<slug>/`, linked to their owning issues with `jit doc`. Every `AGENTS.md` governs its directory subtree and stays at or below 200 lines.
- **no-dev-path-coupling** — Code that locates development artifacts at run time resolves them through one configured repository root or by content identity; a hard-coded `dev/` location, a historical-path allow-list, or a path that depends on the caller's directory depth is a defect. Paths a build system requires literally, such as Cargo manifest dependencies and workspace members, are outside this rule.
<!-- jit:invariants:end -->

## JIT workflow

- Treat `.jit/` as repository-owned workflow configuration and issue data.
- Read `.jit/reference/content-standards.md` before authoring issues or planning documents.
- Derive hierarchy, templates, gates, namespaces, and documentation paths from repository configuration.
- A label means what its `[namespaces.<ns>]` declaration in `.jit/config.toml` says it means; read that declaration before judging what a label on an issue claims.
- Use `jit issue status`, `jit query available`, and the dependency graph to select and sequence work.
- Run the configured gates before completing an issue; a passing review placeholder is advisory evidence only.
<!-- jit:profile-sim-research-guidance:end -->
