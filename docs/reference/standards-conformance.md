# Standards conformance

`gf2-coding` implements the DVB-T2 forward error correction of ETSI EN 302 755
(`@/citation/Etsi2015`) and the 5G NR LDPC codes of 3GPP TS 38.212
(`@/citation/ThreeGpp2017`). This page states the supported configurations, the
codeword bit order and the external evidence each conformance test consumes.

## DVB-T2

### Configurations

| Component | Entry point | Supported configurations |
|---|---|---|
| BCH outer code | `bch::dvb_t2::dvb_t2_bch_code`, `DvbT2BchDecoder` | every `FrameSize` × `CodeRate`; parameters of EN 302 755 Tables 6a and 6b in [`params.rs`](../../crates/gf2-coding/src/bch/dvb_t2/params.rs) |
| LDPC inner code | `LdpcCode::dvb_t2_normal`, `LdpcCode::dvb_t2_short` | every `FrameSize` × `CodeRate`; `(N_ldpc, K_ldpc, Q_ldpc)` in `DvbParams::for_code`, parity address tables beside it in [`ldpc/dvb_t2/`](../../crates/gf2-coding/src/ldpc/dvb_t2/) |
| BCH + LDPC chain | `ldpc::dvb_t2::concat::DvbT2Concat` | every `FrameSize` × `CodeRate` |
| Bit interleaver (§6.1.3) | `ldpc::dvb_t2::bit_interleaver::DvbT2BitInterleaver` | QPSK, 16-QAM, 64-QAM at rates 1/2, 2/3, 3/4 on both frame sizes and rate 3/5 on normal frames; QPSK is the identity, as §6.1.3 prescribes |

The bit interleaver panics on other rates and has no 256-QAM mode. The cell-word
demultiplexer (§6.1.4) and cell interleaver (§6.1.5) are outside the library.

### Bit order

- **BCH.** The standard transmits the highest-degree coefficient first in both
  the message and the parity block. `dvb_t2_bch_code` therefore declares
  `DVB_T2_LAYOUT = SystematicLayout::MessageParityDescending`: coordinate `d`
  is a message bit for `d < K_bch` and a parity bit above it. The layout maps
  and the internal polynomial convention are defined in the `bch::encode`
  module rustdoc.
- **LDPC.** A FECFRAME is `[K_ldpc information bits | N_ldpc − K_ldpc parity
  bits]`, the BCH codeword occupying the information part.

### Test vectors

The external vectors are the DVB-T2 Verification and Validation reference
streams (`@/citation/DvbVerification2010`), distributed by the DVB Project at
<https://dvb.org/specifications/verification-validation/dvb-t2-reference-streams/>.
They are not redistributed in the repository. The tests read them from
`$DVB_TEST_VECTORS_PATH`, defaulting to `~/dvb_test_vectors`, which holds
one unpacked `<stream>_CSP` archive per stream. The consumed test points are
TP04 (BBFRAME), TP05 (BCH codeword), TP06 (FECFRAME) and TP07a (bit-interleaved
FECFRAME).

| Test target | Stream | Asserts |
|---|---|---|
| [`dvb_t2_bch_verification`](../../crates/gf2-coding/tests/dvb_t2_bch_verification.rs) | VV001-CR35 | `encode(TP04) = TP05` and error-free `decode(TP05) = TP04` on every block of the first frame; TP05 begins with TP04; seeded error correction within `t` |
| [`dvb_t2_ldpc_verification_suite`](../../crates/gf2-coding/tests/dvb_t2_ldpc_verification_suite.rs) | VV001-CR35 | `encode(TP05) = TP06` and error-free decode on every block of the first frame; TP06 begins with TP05; `H·c = 0`; round trip |
| [`dvb_t2_chain_tp07a`](../../crates/gf2-coding/tests/dvb_t2_chain_tp07a.rs) | VV020-FEF, VV009-4KFFT, VV014-64QAM34 | `interleave(TP06) = TP07a` and `deinterleave(TP07a) = TP06` bit-exact (16-QAM 1/2, 64-QAM 2/3, 64-QAM 3/4, normal frame) |

The `dvb_t2_bch_verification` tests outside its slow error-correction case run
in the fast tier and pass without assertion when the vectors are absent; every
other vector test is ignored by default. The BCH generator construction is
checked against the standard's minimal polynomials `g₁`…`g₁₂` by unit tests in
`bch::dvb_t2`.

```bash
DVB_TEST_VECTORS_PATH=/path/to/streams ./scripts/cargo-budget.sh --test \
  cargo nextest run -p gf2-coding --release --profile slow --run-ignored all \
  -E 'binary(dvb_t2_bch_verification) | binary(dvb_t2_ldpc_verification_suite) | binary(dvb_t2_chain_tp07a)'
```

## 5G NR LDPC

### Configurations

| Entry point | Supported configurations |
|---|---|
| `QuasiCyclicLdpc::nr_5g(base_graph, z)` | BG1 and BG2 mother codes at every lifting size of TS 38.212 Table 5.3.2-1 (`nr_5g::all_lifting_sizes`) |
| `QuasiCyclicLdpc::nr_5g_rate_matched(base_graph, target_n, target_k)`, `Nr5gRateMatchedDecoder` | shortened and punctured codes over either base graph; lifting size selected per §5.2.2 (`kb_for_z_selection`, `max_payload_for_lifting`) |
| `nr_5g::interleaver::{output_interleaver, interleave_bits, deinterleave_llrs}` | the §5.4.2.2 rate-matching bit interleaver for any `Q_m` dividing `E` |

The rate-matched code pads the message with zero filler bits to `K_b·Z`,
encodes with the mother code, and transmits the mother-code columns in
ascending order after removing the first `2Z` systematic columns, the filler
columns and the parity columns beyond `target_n`. The transmitted word is
therefore not `[message | parity]`; `Nr5gRateMatchedCode::extract_message`
recovers the message from a decoded mother codeword. Bit selection always
starts at column `2Z`; redundancy versions are not modelled.

### External evidence

The base-graph reference tables in
[`data/ldpc/nr_5g/`](../../crates/gf2-coding/data/ldpc/nr_5g/) are an
independent published copy of TS 38.212 Tables 5.3.2-2 and 5.3.2-3; their
upstream source, pinned commit, licence and digests are in
[`PROVENANCE.md`](../../crates/gf2-coding/data/ldpc/nr_5g/PROVENANCE.md), which
also records the evidence for the §5.4.2.2 interleaver.

| Test target | Asserts |
|---|---|
| [`nr5g_external_vectors`](../../crates/gf2-coding/tests/nr5g_external_vectors.rs) | the compiled-in shift table of every (base graph, `i_LS`) pair equals the reference; `nr_5g(bg, z)` equals the reference reduced `V mod Z` at every lifting size; substituting a wrong `i_LS` table is detected; rate-matched codes on both base graphs select the expected `(Z, i_LS)` and round-trip noiselessly |
| unit tests in [`interleaver.rs`](../../crates/gf2-coding/src/ldpc/nr_5g/interleaver.rs) | permutations derived from the §5.4.2.2 loop |
| [`nr5g_regression`](../../crates/gf2-coding/tests/nr5g_regression.rs) | bit-exact codewords for fixed rate-matched `(n, k)` pairs; fixtures generated by the library itself, so they detect change, not conformance |

All three run in the fast tier with committed data.
