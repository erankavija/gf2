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
| Run receipt | [`oracle-receipt.md`](oracle-receipt.md) |

Every number below is read out of those fixtures or out of the receipt. The
receipt carries the versions, the host, the revision the fixtures were
generated from, and the SHA-256 of each fixture and each generating source
file.

## Oracles

Both oracles run on every corpus row. A row an oracle cannot produce is a
blocking finding, not a recorded gap.

| Oracle | Entry point | Version |
|---|---|---|
| SageMath [SageMath2026] | `codes.BCHCode` | recorded in `sage.json`, field `oracle.version` |
| GAP [GapGroup2026] with GUAVA [Joyner2026] | `BCHCode(n, b, delta, F)` | recorded in `gap.json`, fields `oracle.gap_version`, `oracle.guava_version`, `oracle.sonata_version` |

The SageMath launcher of the pinned release forwards no arguments to a script,
so the oracle script is run under the interpreter that carries the SageMath
library and reads the library version at run time. The GAP script is invoked
with `-A` so no package beyond GUAVA and its SONATA dependency loads.

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
the selected splitting field. Both oracles are therefore run on gf2's $\alpha$
rather than on the root they would pick unaided, and gf2 is never adjusted to
match an oracle.

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
because its splitting field is its base field. For B4 the coincidence is threefold: the registry's
degree-16 selection, the Conway polynomial for $\mathrm{GF}(2^{16})$, and the
ETSI EN 302 755 normal-frame field polynomial are the same polynomial.
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
index 2. Its coefficient
canonicalization is still explicit: a $\mathrm{GF}(2^{8})$ symbol is written as
the integer whose bits are its polynomial-basis coordinates over the registry
modulus, which is GAP's `Coefficients(CanonicalBasis(GF(256)), e)` order
because GAP's canonical basis is $1, Z(2^{8}), \dots, Z(2^{8})^{7}$, and is
SageMath's `e.polynomial().list()` order because SageMath's $B$ carries the
same modulus.

## Where an oracle needed a non-default construction

Two situations arise. Both are recorded in `gap.json` per row rather than
worked around.

**GUAVA's default root differs from gf2's on N2 and N3.** GUAVA's `BCHCode`
builds on `PrimitiveUnityRoot(q, n)`, which is $Z(q^{s})^{(q^{s}-1)/n}$. On the
six other rows that element is exactly the transported gf2 root, and
`BCHCode(n, b, \delta, F)` therefore already constructs gf2's code. On N2 and
N3 it is a different element of order $n$: index 60 against 91, and index 14
against 16. For those rows the script runs GUAVA's own `BCHCode` generator
derivation, the cyclotomic-coset loop over `MinimalPolynomial(F, \alpha^{j})`
from `codegen.gi`, with `PrimitiveUnityRoot` replaced by the transported gf2
root, and wraps the result with GUAVA's `GeneratorPolCode`. The fixture keeps
both: `generator` is the polynomial at gf2's root, `bchcode_generator` is the
one GUAVA's unaided `BCHCode` returns, and `bchcode_generator_matches` says
whether they are equal. The suite asserts the implication that they agree
whenever the roots agree, which holds on all six coinciding rows.

**GUAVA cannot build a code object at the DVB-T2 mother length.** GUAVA's
`GeneratorPolCode` reaches `GeneratorMatrixFromPoly`, which materializes the
whole generator matrix. At B4 that matrix is $65343 \times 65535$ and exhausts
memory before any code object exists. The generator derivation inside `BCHCode`
runs normally at that size, so the B4 row records the generator that derivation
produces, encodes through the same cyclic-code polynomial map, and states the
limitation in `bchcode_unavailable`. The claim that skipping the wrapper
changes nothing is checked rather than assumed: on the seven rows where the
wrapper does build, the fixture records that its generator equals the one the
loop derived. GAP additionally verifies at B4 that the derived generator
divides $x^{n} - 1$, recorded as `generator_divides_x_n_minus_one`.

SageMath needed no non-default construction on any row. It accepts a
`primitive_root` argument directly, and its lazy code objects handle the mother
length without materializing a generator matrix.

## Standards vectors

The in-tree DVB-T2 verification material is the ETSI EN 302 755 [Etsi2015]
data committed under `crates/gf2-coding/src/bch/dvb_t2/`: the parameter table
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

The ETSI conformance streams `VV001-CR35_CSP` and its siblings, which carry
payload and parity test points TP04 and TP05, are not committed to this
repository. The existing tests that consume them are host-gated on
`DVB_TEST_VECTORS_PATH` and remain untouched. The committed ETSI material named
above is therefore the whole of the in-tree DVB-T2 evidence this suite can wire
in, and the four checks above are what it supports.

## Citations

Every work this document names resolves in `.jit/references.toml`:
[SageMath2026], [GapGroup2026], [Joyner2026], and [Etsi2015] for ETSI
EN 302 755 V1.4.1, the edition the in-tree parameter and generator tables cite.

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
./scripts/cargo-budget.sh --test cargo nextest run -p gf2-coding --all-features \
    --cargo-profile ci-test --profile ci -E 'binary(bch_oracle_agreement)'
```

Its thirteen cases run in about a quarter of a second in the `ci-test` profile,
so none carries an ignore tier. The heaviest, the codeword comparison at the
mother length $n = 65535$, measures 0.09 s in release.
