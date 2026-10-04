# Citation map for Rust comments

Issue `554c2935`. The map assigns a citation-registry key to each external
work that a comment or Rustdoc block of a tracked Rust file under `crates/` or
`dev/tools/` cites, and lists the citing files. The registry is
`.jit/references.toml`.

## Command

From the repository root or any subdirectory of it:

```sh
root="$(git rev-parse --show-toplevel)"
script="$root/$(git ls-files --full-name ':(top,glob)**/554c2935-citation-map.py')"
python3 -B "$script" > "${script%.py}.txt"
```

`554c2935-citation-map.txt` is the output. The script exits with status 1 when
a key of its table or of a source address is absent from the registry.

- Section 5 lists `key | files:lines` for every `@/citation/<key>` address in a
  comment line. It is the map from cited work to citing files.
- Section 1 lists `key | regex | files:lines` for the script's table of prose
  phrases that name a registered work: a title, an author, a document number,
  a product name, or, under a path scope, an anaphoric reference such as a bare
  table number in a file that cites the work by key.
- Section 2, "citations without a registry entry", lists the same for classes
  of prose pointer to a work that has no entry.
- Section 3 lists every comment line that matches the generic citation
  heuristic, holds no address and matches neither table, with a one-line
  reason; a line without a reason prints as `UNEXPLAINED`.
- Section 4 counts section 3 by reason.

An entry of section 1 or 2 that matches no line prints `-`.

## What the listings establish

Every work that a comment cites by address or by a detected prose form has a
registry key exactly when the script exits with status 0, every section 2
class prints `-` and section 3 holds no `UNEXPLAINED` line. Coverage rests on
the pattern list and on the read of every comment line recorded in
[`fa4939c3-comment-read.md`](fa4939c3-comment-read.md): the table holds a row
for each prose form that read lists, and a form outside both appears in no
section.

## Scope rule

A citation names a work: an author with a year, a title, a report or standard
number, an archive identifier, a section of a named book, or a software
library to which the comment attributes a method, a constant or a measured
number. The eponyms Barrett, Karatsuba, Keller–Gehrig, Wiedemann, Markowitz,
Berlekamp–Massey, Box–Muller, Clopper–Pearson, Wilson and Welford name methods
and have no row.

## Edition rule

One key identifies one edition or version; the registry entry states it. The
edition of each standard and versioned software key matches the provenance
named here.

- `ThreeGpp2020` (TS 38.212 V16.4.0) carries the clause 5.4.2.2 citations;
  `crates/gf2-coding/data/ldpc/nr_5g/PROVENANCE.md` names that edition for the
  interleaver. Every other TS 38.212 citation carries `ThreeGpp2017` (V15.0.0);
  no comment names another edition.
- `ThreeGpp2017a` (TS 38.214 V15.0.0) is the Release 15 edition of the same
  date as `ThreeGpp2017`.
- `Etsi2015` (EN 302 755 V1.4.1) is the edition
  `crates/gf2-coding/src/ldpc/dvb_t2/dvb_t2_matrices.rs` names.
- `Etsi2012` (TS 102 831 V1.2.1) is the edition
  `crates/gf2-coding/data/dvb_t2_tr102831_reference.toml` names.
- `Ccsds2017` (CCSDS 131.0-B-3) is the issue
  `crates/gf2-coding/examples/nasa_rate_half_k3.rs` prints in its reading list.
- `Nist2013` (FIPS 186-4) and `Nist2001` (FIPS 197, 2001): no comment names
  an edition of either publication.
- `FflasFfpack2021`, `LinBox2025`, `Shoup2025` and `AlbrechtBard2026` are the
  versions `benchmarks/image.lock` pins.
- `Sionna2026` is the commit that
  `crates/gf2-coding/data/ldpc/nr_5g/PROVENANCE.md` records, `RandCore2025` a
  version `Cargo.lock` pins, and `Joyner2026` the version
  `crates/gf2-coding/tests/data/bch_oracle/gap.json` records.
- `SageMath2025` is the version `crates/gf2-core/tests/gfpn_nested_towers.rs`
  names. `SageMath2026` is the version that
  `crates/gf2-coding/tests/data/bch_oracle/sage.json` and the headers of
  `crates/gf2-algebra/tests/data/cas_permanent_f3_batch.csv` and
  `cas_permanent_f5_f7.csv` record.
- `GapGroup2026` is the version
  `crates/gf2-coding/tests/data/bch_oracle/gap.json` records, and
  `AeneasVerif2026` the pair `scripts/verify-lean.sh` pins.
- `Wright2022` is the draft the comment names, 2020-12. `KlyneNewman2002` and
  `Bryan2013` are RFCs, which have one edition each.
- `Higham2002` and `Warren2012` are the editions whose section numbers the
  comments use: chapter 14 of the 2nd edition of the first is matrix inversion,
  and section 7-3 of the 2nd edition of the second transposes a bit matrix.
