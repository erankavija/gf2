# External oracle and standards-vector agreement for the BCH corpus

> **Diátaxis Type:** Reference

This document records how the predeclared BCH conformance corpus is checked
against two external oracles, what each oracle was given, where its field
presentation coincides with gf2's and where an explicit isomorphism carries the
construction across, and which authoritative standards vectors the suite wires
in. The run that produced the committed fixtures is recorded in
[`oracle-receipt.md`](oracle-receipt.md).

## Sources of truth

| Artifact | Path |
|---|---|
| Corpus and evidence protocol | [`plan.md`](plan.md), section `evidence-protocol` |
| Corpus construction | `gf2_coding::test_support::visit_bch_corpus` |
| Corpus emitter | [`../../../crates/gf2-coding/examples/bch_oracle_messages.rs`](../../../crates/gf2-coding/examples/bch_oracle_messages.rs) |
| SageMath oracle | [`oracle/sage_oracle.py`](oracle/sage_oracle.py) |
| GAP/GUAVA oracle | [`oracle/gap_oracle.g`](oracle/gap_oracle.g) |
| Runner | [`oracle/run.sh`](oracle/run.sh) |
| Corpus fixture | [`../../../crates/gf2-coding/tests/data/bch_oracle/corpus.json`](../../../crates/gf2-coding/tests/data/bch_oracle/corpus.json) |
| SageMath fixture | [`../../../crates/gf2-coding/tests/data/bch_oracle/sage.json`](../../../crates/gf2-coding/tests/data/bch_oracle/sage.json) |
| GAP/GUAVA fixture | [`../../../crates/gf2-coding/tests/data/bch_oracle/gap.json`](../../../crates/gf2-coding/tests/data/bch_oracle/gap.json) |
| Agreement suite | [`../../../crates/gf2-coding/tests/bch_oracle_agreement.rs`](../../../crates/gf2-coding/tests/bch_oracle_agreement.rs) |
| Stream reader | [`../../../crates/gf2-coding/tests/test_vectors/mod.rs`](../../../crates/gf2-coding/tests/test_vectors/mod.rs) |
| ETSI verification streams | host data under `VV001-CR35_CSP/TestPoint04` and `TestPoint05`, located by `DVB_TEST_VECTORS_PATH` |
| Run receipt | [`oracle-receipt.md`](oracle-receipt.md) |

Every number below is read out of those fixtures or out of the receipt. The
receipt carries the versions, the host, the revision the fixtures were
generated from, and the SHA-256 of each fixture and each generating source
file.

## Oracles

Both oracles run on every corpus row. A row an oracle cannot produce is a
blocking finding, not a recorded gap.

| Oracle | Entry point | Identity |
|---|---|---|
| SageMath [SageMath2026] | `codes.BCHCode` | `sage.json`, object `oracle` |
| GAP [GapGroup2026] with GUAVA [Joyner2026] | `BCHCode(n, b, delta, F)` | `gap.json`, object `oracle` |

A version label alone does not name a build, so each oracle records what it
observes about itself and the runner turns that into an exact identity in the
receipt's **Oracle identity** section: for SageMath the library version, the
interpreter and its executable path, and the SHA-256 of that executable; for
GAP the kernel and build version, the build datetime, the architecture, the
GMP version, the heap the run used, the SHA-256 of the `gap` binary actually
invoked, and the SHA-256 of the `PackageInfo.g` of the GUAVA and SONATA
installations that loaded. `the_oracle_fixtures_record_their_identities_and_self_checks`
asserts that every one of those members reached the fixture.

The SageMath launcher of the pinned release forwards no arguments to a script,
so the oracle script is run under the interpreter that carries the SageMath
library and reads the library version at run time. The GAP script is invoked
with `-A` so no package beyond GUAVA and its SONATA dependency loads, and with
`-T` so a code-object attempt that exceeds the heap returns to the script.

## The corpus

All rows are narrow sense with first consecutive root $\alpha^{1}$, and all use
the default `[message | parity]` systematic layout. Splitting-field moduli come
from the deterministic registry and search policy in
`gf2_core::field::modulus_select`.

