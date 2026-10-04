# Private BitSlice zero-copy outcome (jit:a1ad6d4e)

> **Diátaxis Type:** Explanation

## Decision

The candidate portfolio is empty under REQ-02. The measured logical-buffer
consumers provide no BitSlice copy-avoidance signal, so this branch selects
**no candidate and no adoption**. Production BitSlice and logical operations
remain unchanged. There is no private prototype, candidate sample, confirmation,
or benchmark-window queue line.

## Consumer and source evidence

The [frozen logical-buffer addendum](../../2037941f-profile-and-optimize-mid-range-buffer-operations/logical-buffer-addendum.md#preserved-no-win-and-scope-exclusions)
admits the isolated XOR, public row-XOR, NR construction, and ISA-L questions.
Its zero-copy rule permits a no-candidate disposition and requires a versioned
amendment before sampling any additional cell. The committed
[profile case inventory](../../../bench_results/2037941f/logical-profile/cases.txt),
[profile summary](../../../bench_results/2037941f/logical-profile/profile-summary.md),
and [consumer baselines](../../2037941f-profile-and-optimize-mid-range-buffer-operations/logical-baselines.md)
cover XOR, row-XOR, and NR construction; they contain no BitSlice copy-inclusive
consumer cell. The NR profile reports allocation symbols with display censoring,
which does not attribute those allocations to BitSlice copying.

The pinned [source ledger](survey/source-evidence.json) records the exact code
lines and the repository-crate callsite search. `BitSlice` holds crate-private
words plus an offset and logical length and exposes indexed bit access.
For a nonempty view, `BitVec::from_bitslice` allocates and copies bits. Its
callsites in the current crate sources are a core unit test and a BCH rustdoc
assertion. Neither is a measured production consumer. The BCH lane encoder's
similarly named bitslice algorithm reads `BitVec` words directly and does not
establish a `BitSlice` copy path.

No measured consumer therefore fixes a defensible copy-inclusive comparator,
offset/alignment domain, complexity budget, or whole-consumer improvement
threshold for this experiment. A synthetic copy microbenchmark would answer a
different question. The current addendum supplies no cell or comparison family
for it.

## Criterion disposition

- **REQ-01–03:** REQ-02's missing-signal stop applies before a prototype can be
  frozen under REQ-01. The empty portfolio authorizes no public borrowed-word
  method, shared representation, private production route, or source change.
- **REQ-04–06:** No prototype is admitted, so no prototype semantic result,
  copy-inclusive cost pair, adjusted interval, or equivalence claim is made.
  Existing BitSlice semantics remain as recorded in the pinned source ledger.
- **REQ-07–08:** The frozen adoption rules have no candidate evidence to
  evaluate. No private integration target qualifies; production remains
  unchanged, and no after measurement is applicable.
- **REQ-09:** This audit identifies no qualifying consumer that requires public
  word borrowing or shared infrastructure. The owner-decision branch is not
  triggered. Any later proposal for a new measured cell or public/shared route
  needs its own protocol amendment or separate authorization, respectively.

This is a missing-signal result, with no speedup or equivalence estimate.
