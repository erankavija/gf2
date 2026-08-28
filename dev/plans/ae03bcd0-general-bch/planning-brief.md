# General BCH Epic Planning Brief

> **Diátaxis Type:** Explanation (planning input for epic `ae03bcd0`)

This brief records the planning-interview decisions for **Harden and generalize
BCH codes over finite fields** that the epic contract does not carry. Breakdown
of the epic consumes this brief together with the epic description; where the
two overlap, the epic description is canonical.

## Construction semantics

- **Root seed sets are inputs, closure is derived.** The construction API
  accepts a seed set that is not required to be closed under $q$-cyclotomic
  conjugacy and computes the closure itself. The epic's REQ-03 phrase "seed
  sets closed under $q$-cyclotomic conjugacy" describes the derived defining
  set, never a precondition on user input.
- **Non-primitive lengths accept both root conventions.** Callers may supply
  only $n$ and let the library derive a canonical element of order $n$, or
  supply an explicitly chosen $n$-th root of unity.
- **Designed distance is a bound, terminologically.** The public API and
  documentation present the witnessed classical bound as a guaranteed lower
  bound on minimum distance and never label it the actual minimum distance
  unless that value is independently established.
- **Stronger defining-set bounds are excluded.** Hartmann–Tzeng and Roos
  bounds stay outside the epic; they are candidates for a later issue but are
  not among the tracked follow-ups REQ-16 requires.

## API shape

The agreed shape is a hybrid of a semantic specification type and generic
derived-code wrappers:

- `BchSpec` is the canonical construction description with one variant per
  independent-input construction (primitive narrow-sense, primitive with
  arbitrary first root, consecutive roots at non-primitive length, seed set,
  explicit generator polynomial).
- `BchCode::construct(spec)` is the single validation and derivation path;
  convenience constructors build the corresponding `BchSpec` and delegate.
- Shortening, puncturing, and extension are generic wrapper types over any
  linear code, never flags inside `BchCode`.
- Derived-code transformations offer count-based conveniences for the
  conventional systematic positions (e.g. shorten-by-count) alongside the
  explicit arbitrary-coordinate forms; both retain the coordinate map REQ-05
  requires.
- The opt-in decoder diagnostic path exposes the corrected codeword, decoded
  message, correction positions, and correction count as structured evidence;
  the fast path stays status-plus-count.

## Encoding architecture

Batch encoding selects among mathematically equivalent algorithm families —
polynomial remainder, precomputed linear recurrences, and matrix-based
kernels — through profile-driven dispatch, while one scalar reference path is
preserved for every family. This dispatch decision is separate from, and
composes with, the AVX2-versus-scalar fallback REQ-13 covers.

## Breakdown wiring

- The field-generic block-code trait cutover becomes its own child issue, and
  story `3931ac6f` (streaming/batch trait unification) depends on that child
  rather than on the whole epic. No direct edge between `3931ac6f` and the
  epic exists or should be added.
- The epic defines its own canonical typed error surface; it takes no
  dependency on story `1b929ce5` (crate-wide typed-error migration).
- Prerequisite `3243bc1f` (BCH property tests) keeps its current contract for
  now; after the canonical construction path exists, its criteria expand into
  the shared BCH property suite with nonbinary construction coverage.
- Prerequisite `88ca7d2f` (BCH throughput benchmarks) likewise expands to
  cover the external-baseline survey, large-batch encoding, generator-matrix
  materialization, and committed SOTA comparison receipts.

Neither prerequisite has been amended yet; the expansions land as issue
updates during or after breakdown, once the canonical interfaces they target
exist.
