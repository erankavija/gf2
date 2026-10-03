# Citation map for Rust comments

Issue `554c2935`. The map assigns a citation-registry key to the external
works that comments and Rustdoc blocks of tracked Rust files under `crates/`
and `dev/tools/` cite by title, author, report number or product name. A sweep
worker replaces each matched prose citation with `@/citation/<key>` and keeps
the section, algorithm, theorem, table or clause number as plain text after
the key. The registry is `.jit/references.toml`.

## Command

From the repository root or any subdirectory of it:

```sh
root="$(git rev-parse --show-toplevel)"
script="$root/$(git ls-files --full-name ':(top,glob)**/554c2935-citation-map.py')"
python3 -B "$script" > "${script%.py}.txt"
```

The output is byte-identical from the root and from `crates/gf2-core`. The
script holds the `(key, phrase regex, path scope, veto)` table and exits with
status 1 when a table key is absent from the registry.
`554c2935-citation-map.txt` is its output:

- Section 1 lists `key | regex | files:lines` for every table row. A row
  whose prose the sweep has replaced prints `-`.
- Section 2, "citations without a registry entry", lists by class the comment
  lines that cite a work for which no entry exists, with the reason.
- Section 3 lists every comment line that matches the generic citation
  heuristic, holds no `@/citation/` address and matches neither table, with a
  one-line reason; a line without a reason prints as `UNEXPLAINED`.
- Section 4 counts section 3 by reason.

A row with a path scope maps an anaphoric reference ("the paper", a bare
theorem number, a continuation line) to the work its file cites by name.

## What the listings establish

Section 1 establishes that each listed line cites a work whose key resolves in
the registry. Section 2 establishes which lines cite a work without an entry;
the comment sweep removes those source pointers, and each class prints `-`
when none remains. Every cited work that the table and the heuristic detect
maps to a key exactly when section 2 holds no line and section 3 holds no
`UNEXPLAINED` line. The heuristic is a pattern list; a citation in a form it
does not match appears in no section.

The section 2 classes:

- Condo et al. (2022), "Fixed Complexity Soft-Output GRAND": Crossref holds no
  work with this title and venue; the attribution rests on `Yuan2025`.
- Knuth, MMIX: the multiplier 6364136223846793005 is attributed to Knuth's
  MMIX generator in *The Art of Computer Programming*, volume 2, 3rd edition,
  by Steele and Vigna (doi:10.1002/spe.3030); the table and line of that
  volume are not checked against the volume itself.
- Shoup §12.4: no edition of the book places linearly generated sequences in
  chapter 12.
- "IEEE AES standard" in `crates/gf2-core/src/primitive_polys.rs`: AES is
  FIPS 197, and the GF(2^8) entry of that file's database is not the AES
  polynomial.
- "the paper" in `crates/gf2-coding/src/fading.rs` and
  `crates/gf2-coding/examples/ldpc_bler_check.rs`: neither file names the
  paper.

## Scope rule

A citation names a work: an author with a year, a title, a report or standard
number, an archive identifier, a section of a named book, or a software
library to which the comment attributes a method, a constant or a measured
number. An eponym that names a method without a year, title or section
(Barrett, Montgomery, Karatsuba, Keller–Gehrig, Wiedemann, Markowitz,
Berlekamp–Massey, Box–Muller, Clopper–Pearson, Wilson, Welford) is a method
name and has no row.

## Edition rule

One key identifies one edition or version.

- 3GPP TS 38.212: a line citing clause 5.4.2.2 maps to `ThreeGpp2020`
  (V16.4.0), the edition `crates/gf2-coding/data/ldpc/nr_5g/PROVENANCE.md`
  records for the interleaver. Every other TS 38.212 citation maps to
  `ThreeGpp2017` (V15.0.0), the edition the registry records for the
  base-graph tables; no comment names another edition.
- 3GPP TS 38.214 maps to `ThreeGpp2017a` (V15.0.0), the Release 15 edition of
  the same date as the TS 38.212 pin; its Tables 5.1.3.1-1 and 5.1.3.1-2 carry
  the modulation order `Q_m` that the citing comment states.
- ETSI EN 302 755 maps to `Etsi2015` (V1.4.1), the edition
  `crates/gf2-coding/src/ldpc/dvb_t2/dvb_t2_matrices.rs` names.
- ETSI TS 102 831 maps to `Etsi2012` (V1.2.1), the edition
  `crates/gf2-coding/data/dvb_t2_tr102831_reference.toml` names. Comments that
  write "TR 102 831" cite the same document; the V1.2.1 cover reads "ETSI TS".
- The CCSDS rate-1/2, constraint-length-7 convolutional code maps to
  `Ccsds2017` (CCSDS 131.0-B-3), the issue that
  `crates/gf2-coding/examples/nasa_rate_half_k3.rs` names in its printed
  reading list; section 3.3 of that issue specifies the code.
- The AES field polynomial maps to `Nist2001` (FIPS 197, 2001), the
  publication whose section 4.2 was checked for the polynomial; no comment
  names an edition.
- Comparison libraries map to the version `benchmarks/image.lock` pins:
  fflas-ffpack 2.5.0, LinBox 1.7.1, NTL 11.6.0, FLINT 3.5.0, M4RI 20260122;
  OpenBLAS maps to 0.3.21, the version `benchmarks/Containerfile` pins.
- Books map to the edition whose numbering the comment uses: `Higham2002`
  (chapter 14 is matrix inversion in the 2nd edition), `Warren2012` (section
  7-3 is "Transposing a Bit Matrix"). `HardyWright2008` and
  `CrandallPomerance2005` are the 6th and 2nd editions; the comments that
  cited them named no edition.
- `DumasPernet2012` section, algorithm, theorem and table numbers follow
  arXiv:1204.3735v1.

## Differences between a comment and its registry entry

- `crates/gf2-core/src/field/ple.rs` titles the Dumas–Pernet chapter
  "Polynomial-time matrix algorithms over finite fields, 2010"; the numbering
  it cites is that of `DumasPernet2012`.
- `crates/gf2-core/src/field/ple.rs` gives arXiv:1703.02438 for
  Dumas–Pernet–Sultan; that identifier belongs to an unrelated paper, and
  `DumasPernetSultan2017` carries arXiv:1601.01798.
- `crates/gf2-coding/src/grand/mod.rs` gives IEEE ISIT as the venue of
  Solomon, Duffy and Médard; `Solomon2020` records IEEE ICC 2020.
- "Granlund-Möller" maps to `MollerGranlund2011`; Möller is the first author.
- "Menezes et al. (1996)" maps to `Menezes1997`, the publisher's copyright
  year; "Lidl & Niederreiter (1997)" maps to `LidlNiederreiter1996`, the
  publisher's publication year of the 2nd edition.
