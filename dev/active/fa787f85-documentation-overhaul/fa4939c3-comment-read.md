# Comment read for citations outside the map's patterns

Issue `fa4939c3`. The citation map
([`554c2935-citation-map.md`](554c2935-citation-map.md)) finds prose citations
by a pattern list. This record lists a read of every comment line the map
script reads, so that a citation in a form outside the list is found by
reading.

## What was read

The commit read is `04c00d0ba3116652526ba51728390b427fdcad8f`. The lines are
those `comment_of` of `554c2935-citation-map.py` selects in the tracked `.rs`
files under `crates/` and `dev/tools/`: a line whose first non-blank
characters are `//`, or the part of a code line after ` // `. They number
41,604 in 737 files.

The per-crate counts of lines that begin with `//` equal the comment column of
`030496bd-comment-census.txt`, whose commit
`fa29c6a3976e162d8994dfc19ea85c90df848f7c` has the same `crates/` tree as the
commit read. The census covers `crates/` only.

## Method

Ten reader agents each read one consecutive window of the extracted comment
lines and reported every line that names or points into an external work:
456 lines. A reader reported a doubtful line instead of dropping it. Each
reported line was then opened in its file, with the surrounding block, and
classified against the scope rule of the map record. The readers did not
report a line whose only citation is an `@/citation/<key>` address without a
locator; the listing takes those lines, and every other line the map lists,
from sections 1 and 5 of the map: 168 lines.

Dispositions:

- `keyed`: the line names or points into a work that has a registry key.
- `new work`: the line names a work that the read registers.
- `not a citation`, with one of these reasons:
  - `section of an internal design or protocol document` and
    `pen-and-paper computation`, the map script's labels;
  - `repository document or harness`: a contract, protocol, script, database
    module or reference harness of this repository;
  - `standard or code family named as a topic`: DVB-T2 or 5G NR as the
    subject, with no clause, table or number attributed;
  - `method or object name`: an algorithm, inequality, generator, cipher, hash
    or mathematical object named without year, title, section or number
    (Chan–Golub–LeVeque, Hoeffding, SplitMix64, ChaCha20, BLAKE3, Conway
    polynomial), the eponym class of the scope rule, which a registry key
    for the name's work leaves unchanged, as for Wilson;
  - `names software without attributing a method or number`: nalgebra,
    Armadillo, the HIP runtime, and `sha256sum` of coreutils, whose check-file
    format is a format and not a method, constant or measured number;
  - `code identifier`: a preset, function or type name;
  - `no external work named`: "standard", "published", "oracle" or
    "reference" with no particular work;
  - `part of a sentence whose citation is listed at an adjacent line`.

## Result

| Disposition | Reported by a reader | Taken from the map | Total |
| --- | ---: | ---: | ---: |
| `keyed` | 311 | 165 | 476 |
| `new work` | 6 | 3 | 9 |
| `not a citation` | 139 | 0 | 139 |
| total | 456 | 168 | 624 |

Section 4 of the listing counts `not a citation` by reason.

The read registers four works, each verified against the URL its registry
entry records:

| Key | Work | Citing lines |
| --- | --- | --- |
| `KlyneNewman2002` | RFC 3339 | `crates/gf2-core/benches/tuning_calibration.rs:1196`, `crates/gf2-core/src/tuning/mod.rs:137,147` |
| `Bryan2013` | RFC 6901, JSON Pointer | `dev/tools/tuning-campaign-support/src/schema.rs:5,8,10` |
| `Wright2022` | JSON Schema draft 2020-12, Core | `dev/tools/tuning-campaign-support/src/schema.rs:18` |
| `MacKay2006` | alist format | `crates/gf2-sim/src/bin/export_alist.rs:1,6` |

Fifteen lines carry an address that the commit read lacks. Each edit stays
within its line, so the line counts of the commit read hold for the listed
tree.