| Row | Base field | $n$ | $\delta$ | Splitting field | $k$ | $\deg g$ |
|---|---|---|---|---|---|---|
| B1 | $\mathrm{GF}(2)$ | 15 | 7 | $\mathrm{GF}(2^{4})$ | 5 | 10 |
| B2 | $\mathrm{GF}(2)$ | 127 | 21 | $\mathrm{GF}(2^{7})$ | 64 | 63 |
| B3 | $\mathrm{GF}(2)$ | 255 | 9 | $\mathrm{GF}(2^{8})$ | 223 | 32 |
| B4 | $\mathrm{GF}(2)$ | 65535 | 25 | $\mathrm{GF}(2^{16})$ | 65343 | 192 |
| N1 | $\mathrm{GF}(3)$ | 13 | 5 | $\mathrm{GF}(3^{3})$ | 4 | 9 |
| N2 | $\mathrm{GF}(5)$ | 31 | 4 | $\mathrm{GF}(5^{3})$ | 22 | 9 |
| N3 | $\mathrm{GF}(9)$ | 10 | 3 | $\mathrm{GF}(3^{4})$ | 6 | 4 |
| N4 | $\mathrm{GF}(2^{8})$ | 255 | 33 | $\mathrm{GF}(2^{8})$ | 223 | 32 |

The $k$ and $\deg g$ columns are read from `corpus.json`, and
`the_corpus_fixture_records_the_constructed_rows` asserts them against the
codes the construction derives; the remaining columns are the protocol's own
predeclared inputs.

B4 is the DVB-T2 normal-frame mother code. Its field polynomial, its
correction radius $t = 12$, and hence $\delta = 2t + 1 = 25$ come from
`DvbBchParams::for_code(FrameSize::Normal, ...)`, which carries ETSI EN 302 755
Table 6b [Etsi2015]. Its redundancy $\deg g = 192$ is the standard's normal-frame
BCH parity length. N4 is the Reed-Solomon row: its splitting field is its symbol
field, so its extension witness is the degree-one extension of the selected
$\mathrm{GF}(2^{8})$.

## Canonical index

Every field element is written as its canonical index: the integer whose
base-$p$ digits are the element's canonical prime coordinates, coordinate zero
least significant. That is the numbering `FieldIdentity::write_prime_coords`
defines, so it names a property of the field presentation rather than of a
carrier type. For a tower the coordinates group by relative degree, so a
splitting-field index decomposes into base-field digits in base $|B|$.

A sequence of base-field symbols is written as a lowercase hex string. Over
$\mathrm{GF}(2)$ symbols are bit-packed under the repository's canonical
little-endian bit indexing, symbol $i$ occupying bit $i \bmod 8$ of byte
$\lfloor i/8 \rfloor$ with the last byte zero-padded. Over every larger base
field one symbol occupies one byte holding its canonical index, which the corpus
admits because no row's base field exceeds 256 elements.

## Messages

Messages are drawn per row from the predeclared seed `0xAE03BCD0`, so a row
reproduces independently of the rows before it.

| Property | Value |
|---|---|
| Generator | `rand::rngs::StdRng`, the repository's standard seeded test generator |
| Crate version | `rand` 0.8.5 from `Cargo.lock`, whose `StdRng` is ChaCha12 from `rand_chacha` 0.3.1 |
| Seeding | `SeedableRng::seed_from_u64(0xAE03BCD0)`, a fresh generator per row |
| Draw | `gen_range(0..base_order)`, one canonical base-field index per symbol, messages filled in order |
| Length | the row's derived $k$ |
| Count | four messages per row, two for a row longer than 4096 |

`the_corpus_messages_regenerate_from_the_predeclared_seed` redraws them from
the seed and asserts equality with the fixture, which is the determinism
witness.

## What agreement means

