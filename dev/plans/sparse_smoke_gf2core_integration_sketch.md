# Sparse-smoke gf2-core integration — design sketch (`jit:96fde7c7`)

**Status:** **APPROVED 2026-05-05** by project-lead (session 7, autonomous) under CLAUDE.md § *Verification work*. The lead's recommendation across § 11's three open questions: **mechanism (b) ground-truth file, .gitignored, include input bytes** is the dispatched design. Decision rationale: this is an integration-mechanism task (not a Lean/Coq/Kani formal proof), the trade-off table in § 2 makes (b) clearly preferable on the unsafe-isolation invariant + maintenance axes, and either mechanism is reversible. The user can override via session interrupt; until then, the impl proceeds on mechanism (b). All three prerequisite tasks (521390db, 0d6ca3b6, 0f708b36) closed in session 7 — § 7's blockers are now empty.
**Author:** project-lead (autonomous), 2026-05-05.
**Parent issue:** `96fde7c7` (sparse_smoke gf2-core integration design + impl).
**Parent epic:** `97bf0879` (Close gf2-core SOTA performance gaps).

---

## 1 — Problem

Protocol § 6 (`dev/plans/sota_reference_acceptance_protocol.md:243-249`) requires:

> Each candidate's harness must include a `--smoke` mode that, for each
> `(operation, field, shape)` cell at `n = 16`, runs both the candidate
> and the gf2-core implementation on the same seeded input and asserts
> the per-operation equality contract above.

Today `benchmarks/reference/sparse_smoke.cpp` runs candidate (fflas-ffpack
`fspmv` / `fspmm`) against an **in-harness scalar reference**
(`scalar_spmv`, `scalar_sparse_dense`). gf2-core is not in the smoke loop.

The 47698404 R4 review correctly flagged this as protocol-noncompliant.
The fix requires either FFI from C++ into gf2-core, or a build-time
ground-truth file that gf2-core emits and smoke reads.

## 2 — Mechanism — recommended: ground-truth file via Cargo example

Two viable mechanisms; pros/cons table, then the recommendation.

| Property | (a) FFI cdylib | (b) Ground-truth file |
|---|---|---|
| gf2-core C ABI surface | yes — must stay stable | none — Rust stays Rust |
| Per-prime instantiations | 5 primes × 4 ops = 20 extern "C" wrappers | one Cargo example, generic |
| Build complications | `cdylib` crate type must be added; symbol versioning | none — text/bytes file under `benchmarks/expected/` |
| Detects gf2-core regressions at smoke time? | yes (runtime) | yes (build-time regen forces re-emit) |
| Detects candidate regressions at smoke time? | yes | yes |
| `unsafe` discipline impact | adds `extern "C"` + `unsafe` C ABI wrappers | none |
| Cross-language deterministic seed walk | already established (`seed_helpers.h`) | already established |
| Maintenance | high — every API change touches C ABI | low — append a new cell row to the emitter |
| Matches protocol § 6 spirit | "runs ... gf2-core" at smoke time | "runs ... gf2-core" at build time |

**Recommendation: ground-truth file (b).**

