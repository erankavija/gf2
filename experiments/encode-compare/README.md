# Encoder comparisons

Throughput of gf2's encoders against the reference implementations on
identical seeded messages (@/issue/be6d6f45, @/issue/040bc2b8). `compare.sh nr`
times NR rate-matched LDPC encoding at redundancy version 0, one bit per
symbol, against srsRAN (`@/citation/Srsran2026`) with the encoder its "auto"
factory selects and AFF3CT 4.7.0 (`@/citation/Cassagne2019`)
`Encoder_LDPC_QC_fast` with `Puncturer_5G` at int8 and int32 bits.
`compare.sh bch` times the DVB-T2 rate-1/2 outer BCH codes of both frame sizes
(`@/citation/Etsi2015`) through `DvbT2BchCode::encode_into` against bchlib
(`@/citation/Bchlib2026`) `bch_encode`, cycling a pool of 1 or 256 distinct
messages. bchlib runs in two builds: its runtime-parameter one and one with
`CONFIG_BCH_CONST_PARAMS` fixed to each cell's (m, t), the configuration its
source documents as the faster encoder; the table marks the faster per cell.

Each arm takes its library's native input format, converted before timing.
An arm is timed only after it reproduces gf2's codewords for every message of
the cell; AFF3CT derives its own base graph and lifting size, and a cell where
those differ from gf2's is reported and skipped. A row is the median of the
repetitions [`src/lib.rs`](src/lib.rs) states, every worker running its own
encoder over the shared messages.

The comparison binary is the [`external/`](external/) crate, outside the
workspace: its build compiles srsRAN's LDPC translation units and links
AFF3CT's static library and compiles bchlib from the trees `GF2_SRSRAN_ROOT`,
`GF2_AFF3CT_ROOT` and `GF2_BCHLIB_ROOT` name, all `-O3 -march=native`; gf2
builds with `-C target-cpu=native`.