A BCH code is determined by $(q, n, b, \delta)$ **and** the primitive $n$-th
root of unity $\alpha$: a different $\alpha$ gives a different, equivalent
generator polynomial. gf2 takes $\alpha$ as the deterministic element of exact
order $n$ derived by `element_of_exact_order` from the canonical generator of
the selected splitting field. Each oracle is run on gf2's $\alpha$, and gf2 is
never adjusted to match an oracle. GUAVA's `BCHCode` also picks a root of its
own, and the section below compares that code object against a gf2 code built
on the root it picked.

Per row the suite checks four equalities against each oracle.

1. The oracle's generator polynomial equals gf2's, coefficient by coefficient
   in canonical base-field indices.
2. The oracle's dimension equals gf2's $k$, and its defining set equals gf2's
   closed defining set.
3. The oracle's own encoding of each message is a word of the gf2 code, its
   remainder modulo gf2's generator being zero, and equals gf2's polynomial
   product $m(x)\,g(x)$ coefficient by coefficient.
4. The oracle's systematic codeword for the information set gf2 uses, namely
   $x^{r}m(x) - (x^{r}m(x) \bmod g)$ computed with the oracle's own generator
   and polynomial arithmetic, equals gf2's default-layout encoding after the
   layout's coordinate map takes user coordinate $u$ to polynomial degree
   $(u + n - k) \bmod n$.

All four hold on all eight rows for both oracles. No disagreement is recorded,
because none was observed.

The oracle encoder used for equality 3 is SageMath's
`CyclicCodePolynomialEncoder` and GUAVA's `CodewordVector`, which is the map
$c(x) = m(x)\,g(x)$ that a GUAVA cyclic code defines. The GAP fixture records
per row whether that map was additionally cross-checked against a GUAVA
`GeneratorPolCode` code object built at gf2's root, in field
`native_encoder_cross_checked`.

## Presentation and root reconciliation

`gf2_core::field::modulus_select` prefers a registry entry and otherwise
searches candidates in a fixed rank order. SageMath and GAP both present
$\mathrm{GF}(p^{d})$ by its Conway polynomial, SageMath as the default modulus
of `GF(p^d)` and GAP as the defining polynomial of `Z(p^d)`. Six of the eight
rows land on a presentation that coincides with that Conway one; two do not and
are carried across by an explicit isomorphism.

| Row | gf2 splitting-field modulus | Coincides with Conway | $\alpha$ (gf2 index) | Reconciliation |
|---|---|---|---|---|
| B1 | $x^{4} + x + 1$ | yes | 2 | coincides; $\alpha = x = Z(2^{4})$ |
| B2 | $x^{7} + x + 1$ | yes | 2 | coincides; $\alpha = x = Z(2^{7})$ |
| B3 | $x^{8} + x^{4} + x^{3} + x^{2} + 1$ | yes | 2 | coincides; $\alpha = x = Z(2^{8})$ |
| B4 | $x^{16} + x^{5} + x^{3} + x^{2} + 1$ | yes | 2 | coincides; $\alpha = x = Z(2^{16})$ |
| N1 | $x^{3} + 2x + 1$ | yes | 9 | coincides; $\alpha = x^{2} = Z(3^{3})^{2}$ |
| N2 | $x^{3} + x + 1$ | no, Conway is $x^{3} + 3x + 3$ | 20 | mapped by isomorphism |
| N3 | $z^{2} + (1 + y)$ over $\mathrm{GF}(9) = \mathrm{GF}(3)[y]/(y^{2}+1)$ | not comparable, gf2 uses a tower | 43 | mapped by isomorphism |
| N4 | $x^{8} + x^{4} + x^{3} + x^{2} + 1$, base equals splitting field | yes | 2 | coincides; $\alpha = x = Z(2^{8})$ |

The evidence for each "coincides" row is in the fixtures rather than in this
prose: `sage.json` records SageMath's own `splitting_field_modulus` together
with `splitting_field_matches_gf2`, and `gap.json` records
`splitting_presentation` as `conway` exactly when gf2's splitting-field modulus
equals GAP's `ConwayPolynomial(p, d)`, and `base_presentation` as `conway` on
the same test for the base field, which is where N4's coincidence is recorded
because its splitting field is its base field.

