# Contributing to gf2

Thank you for contributing. The root [`AGENTS.md`](AGENTS.md) is the canonical
engineering contract: supported commands, architecture boundaries, test tiers,
documentation policy, proof requirements, performance evidence, and JIT
workflow all live there. This file is only the contributor entry point.

## Before you start

- Install Rust 1.95 or later, Cargo, Git, and `cargo-nextest`.
- Read `AGENTS.md` and `.jit/reference/content-standards.md`.
- Select work with `jit query available`, or inspect the requested item with
  `jit issue status <id>` and `jit issue show <id>`.
- Confirm dependencies and required gates before changing code.

The core project invariants are addressable JIT registry items. In particular,
changes must preserve `@/inv/bitvec-tail-padding`,
`@/inv/canonical-bit-indexing`, `@/inv/finite-field-laws`,
`@/inv/standards-vector-conformance`,
`@/inv/backend-behavioral-equivalence`,
`@/inv/unsafe-kernel-isolation`, and
`@/inv/crate-dependency-direction`. The complete rendered registry is in
`AGENTS.md`; `.jit/invariants.toml` is its source of truth.

## Make and verify the change

Work test-first and keep the change attributable to its JIT issue. Update
permanent documentation or linked design material when behavior or architecture
changes. Use shared conformance suites for shared mathematical, codec, modem,
pipeline, and backend contracts.

Run the repository contract before handing off:

```bash
./scripts/cargo-ci.sh
```

Also run any specialized gates required by the issue, such as Lean, Kani,
assembly-receipt, Criterion, or parallelism evidence. Benchmark claims need a
reproducible protocol and committed receipt; they are not established by a
wall-clock assertion in an ordinary test.

## Submit

Use a conventional commit subject and include the JIT short ID in the scope when
the commit implements an issue, for example:

```text
fix(jit:8ce6f8aa): preserve zeroed BitVec tail padding
```

In the pull request or handoff, identify the JIT item, summarize observable
behavior, list verification performed, link documentation and benchmark/proof
evidence, and call out any explicitly approved exception. Do not duplicate
registry facts in prose; cite their `@/inv/...`, `@/rule/...`, or
`@/gate/...` address.

This project follows the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct).