| Line | Key |
| --- | --- |
| `crates/gf2-algebra/src/testutil.rs:152` | `Hinnant2021` |
| `crates/gf2-coding/src/fading.rs:84,93,102` | `Yuan2025` |
| `crates/gf2-coding/src/ldpc/dvb_t2/builder.rs:1` | `Etsi2015` |
| `crates/gf2-coding/tests/test_vector_parser.rs:2` | `DvbVerification2010` |
| `crates/gf2-coding/tests/test_vectors/mod.rs:19` | `DvbVerification2010` |
| `crates/gf2-core/src/gfpn/ext_config.rs:40` | `AeneasVerif2026` |
| `crates/gf2-core/src/gfp/mod.rs:518` | `DumasPernet2012` |
| `crates/gf2-core/src/gfp/specialized.rs:660` | `DumasPernet2012` |
| `crates/gf2-core/benches/tuning_calibration.rs:1196` | `KlyneNewman2002` |
| `crates/gf2-core/src/tuning/mod.rs:137` | `KlyneNewman2002` |
| `dev/tools/tuning-campaign-support/src/schema.rs:5` | `Bryan2013` |
| `dev/tools/tuning-campaign-support/src/schema.rs:18` | `Wright2022` |
| `crates/gf2-sim/src/bin/export_alist.rs:1` | `MacKay2006` |

The channel parameters of `RicianConfig::fig8`, `fig9` and `fig10` in
`fading.rs` are those of the block Rician fading figures of
`@/citation/Yuan2025`; arXiv:2310.10737v5 numbers those figures 11, 12 and 13.

Every other `keyed` line in a form the pattern list lacked has a row in the
map script, under a path scope where the form is anaphoric. No work is
unidentified, so the map's section 2 gains no class.

## Attributions the sources contradict

The read compared three comments with their sources and found a difference.
The comments are unchanged.

- `crates/gf2-coding/tests/shortened_fast_path.rs:28` places the polynomial
  $x^{16} + x^5 + x^3 + x^2 + 1$ in `@/citation/Etsi2015` Table 6b, and lines
  32 and 37 take $t = 12$ and $K_{bch} = 32208$ from "the table". EN 302 755
  V1.4.1 gives the polynomial as $g_1(x)$ in Table 7(a) and both parameters in
  Table 6(a); Table 6(b) holds the short-frame coding parameters.
- `crates/gf2-coding/src/ldpc/dvb_t2/bit_interleaver.rs:690` quotes the title
  of §6.1.3 as "Bit Interleaving (for 16-QAM, 64-QAM and 256-QAM)". The clause
  is titled "Bit Interleaver (for 16-QAM, 64-QAM and 256-QAM)".
- `crates/gf2-kernels-hip/src/host/streams.rs:120` states that the HIP runtime
  documents `hipStreamSynchronize` and `hipStreamQuery` as thread-safe. The
  HIP 7.15.0 stream-management reference states it for `hipStreamQuery` only
  (<https://rocm.docs.amd.com/projects/HIP/en/latest/doxygen/html/group___stream.html>).

`crates/gf2-coding/tests/dvb_t2_chain_tp07a.rs` names the stream
configurations VV009-4KFFT, VV014-64QAM34 and VV020-FEF under
`@/citation/DvbVerification2010`, whose entry names configuration VV001-CR35.

## Command

From the repository root or any subdirectory of it:

```sh
root="$(git rev-parse --show-toplevel)"
script="$root/$(git ls-files --full-name ':(top,glob)**/fa4939c3-comment-read.py')"
python3 -B "$script" > "${script%.py}.txt"
```

`fa4939c3-comment-read.txt` is the output and `fa4939c3-hits.tsv` the
classified lines. The listing names the commit read and the object ids of the
`crates` and `dev/tools` trees it is generated on, then gives, per file and
per crate, the lines read and the lines that begin with `//`, then every
classified line with its comment text. The script's header states the
conditions under which it exits with status 1.

## Limits

The read is a recorded review by reader agents, reproducible as a listing; it
is not a mechanical proof that no comment cites an unregistered work. A reader
can pass over a line, and the classification of a reported line is a
judgement against the scope rule. The script checks that the listing and the
map agree line by line and that the line counts hold; it does not check the
judgement. The three source comparisons above cover the lines whose source
text was open during classification, not every attributed locator.