For B4 the coincidence is threefold: the registry's degree-16 selection, the
Conway polynomial for $\mathrm{GF}(2^{16})$, and the ETSI EN 302 755
normal-frame field polynomial are the same polynomial.
`the_registry_selects_the_etsi_normal_frame_field_polynomial` asserts the
registry half of that.

## The isomorphism for N2 and N3

Let $B$ be the base field, $E = B[z]/(m_E)$ gf2's splitting field, and $F$ the
oracle's field of the same order.

The oracle first fixes an embedding $\iota$ of the base field. SageMath builds
$B$ with gf2's own defining polynomial, so base coordinates agree by
construction, and takes $\iota = \mathrm{Hom}(B, F)[0]$, the embedding
`codes.BCHCode` itself uses. GAP admits no defining polynomial other than the
Conway one, so its script maps gf2's base generator $y$ to the root of gf2's
base modulus in $\mathrm{GF}(q)$ of least canonical index, and inverts that map
when writing base-field coordinates back out.

The embedding then extends to $\Phi : E \to F$ by sending $z$ to a root $Z_{0}$
of $\iota(m_E)$ in $F$ and extending $B$-linearly. $\Phi$ is a field
isomorphism because $\iota(m_E)$ is irreducible over $\iota(B)$ and $Z_{0}$ is
one of its roots. Both scripts fix $Z_{0}$ as the root of least canonical index
and record which root that is.

Which root is chosen does not affect any recorded quantity. Two choices differ
by an element $\tau$ of $\mathrm{Gal}(F/\iota(B))$; the transported roots then
differ by $\tau$, the root sets differ by $\tau$, and the generator
polynomial's coefficients lie in $\iota(B)$, which $\tau$ fixes pointwise. The
codewords likewise lie in the base field. The choice is therefore a
presentation detail, recorded for auditability rather than relied on.

Each script checks the transport it built rather than assuming it:
`sage.json` records `transport_checks.ext_modulus_vanishes`, that the chosen
root satisfies the transported modulus, and `transport_checks.alpha_order`,
that the transported $\alpha$ has exact order $n$; the GAP script raises an
error and produces no fixture if either fails. The suite case
`the_oracle_fixtures_record_their_versions_and_self_checks` asserts that the
recorded order equals the row's $n$.

For N2 both oracles independently chose the same isomorphism, sending gf2's
$x$ to the element of canonical index 64 in $\mathrm{GF}(125)$ and gf2's
$\alpha$ to index 91. For N3 both sent gf2's $z$ to index 50 in
$\mathrm{GF}(81)$ and gf2's $\alpha$ to index 16; SageMath additionally records
the image of gf2's $y$ at index 74 of $\mathrm{GF}(81)$, GAP at index 4 of
$\mathrm{GF}(9)$. The agreement of two independent transports is not required
by the argument above, but it is what both fixtures record.

N4 needs no root transport, because its splitting field is its base field and
gf2's $\mathrm{GF}(2^{8})$ presentation is the Conway one. `sage.json` records
that SageMath's `Hom(B, B)[0]` is the identity on this row rather than a
Frobenius power, by recording the image of the base generator at its own
index 2.

Its coefficient canonicalization is still explicit: a $\mathrm{GF}(2^{8})$
symbol is written as the integer whose bits are its polynomial-basis
coordinates over the registry modulus, which is GAP's `Coefficients(CanonicalBasis(GF(256)), e)` order
because GAP's canonical basis is $1, Z(2^{8}), \dots, Z(2^{8})^{7}$, and is
SageMath's `e.polynomial().list()` order because SageMath's $B$ carries the
same modulus.

## GUAVA's two witnesses

GUAVA's `BCHCode(n, b, delta, F)` builds on `PrimitiveUnityRoot(q, n)`, which
is $Z(q^{s})^{(q^{s}-1)/n}$, so its root is GUAVA's own choice rather than an
argument. Each row therefore carries two GUAVA results in `gap.json`, and both
are compared against a gf2 code.

