## Rules

- **@/rule/label-format** — Every label must match the canonical `namespace:value` format (namespace lowercase-kebab, value non-empty). Blocks the write and fails validation. (error, enforced)
- **@/rule/namespace-registry** — Every label's namespace must be declared in the namespace registry. An unknown namespace fails validation but never blocks a write. (error, advisory)
- **@/rule/namespace-unique-priority** — At most one `priority:` label per issue: `priority` is a unique namespace. Blocks the write and fails validation. (error, enforced)
- **@/rule/namespace-unique-resolution** — At most one `resolution:` label per issue: `resolution` is a unique namespace. Blocks the write and fails validation. (error, enforced)
- **@/rule/namespace-unique-team** — At most one `team:` label per issue: `team` is a unique namespace. Blocks the write and fails validation. (error, enforced)
- **@/rule/namespace-unique-type** — At most one `type:` label per issue: `type` is a unique namespace. Blocks the write and fails validation. (error, enforced)
- **@/rule/type-hierarchy-known** — Every `type:<value>` label must name a type declared in the configured type hierarchy. An unknown type fails validation but never blocks a write. (error, advisory)
- **@/rule/jit-content-standards** — Warn when nonterminal work lacks marked, stable success criteria. (warn, advisory)
- **@/rule/hard-criteria-covered** — Before an epic completes, every hard requirement is credited to a completed implementation descendant. (error, enforced)
- **@/rule/coverage-preview** — While breakdown is reviewed, every hard requirement is credited to a drafted implementation descendant. (error, enforced)
- **@/rule/orphan-leaf** — Warn when a leaf-level-typed issue carries no parent-membership label, leaving it unattached to any strategic container. Advisory: never blocks a write. (warn, advisory)
- **@/rule/strategic-consistency** — Warn when a strategic-typed issue lacks its own identifying membership label. Advisory: never blocks a write. (warn, advisory)
- **@/rule/namespace-unique-brackets** — At most one `brackets:` label per issue: `brackets` is a unique namespace. Blocks the write and fails validation. (error, enforced)
- **@/rule/namespace-unique-ppc-kernel** — At most one `ppc-kernel:` label per issue: `ppc-kernel` is a unique namespace. Blocks the write and fails validation. (error, enforced)

## Gates

- **@/gate/asm-artefact-present** — ASM artefact present: Fails if SIMD source files are modified without a corresponding sibling *.asm.txt update.
- **@/gate/breakdown-review** — Breakdown Review: Independently review decomposition quality, issue content, and dependency ordering before implementation.
- **@/gate/cargo-ci** — Rust CI checks pass: Run the full release-mode Rust workspace CI pipeline: build, tests, clippy, and formatting.
- **@/gate/cargo-kani** — Kani proof harnesses pass: Run all bounded model-checking harnesses and retain the bounded summary and compressed proof log.
- **@/gate/clippy** — Clippy lints pass: Require the configured Clippy targets to pass with warnings denied.
- **@/gate/code-review** — Code Review: Review issue-attributable implementation changes against repository policy and success criteria.
- **@/gate/coverage-preview** — Coverage Preview: Validate the container named by the breakdown issue's brackets label.
- **@/gate/criterion-1.5x** — Criterion 1.5x speedup: Require geomean speedup against the pinned baseline of at least 1.5x for the kernel named by ppc-kernel:<id>.
- **@/gate/doc-review** — Documentation Review: Automatically review issue-attributable documentation impact across README files, crate docs, docs/, and linked design material.
- **@/gate/fmt** — Code formatted: Require cargo fmt to report no formatting drift.
- **@/gate/gf2-kernels-hip-ci** — HIP kernel crate checks pass: Run the ROCm-only gf2-kernels-hip release tests, formatting, and Clippy that workspace cargo-ci intentionally excludes.
- **@/gate/holistic-review** — Holistic Container Review: Independently review a configured-hierarchy container for hard-criterion completion and coherence across descendants.
- **@/gate/jit-validate** — Issue Validation: Run declarative validation for the gated issue.
- **@/gate/lake-build** — Lean4 proofs compile: Require lake build to succeed with no sorry warnings in hand-written proof files.
- **@/gate/parallelism-pays** — Parallelism speedup demonstrated: Require a committed benchmark receipt showing that measured parallel speedup meets the issue's stated target.
- **@/gate/permanent-sampling-feas-ci** — Permanent sampling feasibility checks pass: Run the standalone permanent-sampling-feas release tests, formatting, and Clippy that workspace cargo-ci intentionally excludes.
- **@/gate/permanent-wave-gpu-ci** — Permanent wave prototype checks pass: Run the standalone permanent-wave-gpu release tests, formatting, Clippy, and ROCm probe build that workspace cargo-ci intentionally excludes.
- **@/gate/plan-review** — Plan Review: Independently review the linked plan before implementation work fans out.
- **@/gate/repo-validate** — Repository Validation: Run structural and declarative validation for the whole repository.
- **@/gate/research-review** — Research Review: Verify scientific rigor: claims trace to artifacts, statistics are sound, citations resolve, results are reproducible.
- **@/gate/tdd-reminder** — Write tests first (TDD): Remind implementers to establish failing behavioral evidence before implementation.
- **@/gate/tests** — All tests pass: Run the legacy cargo test gate retained for issues that explicitly require it.
- **@/gate/verify-lean** — Lean extraction regenerates: Regenerate LLBC and Lean from current Rust with Charon and Aeneas, then build the proof package.
