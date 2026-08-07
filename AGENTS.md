# Engineering contract for gf2

This is the canonical repository-wide guidance for human and automated
contributors. Tool-specific files may add local operating notes, but they must
point here and must not restate or weaken this contract. More specific
`AGENTS.md` files, if present below the repository root, govern their subtree.

## Mission

Build a research-grade, high-performance finite-field and coding-theory toolkit
with composable APIs, standards-backed correctness, reproducible research, and
production-safe acceleration. Optimization never relaxes mathematical or
standards conformance.

## Supported toolchain and commands

- The Rust MSRV is 1.95. Use that toolchain for compatibility-sensitive work.
- Run the repository CI contract with `./scripts/cargo-ci.sh`.
- Build all ordinary workspace crates with
  `cargo build --workspace --all-features`.
- Run the fast test tier with
  `cargo nextest run --workspace --all-features --release --profile ci`.
- For focused work, keep release mode and select a package or test expression,
  for example `cargo nextest run -p gf2-core --release --profile ci`.
- Check formatting with `cargo fmt --all -- --check` and lint with
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- Build API documentation with `cargo doc --workspace --all-features --no-deps`.
- Run Lean regeneration with `./scripts/verify-lean.sh`; build committed proof
  sources with `(cd proofs && lake build)`.
- Build the ROCm-only crate explicitly with
  `cargo build --manifest-path crates/gf2-kernels-hip/Cargo.toml` on a suitable
  host. It is intentionally outside the default Cargo workspace.

Do not run multiple Cargo builds or nextest suites concurrently against the
shared target directory. Tests, examples, simulations, and benchmarks that do
substantial work must use release mode.

## Architecture boundaries

- `gf2-core` owns low-level bit storage, dense and sparse linear algebra,
  finite-field abstractions and arithmetic, and safe runtime dispatch. It has
  no production dependency on another gf2 workspace crate.
- `gf2-coding` owns codes, modems, channels, and coding-domain composition and
  depends inward on `gf2-core`.
- `gf2-algebra` owns packed F_3/F_5/F_7 arithmetic and permanent algorithms; it
  may dispatch to isolated kernels without becoming a kernel layer itself.
- `gf2-sim` owns the CPU/GPU simulation pipeline and orchestration. Its design
  source of truth is linked through JIT item `@/issue/ec530af9`.
- `gf2-kernels-simd` and `gf2-kernels-hip` are the only production crates that
  may contain `unsafe`. Every unsafe boundary requires an explicit safety
  contract. Other production crates deny unsafe code.
- `proofs/` owns Lean verification of selected arithmetic and algorithm paths.

Keep mathematical primitives, coding-domain behavior, orchestration, and
machine-specific kernels separated. Dependencies point inward; a lower layer
must not acquire a production dependency on a higher one. See
`@/inv/crate-dependency-direction` and
`@/inv/unsafe-kernel-isolation`.

## Correctness and test policy

Work test-first: establish failing behavioral evidence, implement the smallest
coherent change, then add shared property or conformance coverage where the
contract is mathematical or implemented by several backends.

- Test observable semantics, not private layout, except in the one canonical
  suite for an intentionally stable external representation.
- Exercise word-boundary cases 0, 1, 63, 64, and 65 when bit-packed behavior is
  involved. Preserve zero tail padding and canonical little-endian bit indexing.
- Every finite-field implementation runs the shared field-law suite. Standards
  implementations cite the authoritative edition or named vector source and
  include external conformance evidence.
- Scalar, SIMD, parallel, and GPU implementations share behavioral suites.
  Seeded work remains deterministic across supported worker counts, scheduling,
  checkpoint/resume, and fallbacks.
- Public APIs need rustdoc stating purpose, panics, safety conditions, and
  non-obvious complexity. Add a runnable example where it teaches a workflow or
  clarifies a material contract that prose and focused tests leave unclear;
  prefer one type- or module-level walkthrough over per-method repetition.
  Repetitive examples for accessors, constants, constructors, predicates, and
  direct field mappings are documentation and doctest burden.

The ordinary fast tier has a five-second per-test kill and a sixty-second suite
budget. Tests expected to exceed it use a descriptive `#[ignore = "slow: ..."]`
or `#[ignore = "sim: ..."]`; normal agent work never opts into ignored tests.
The nightly slow tier uses
`cargo nextest run --workspace --all-features --release --profile slow --run-ignored ignored-only`
and has a 600-second budget. Tests too heavy for that tier must self-gate on
required host data or benchmark mode, or use the configured `slow-serial` group.
Do not widen a wall-clock assertion to accommodate a busy runner.

## Performance and research evidence

Profile before optimizing. A performance, crossover, or scalability claim must
name a reproducible benchmark protocol and cite its committed receipt from a
suitable uncontended host. Machine-dependent assertions are benchmark gates,
not ordinary tests. Use `GF2_BENCH=1` only on a prepared benchmark host and use
the applicable lock wrapper under `dev/benchmarks/`.