**The unaided `BCHCode` object.** `bchcode_generator`, `bchcode_k`,
`bchcode_defining_set`, `bchcode_codewords_native` and
`bchcode_codewords_systematic` come from the object `BCHCode(n, b, delta, F)`
returns: `GeneratorPol` and `Dimension` read off that object, the defining set
as the exponents of GUAVA's own root that its generator annihilates, and the
codewords from `CodewordVector` on it together with the systematic reduction
$x^{r}m(x) - (x^{r}m(x) \bmod g)$ against its generator. `guava_root_gf2_index`
writes GUAVA's root back in gf2's coordinates, and
`guava_bchcode_objects_agree_at_their_own_root` compares each of them with the
gf2 code carrying that root: the corpus row itself where the two roots
coincide, and, on N2 and N3, a `BchSpec::NonPrimitiveConsecutive` construction
with `RootSelection::Explicit` on GUAVA's root, which is gf2 index 6 against
$\alpha = 20$ on N2 and 13 against $\alpha = 43$ on N3.

**The derivation at gf2's root.** `generator`, `defining_set`, `k` and
`codewords_*` come from GUAVA's `BCHCode` derivation with `PrimitiveUnityRoot`
replaced by the transported gf2 root: the cyclotomic-coset loop over
`MinimalPolynomial(F, alpha^j)` from `codegen.gi`, wrapped by GUAVA's
`GeneratorPolCode`. `bchcode_generator_matches` records whether the two
witnesses derive the same generator, which they do exactly where the two roots
coincide.

The inverse of the isomorphism is validated before `guava_root_gf2_index` is
read out of it: the script carries gf2's own $\alpha$ back through it as
`alpha_gf2_index` and stops with an error unless that reproduces the corpus's
`alpha`.

**Whether a row has a code object is an observation, not a threshold.** GUAVA's
`BCHCode` reaches `GeneratorPolCode`, which materializes the whole generator
matrix through `GeneratorMatrixFromPoly`, so at a long length the object can
need more heap than the run has: at B4 that matrix is $65343 \times 65535$.
The script therefore calls `BCHCode` on every row through `CALL_WITH_CATCH`
under `-T`, where exceeding the heap `-o` sets returns to the caller instead of
ending the run. Each row records the outcome as `bchcode_built` beside the heap
the attempt ran under, its processor time, and the process's peak resident set
size after it, read from the kernel. `ORACLE_GAP_HEAP` sets that heap and the
receipt tabulates every attempt together with the diagnostics GAP printed.

A row whose attempt does not yield a code object still carries a GUAVA result:
its `BCHCode` generator derivation, and its codewords through GUAVA's
cyclic-code polynomial map $c(x) = m(x)\,G(x)$.
`a_row_without_a_guava_code_object_records_the_derivation_and_the_attempt`
asserts exactly that shape, so there is no state in which a row simply has no
GUAVA result. That the map agrees with a code object where one exists is
checked rather than assumed: `native_encoder_cross_checked` records, on every
row whose wrapper does build, that encoding through the GUAVA code object gives
the same word as the polynomial map. GAP verifies on every row that the derived
generator divides $x^{n} - 1$ and that the defining set is exactly the set of
exponents its generator annihilates, recorded as
`generator_divides_x_n_minus_one` and `defining_set_root_checked`.

SageMath needs no non-default construction on any row. It accepts a
`primitive_root` argument directly, and its lazy code objects handle the mother
length without materializing a generator matrix.

## Standards vectors

Two authoritative ETSI sources are wired in: the committed EN 302 755
[Etsi2015] tables, and the DVB-T2 verification and validation reference
streams, which are host data rather than repository content.

### The committed ETSI tables

The in-tree DVB-T2 material is the ETSI EN 302 755 [Etsi2015] data committed
under `crates/gf2-coding/src/bch/dvb_t2/`: the parameter table
`DvbBchParams::for_code`, carrying Tables 6a and 6b, and the generator-polynomial
table `NORMAL_GENERATORS`, carrying $g_{1}(x)$ through $g_{12}(x)$ for the
normal frame. The suite wires both in.

- `the_registry_selects_the_etsi_normal_frame_field_polynomial` asserts that
  the deterministic degree-16 registry selection is the field polynomial the
  ETSI parameter table pins.
