# Citation map for Rust comments

Issue `554c2935`. The map assigns a citation-registry key to every external
work that a comment or Rustdoc block of a tracked Rust file under `crates/` or
`dev/tools/` cites by title, author, report number or product name. A sweep
worker replaces each matched prose citation with `@/citation/<key>` and keeps
the section, algorithm, theorem, table or clause number as plain text after
the key. The registry is `.jit/references.toml`.

## Command

From any directory inside the repository:

```sh
python3 -B dev/active/fa787f85-documentation-overhaul/554c2935-citation-map.py \
  > dev/active/fa787f85-documentation-overhaul/554c2935-citation-map.txt
```

The script holds the `(key, phrase regex, path scope, veto)` table and exits
non-zero when a table key is absent from the registry.
`554c2935-citation-map.txt` is its output:

- Section 1 lists `key | regex | files:lines` for every table row.
- Section 2 lists the citations that have no key, each with the fact that
  blocks registration.
- Section 3 lists every comment line that matches the generic citation
  heuristic, holds no `@/citation/` address and matches neither table, with a
  one-line reason; a line without a reason prints as `UNEXPLAINED`.
- Section 4 counts section 3 by reason.

A row with a path scope maps an anaphoric reference ("the paper", a bare
theorem number, a continuation line) to the work its file cites by name.

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
- ETSI EN 302 755 maps to `Etsi2015` (V1.4.1), the edition
  `crates/gf2-coding/src/ldpc/dvb_t2/dvb_t2_matrices.rs` names.
- ETSI TS 102 831 maps to `Etsi2012` (V1.2.1), the edition
  `crates/gf2-coding/data/dvb_t2_tr102831_reference.toml` names. Comments that
  write "TR 102 831" cite the same document; ETSI publishes it as a TS.
- Comparison libraries map to the version `benchmarks/image.lock` pins:
  fflas-ffpack 2.5.0, LinBox 1.7.1, NTL 11.6.0, FLINT 3.5.0, M4RI 20260122;
  OpenBLAS maps to 0.3.21, the version `benchmarks/Containerfile` pins.
- Books map to the edition whose numbering the comment uses: `Higham2002`
  (chapter 14 is matrix inversion in the 2nd edition), `Warren2012` (section
  7-3 is "Transposing a Bit Matrix"). `HardyWright2008` and
  `CrandallPomerance2005` are the current editions; the citing comments name
  no edition.
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
- "AMD Zen 3 Software Optimization Guide" maps to `Amd2020`, the guide for
  Family 19h, the family of the Zen 3 core.
