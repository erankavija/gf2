# The residual-shift kernel route: retention or removal

> **Diátaxis Type:** Explanation

Report for `f8dd4dde` (epic `1a379447`). This document is the record REQ-08
asks for, in place before the route is measured. It states why the route is in
the tree, what decides whether it stays, and what removing it consists of.

## The route ships in order to be measured

`gf2-core`'s residual `BitVec` shift branch — the one a `k % 64` other than zero
reaches — selects between the portable funnel and a `bmi2`-gated funnel kernel in
`gf2-kernels-simd`. Both routes answer one behavioural suite, and one shipped
build reaches either of them through the force switch of
`gf2_core::residual_shift`.

That is the whole of this work's claim. Nothing here measures either route or
compares them: the [profile](../c04dd4ac-zen3-shifts-and-permutations/shift-profile.md)
records the branch as material and nominates the form, and the
[feasibility record](../c04dd4ac-zen3-shifts-and-permutations/shift-feasibility-record.md)
establishes that the form compiles at the repository's MSRV and reaches the
instructions its nomination names. Feasibility is not a win, and neither is
shipping. The A/B confirmation `00dd43c3` is what measures the two routes
against each other.

## The frozen rule decides

`00dd43c3` freezes its acceptance rule before it launches, under the epic's
[measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and the [shared protocol](../f547c394/protocol.md). That rule, not a reading of
its numbers afterwards, decides the route's disposition:

- A qualifying verdict retains the route in production. The confirmation's
  outcome record is then the authority on it, and this document is discharged.
- A non-qualifying verdict returns the residual branch to the portable funnel
  alone. A family whose ledger admits no confirmatory cell records that
  arithmetic as its outcome, which is a non-qualifying verdict for this purpose.

The route has no downstream production caller to weigh against either outcome:
the two public shift methods are its only consumers, which the profile's empty
consumer set records.

## What removal consists of

Removal deletes the kernel route and leaves the residual branch on the funnel
the portable path already runs. The items that go:

- `crates/gf2-kernels-simd/src/x86/shift_funnel.rs`, its sibling assembly
  artefact `crates/gf2-kernels-simd/src/x86/asm/shift_funnel.asm.txt`, and the
  module declaration in `crates/gf2-kernels-simd/src/x86/mod.rs`.
- `crates/gf2-kernels-simd/src/shift_funnel.rs` — the bundle, its detection and
  its safe wrappers — and the module declaration in that crate's `lib.rs`.
- `gf2_core::simd::maybe_shift_funnel` and its `OnceLock`, in both the `simd` and
  the non-`simd` form.
- In `crates/gf2-core/src/residual_shift.rs`: the `Bmi2Funnel` route, the
  `Funnel` enum it is the second variant of, the lane witness and the force
  switch. The module keeps its two entry points and the portable funnels they
  call, and its selection point collapses to the one route left.
- The route sweep and the witness assertions of
  `crates/gf2-core/tests/residual_shift_routes.rs`. The corpus itself stays: it
  is the behavioural coverage of the public methods, which outlives either
  disposition.
- The `# Routes` paragraphs of `BitVec::shift_left` and `BitVec::shift_right`
  lose their residual-kernel sentence and keep the word-aligned bundle's.

Removal also closes this document: a removed route needs no retention rule, and
the confirmation's outcome record carries the reason.