Keep permanent documentation under `README.md`, crate-level rustdoc, or `docs/`.
Keep active designs, experiments, plans, presentations, and benchmark receipts
in the development areas registered in `.jit/config.toml`; link issue-scoped
material with `jit doc`. Prefer citations or generated projections over copied
facts. See `@/inv/single-source-prose`.

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
- Preserve one canonical abstraction. Change a shared convention at its source
  or record a named, tracked exception instead of creating a private parallel
  variant.
- Use conventional commit subjects (`feat`, `fix`, `docs`, `test`, `refactor`,
  `perf`, or `chore`), keep the first line under 72 characters, and include the
  JIT short ID in the scope when a commit implements an issue.

<!-- jit:profile-sim-research-guidance:begin -->
## gf2 Engineering Invariants

<!-- jit:invariants:begin -->
- **bitvec-tail-padding** — Every BitVec mutation leaves padding bits beyond len_bits zero, including construction from externally supplied words.
- **canonical-bit-indexing** — Canonical bit index i maps to word i >> 6 and mask 1u64 << (i & 63); conversions preserve this little-endian numbering.
- **finite-field-laws** — Every finite-field implementation satisfies the shared field-law conformance suite for its supported domain.
- **standards-vector-conformance** — Standards-based coding and modem behavior is checked against the authoritative standard or a named, versioned vector source; generated fixtures do not replace external conformance evidence.
- **backend-behavioral-equivalence** — Scalar, SIMD, parallel CPU, and GPU implementations expose equivalent observable results within their declared numerical contract.
- **single-source-prose** — Every fact with a single source of truth reaches prose by projection or citation; volatile facts (counts, enumerations, registry contents) are stated structurally or derived, and a hand-maintained copy is a staleness defect.
- **semantic-test-assertions** — Tests assert observable semantic properties or relationships; exact field-name inventories and literal-value assertions are confined to one canonical suite for an intentionally stable external contract.
- **shared-test-contracts** — Every implementation of a shared field, codec, modem, pipeline, or kernel interface runs the same behavioral conformance suite; implementation-specific tests cover only implementation-specific behavior.
- **accelerator-safe-fallback** — Unsupported SIMD or GPU capabilities and recoverable accelerator resource failures select a tested safe fallback; fatal kernel or driver failures remain explicit.
- **semantic-types** — Every identity, constrained token, closed vocabulary, and protocol sentinel has one canonical semantic type; raw strings exist only at parsing and serialization boundaries.
- **canonical-cutover** — Superseded aliases, fields, and representations are removed after cutover; compatibility code is allowed only in a named, versioned migration boundary with a tracked removal condition.
- **unsafe-kernel-isolation** — Production unsafe code is isolated to gf2-kernels-simd and gf2-kernels-hip, with explicit safety contracts at every unsafe boundary.
- **crate-dependency-direction** — Crate dependencies point from domain orchestration toward mathematical primitives and isolated kernels; mathematical, coding-domain, simulation, and kernel layers do not acquire reverse production dependencies.
- **deterministic-seeded-execution** — A fixed seed and configuration produce identical observable results across supported worker counts, scheduling paths, checkpoint/resume boundaries, and accelerator fallbacks.
- **benchmark-backed-performance** — Performance and crossover claims cite a reproducible benchmark protocol and committed receipt from a suitable uncontended host; estimates are identified as estimates.
- **fast-test-tier-budget** — Release-mode fast-tier tests respect the configured five-second per-test and sixty-second suite budgets; slower work carries a descriptive ignore tier.
- **slow-test-tier-budget** — Ignored slow and simulation tests respect the six-hundred-second nightly budget or self-select an explicitly serialized or host-gated execution path.
- **convention-convergence** — A shared convention or abstraction has one form: work that finds it harmful or ill-fitting changes it at its source, or reports the mismatch as a blocking concern before proceeding. A local parallel variant, a private helper duplicating a shared mechanism, or a bypass around an abstraction is a defect unless it is a named, cited exception with a tracked convergence condition.
<!-- jit:invariants:end -->

## JIT workflow

- Treat `.jit/` as repository-owned workflow configuration and issue data.
- Read `.jit/reference/content-standards.md` before authoring issues or planning documents.
- Derive hierarchy, templates, gates, namespaces, and documentation paths from repository configuration.
- A label means what its `[namespaces.<ns>]` declaration in `.jit/config.toml` says it means; read that declaration before judging what a label on an issue claims.
- Use `jit issue status`, `jit query available`, and the dependency graph to select and sequence work.
- Run the configured gates before completing an issue; a passing review placeholder is advisory evidence only.
<!-- jit:profile-sim-research-guidance:end -->