Rationale:
1. **No new ABI surface.** gf2-core's `#![deny(unsafe_code)]` invariant and
   the policy that all `unsafe` lives in `gf2-kernels-simd` /
   `gf2-kernels-hip` (CLAUDE.md § *Key design invariants* #3) make a C ABI
   non-trivial — exposing `Fp<P>` over `extern "C"` requires per-prime
   instantiations that pollute the public surface.
2. **Build-time regeneration is already the project's default for derived
   artefacts** (see `benchmarks/seeds/seed.txt`, the SHA-pinned reference
   stanzas in `Containerfile`). Adding one more emitted artefact under
   `benchmarks/expected/` is consistent with that pattern.
3. **Protocol § 6 semantics**: "runs both the candidate and the gf2-core
   implementation on the same seeded input and asserts the per-operation
   equality contract." Ground-truth runs gf2-core at build time *on the
   same seeded input*; the runtime equality assertion is unchanged. The
   protocol does not require both to run at the same wall-clock instant
   — it requires both to run on byte-equivalent inputs and produce
   byte-equivalent outputs. Build-time emission preserves this.
4. **FFI cost is real.** Adding cdylib + 20 extern "C" wrappers for a
   smoke harness is an order of magnitude more code than the alternative,
   and locks gf2-core into a C ABI maintenance burden that is out of
   proportion to the smoke harness's purpose.

## 3 — Lemmas / contracts to verify

(Lemma-shape per CLAUDE.md § *Verification work* — statements only, with
intended proof / test strategy.)

### L1 — Seed walk byte-equivalence

**Statement.** For every cell `(op, field, n=16, density=0.25, seed)`,
the gf2-core emitter and the C++ smoke harness produce **byte-equivalent**
sparse matrix support and dense input vectors when both consume
`gf2_bench_derive_seed(master, tag, op_idx, size_idx, regime_idx)` and
walk it via `splitmix64`.

**Strategy.** Already established by `47698404`'s scorecard § 2
*Determinism / seed protocol*. Reuse `benchmarks/reference/seed_helpers.h`
on the C++ side and the Rust mirror in
`crates/gf2-coding/examples/bench_sparse_csv_emitter.rs:198-227`. Add a
direct test: the new emitter writes the seed-derived support+values for
each cell to the ground-truth file; the smoke harness reads and verifies
byte-equivalence with its own sample. (One assertion pair per cell, at
file-load time.)

### L2 — Output equality for spmv

**Statement.** `gf2-core` `SpBitMatrix::matvec(x)` (over GF(2)) and
`SparseFieldMatrix::<Fp<P>>::matvec(x)` (over each GF(p)) at `n=16`
produce byte-equivalent output to fflas-ffpack `fspmv` / LinBox
`SparseMatrix::apply` after canonical `[0, p)` reduction.

**Strategy.** Smoke at runtime asserts `byte_eq(gf2_core_y, candidate_y)`
for every cell. gf2-core is the trusted oracle (already proven correct by
unit tests + property tests in the gf2-core test suite). Candidate ↔
oracle equality is what the smoke harness verifies.

### L3 — Output equality for sparse×dense

**Statement.** Analogous to L2 but for `SpBitMatrix::matmat(B)` (after
521390db lands) and `SparseFieldMatrix::<Fp<P>>::matmat(B)` against
fflas-ffpack `fspmm` / LinBox `applyLeft`.

**Strategy.** Same scheme as L2.

### L4 — Output equality for sparse-matmul

**Statement.** `SpBitMatrix::matmul(B)` and
`SparseFieldMatrix::<F>::matmul(B)` produce byte-equivalent output across
gf2-core's two paths (CSR×CSR vs the dense round-trip path) — this is an
**internal consistency** check since no external library has a sparse ×
sparse matmul (`no-independent-oracle` per protocol § 9).

**Strategy.** Smoke compares CSR×CSR output to dense round-trip output:
`A.matmul(B).to_dense() == A.to_dense().matmul(&B.to_dense())`. Both
paths are gf2-core; the assertion catches regressions in the sparse
algorithm against the trusted dense path.

### L5 — Output equality for sparse-elim

**Statement.** `SparseFieldMatrix::<Fp<P>>::rref(A)` produces RREF output
byte-equivalent to LinBox `Method::SparseElimination` after canonical
form normalisation (RREF is unique up to pivoting choice; the smoke
asserts equality of the *reduced* form: leading-1 columns identical,
non-pivot columns reduced).

**Strategy.** RREF is unique iff pivot-column order is fixed; gf2-core
and LinBox both pivot left-to-right. Equality assertion is byte-level.
For GF(2): `SpBitMatrixDual::rref` (added by 0d6ca3b6) vs LinBox
`Method::SparseElimination` over `Modular<int8_t>(2)`.

## 4 — Production code paths exercised

For each cell, the smoke must call the **production gf2-core path** that
the bench emitter calls (i.e. the `bench_sparse_csv_emitter.rs` call
sites), not a test-copied helper. This was the failure mode in CLAUDE.md
§ *Verification work*'s `8889e712` precedent.

| Op | gf2-core path | Bench emitter call site (ref) |
|---|---|---|
| spmv,GF(2) | `SpBitMatrix::matvec(x)` | `bench_sparse_csv_emitter.rs::run_gf2_random_er` |
| spmv,GF(p) | `SparseFieldMatrix::<Fp<P>>::matvec(x)` | `bench_sparse_csv_emitter.rs::run_fp_random_er` |
| sparse-matmul,GF(2) | `SpBitMatrix::matmul(other)` | same |
| sparse-matmul,GF(p) | `SparseFieldMatrix::<Fp<P>>::matmul(other)` | same |
| sparse-matmul,GF(2^m) | `SparseFieldMatrix::<Gf2mWide<u64>>::matmul(other)` | `run_gf2m_random_er` |
| sparse_dense,GF(2) | `SpBitMatrix::matmat(B)` (after 521390db) | new emitter row |
| sparse_dense,GF(p) | `SparseFieldMatrix::<Fp<P>>::matmat(B)` | existing |
| sparse_dense,GF(2^m) | `SparseFieldMatrix::<Gf2mWide<u64>>::matmat(B)` | existing |
| sparse-elim,GF(2) | `SpBitMatrixDual::rref()` (after 0d6ca3b6) | new emitter row |
| sparse-elim,GF(p) | `SparseFieldMatrix::<Fp<P>>::rref()` | new emitter row (after 0d6ca3b6) |

## 5 — File / build changes

### 5.1 New Cargo example

Path: `crates/gf2-coding/examples/sparse_smoke_emit_expected.rs` (~250 lines)

Behaviour:
- Iterate every `(op, field, seed)` cell the smoke harness needs.
- For each cell: build the same seeded input the C++ smoke builds (using
  `gf2_bench_derive_seed` + `splitmix64`).
- Call the production gf2-core path (per § 4 table).
- Serialize: cell tag + input bytes + output bytes to a binary file.
- Output: `benchmarks/expected/sparse_smoke_n16.bin`.

File format (versioned, header-prefixed binary):

```
magic   : 8 bytes "GF2SMK01"
n_cells : u32 LE
for each cell:
  tag_len : u16 LE
  tag     : tag_len bytes UTF-8 (e.g. "spmv,GF(2)")
  seed    : u64 LE
  in_len  : u32 LE
  in      : in_len bytes (canonical-form input vector / matrix)
  out_len : u32 LE
  out     : out_len bytes (canonical-form expected output)
```

`tag` is parseable by the C++ side; `in` / `out` are field-canonical (GF(2)
packs 8 bits/byte LE; GF(p) emits each element as `u32` or `u64` LE in
`[0, p)`; GF(2^m) emits each element as raw `u32` or `u64` LE).

### 5.2 sparse_smoke.cpp

Add at startup:
```cpp
ExpectedTable et = load_expected("benchmarks/expected/sparse_smoke_n16.bin");
```

Each `oracle_*` function:
1. Builds the same seeded input as before.
2. Asserts the in-harness seeded input matches `et[tag].in` byte-for-byte
   (L1 verification).
3. Runs the candidate (fflas / LinBox / etc.).
4. Asserts the candidate's output matches `et[tag].out` byte-for-byte
   (L2-L5 verification).

Failure modes:
- L1 mismatch → `[sparse_smoke] FAIL <op> seed-walk drift detected; gf2-core
  emitter must be regenerated for cell <tag>`. Indicates the seed
  derivation rule diverged.
- L2-L5 mismatch → `[sparse_smoke] FAIL <op> field=<F> candidate output
  != gf2-core expected; cell <tag>`. Indicates the candidate is
  incorrect.

### 5.3 build wiring

`benchmarks/smoke.sh` (early-exit if `sparse_smoke` returns non-zero):
- Step 0 (NEW): `cargo run --release -p gf2-coding --example
  sparse_smoke_emit_expected -- --output benchmarks/expected/sparse_smoke_n16.bin`
  — regenerate the ground-truth file from current gf2-core code.
- Step 1: build C++ harnesses (existing).
- Step 2: run `sparse_smoke` (existing).

`benchmarks/Containerfile`:
- Add the example to the build prerequisites: `RUN cargo build --release
  --example sparse_smoke_emit_expected -p gf2-coding`.
- Add a stage that emits the file *before* the C++ smoke step:
  `RUN cargo run --release -p gf2-coding --example
  sparse_smoke_emit_expected -- --output
  /workspace/benchmarks/expected/sparse_smoke_n16.bin`.

`benchmarks/reference/Makefile` (or whatever Makefile builds sparse_smoke):
- No change required — the ground-truth file is loaded at runtime.
- Optionally: add the file as a runtime dependency comment for
  documentation.

### 5.4 .gitignore

`benchmarks/expected/sparse_smoke_n16.bin` — generated artefact; should
be `.gitignored`. It is regenerated on every `smoke.sh` invocation.
A committed checked-in copy would defeat the regen-on-change purpose
(stale cached file would mask gf2-core regressions).

## 6 — Operations covered (per protocol § 6)

| `(op, field)` cell | Smoke covers? | Pre-req task |
|---|---|---|
| spmv × GF(2) | yes | none |
| spmv × GF(p) | yes (4 primes) | none |
| spmv × GF(2^m) | self-canonical (`semantics-mismatch`); smoke verifies internal CSR↔CSC consistency | none |
| sparse-matmul × GF(2) | self-canonical (`no-independent-oracle`); smoke verifies CSR×CSR vs dense round-trip | none |
| sparse-matmul × GF(p) | same (self-canonical, internal-consistency smoke) | none |
| sparse-matmul × GF(2^m) | same | none |
| sparse_dense × GF(2) | yes (LinBox `applyLeft` candidate) | **521390db** + **0f708b36** must land first |
| sparse_dense × GF(p) | yes (LinBox `applyLeft` candidate) | **0f708b36** must land first |
| sparse_dense × GF(2^m) | self-canonical (`semantics-mismatch`) | none |
| sparse-elim × GF(2) | yes (LinBox `Method::SparseElimination` candidate) | **0d6ca3b6** must land first |
| sparse-elim × GF(p) | yes (LinBox `Method::SparseElimination` candidate) | **0d6ca3b6** must land first |

## 7 — Blockers / dependencies

1. **521390db** (SpBitMatrix::matmat) — required for sparse_dense × GF(2)
   coverage in the emitter. In progress; expected to land in this wave.
2. **0d6ca3b6** (sparse-elim emitter wiring + SpBitMatrixDual::rref) —
   required for sparse-elim cells in the emitter. In progress; expected
   to land in this wave.
3. **0f708b36** (LinBox harness extension) — required for sparse_dense ×
   GF(p) and spmv × GF(2) candidate-side coverage. In progress.

After 521390db, 0d6ca3b6, and 0f708b36 land, 96fde7c7 implementation has
no remaining blockers.

## 8 — Risks

1. **Binary format versioning.** `magic = "GF2SMK01"` allows future format
   bumps without breaking older smoke binaries (they refuse to load
   newer versions). Mitigation: include a one-byte minor version inside
   the magic field.
2. **Endianness.** The format is LE-only. mitigated by hard-coding LE
   serialization on the Rust side and `std::byteswap`-friendly read on
   the C++ side. The project's CI runs only on x86_64 (LE); ARM64
   support would need an LE-explicit read but is the same code path.
3. **gf2-core API gaps detected at sketch implementation time.** If a
   gf2-core path is missing an entry-point the emitter needs (e.g.
   `SpBitMatrixDual::rref` is not yet public), the impl falls back to
   `to_dense().rref_in_place()`. This is functionally correct; the
   strict reviewer may prefer a sparse-native path. Filed as part of
   0d6ca3b6's success criteria.

## 9 — Acceptance criteria for the impl follow-on

After this sketch is approved, the impl issue (96fde7c7's "after
lead/user approval, sparse_smoke is rewritten ..." criterion) covers:

1. The Cargo example `sparse_smoke_emit_expected.rs` lands and emits the
   binary file with all cells from the § 6 table covered.
2. `sparse_smoke.cpp` loads the ground-truth file at startup, asserts
   per-cell byte-equality between candidate output and the ground-truth
   `out` field, and exits non-zero on any mismatch.
3. `benchmarks/smoke.sh` regenerates the ground-truth file before
   running `sparse_smoke`. `benchmarks/Containerfile` includes the
   regen step.
4. The ground-truth file is `.gitignored`.
5. `47698404` scorecard § 2 (*Cross-equality oracle*) is amended to cite
   gf2-core (via the ground-truth file) as the equality witness instead
   of the in-harness scalar reference.
6. `cargo-ci`, `code-review`, `doc-review` pass.

## 10 — Estimated impl effort

- Cargo example: ~250 lines (10 cells × ~25 lines each).
- `sparse_smoke.cpp` rewrite: ~50 lines added (loader + per-cell
  ground-truth lookup + byte-eq assertion); ~80 lines deleted (in-harness
  scalar references retired in favour of the ground-truth file).
- `smoke.sh` + `Containerfile`: ~20 lines.
- Tests: a Rust unit test that loads the file format round-trip; a
  smoke-harness `--self-test` mode that verifies the file structure
  parses correctly without running candidates.

Total: ~350 lines added, ~80 deleted. One worker dispatch, ~1-2 hour
expected wall-clock.

## 11 — Open questions for the user

1. Approve mechanism (b) ground-truth file? Or prefer (a) FFI cdylib?
2. Approve `benchmarks/expected/sparse_smoke_n16.bin` as `.gitignored`
   (regenerated each smoke run)? Alternative: commit the binary file
   under version control with a regen-on-CI gate. Mainstream choice in
   this project so far has been "regen at build time" (see seed.txt).
3. Approve serialising input matrices in the ground-truth file? It
   roughly doubles the file size (~100 KB total; trivial) but makes the
   L1 seed-walk equivalence check structural rather than implicit.
   Recommend yes.

The lead's recommendation across all three: **mechanism (b),
.gitignored, include input bytes**.