- `the_mother_generator_is_the_etsi_generator_product` asserts that the
  canonical mother code's generator is the product $g_{1} \cdots g_{12}$ of the
  ETSI table, of degree equal to the standard's normal-frame parity length.
- `both_oracles_reproduce_the_etsi_generator_product` asserts that the
  generator each oracle derived for B4 is that same product.
- `the_shortened_dvb_t2_parity_matches_the_mother_code` encodes seeded payloads
  of the standard's $K_{\mathrm{bch}}$ through the ETSI-generator-driven
  shortened encoder and through the canonical mother code with zero symbols in
  the shortened positions, and asserts the parity is equal. Shortening prepends
  $K - K_{\mathrm{bch}}$ zero symbols at the high message coordinates of the
  descending transmission layout, which leaves the message polynomial and hence
  the parity unchanged.

### The DVB-T2 verification streams

The DVB-T2 verification and validation working group publishes modulator
reference streams produced on the Common Simulation Platform, one file per test
point of a named configuration. Set `VV001-CR35` is the normal frame at code
rate 3/5; its test point 04 carries the BCH input blocks and test point 05 the
BCH output blocks. The streams are distributed with the standard's conformance
material rather than committed here, so the suite locates them through
`test_vectors_path()`, which reads `DVB_TEST_VECTORS_PATH` and otherwise falls
back to `dvb_test_vectors` under the invoking user's home directory. The test
prints the path it resolved.

`the_etsi_dvb_t2_streams_encode_to_their_verified_codewords` loads the set
through `TestVectorSet::load` and, for every block of every frame, encodes the
test point 04 payload through the **canonical model** — the mother
`BinaryBchCode` of `DvbBchParams::for_code(FrameSize::Normal, ...)` at the code
rate the set's configuration name fixes, zero symbols at the shortened message
coordinates, and the standard's descending transmission layout — then asserts
the resulting codeword equals the test point 05 block bit for bit, message bits
against test point 04 and parity bits against the mother code's. The test
prints the resolved path and returns when the streams are absent, so a host
without them runs the rest of the suite.

The comparison is exhaustive rather than sampled, which Amendment 1 of the
plan's `evidence-protocol` section predeclares. The run behind this document
observed:

| Property | Value |
|---|---|
| Stream set | `VV001-CR35`, normal frame, rate 3/5 |
| Frames compared | 4 |
| Blocks per frame | 202 |
| Blocks compared | 808 |
| Blocks agreeing bit for bit | 808 |
| Test point 04 SHA-256 | `c658dc04cacebe24a86a42a89f8ffe588f1e269506b6da05eae7d6582d8570b8` |
| Test point 05 SHA-256 | `f4aaf105b01768b1269d21accef923de09d0040cba073a73909d8f814f3ed929` |

The frame and block counts are asserted, so a truncated copy of the streams
fails the case rather than comparing fewer blocks in silence.

## Citations

Every work this document names resolves in `.jit/references.toml`:
[SageMath2026], [GapGroup2026], [Joyner2026] and [Etsi2015].

The DVB-T2 verification streams are identified by their set name and by the
SHA-256 of each file in the table above, which is what the run observed on the
host the receipt names.

## Reproduction

```bash
dev/active/ae03bcd0-general-bch/oracle/run.sh
```

The runner builds the corpus emitter, runs both oracles over the corpus it
writes, and rewrites [`oracle-receipt.md`](oracle-receipt.md) from what the run
observed. Re-running from the revision the receipt names rewrites the three
fixtures byte for byte, so `git status` reports only the receipt's own
timestamps.

The suite is then

```bash
DVB_TEST_VECTORS_PATH=<the directory holding VV001-CR35_CSP> \
  ./scripts/cargo-budget.sh --test cargo nextest run -p gf2-coding --all-features \
    --cargo-profile ci-test --profile ci -E 'binary(bch_oracle_agreement)'
```

No case carries an `#[ignore]` tier, so the fast tier runs the suite whole and
the fast tier's budgets are the ones that apply to it.
