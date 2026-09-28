# Approved correction to the shift-story background

> **Diátaxis Type:** Explanation
>
> **Status:** approved by the invoker and applied on 2026-09-28.

Documentation review `228fc432` finding F2 identifies a stale statement in
story `c04dd4ac`. The [implemented route](../f8dd4dde/retention-rule.md) and
[confirmation outcome](../00dd43c3/confirmation-outcome.md) establish the
current behavior.

Approved replacement of only the first sentence under `## Background`:

> BitVec explicitly dispatches whole-word shifts; residual-bit shifts use ordinary Rust loops.

With:

> With the `simd` feature, `BitVec` provides capability-gated whole-word and residual shift routes; the residual route uses BMI2 when available and otherwise retains the scalar funnel.

All remaining description text, success criteria, dependencies and gates stay
unchanged. This corrects current-state prose and adds no implementation scope.
