# 5G NR rate-matched encoder baselines: plan

> **Diátaxis Type:** Explanation

This plan fixes the operation, the configuration grid, the comparator arms and
the campaign order for JIT issue `12fdeb5b`. It changes no production kernel.
The normative rules are the
[measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and [protocol version 4](../f547c394/protocol.md); this document declares only
what the contract leaves to the family, and the frozen addenda carry the
numbers.

## The compared operation

The consumer operation is the whole encode: `target_k` information bits in,
`target_n` rate-matched codeword bits out. gf2 reaches it through
`<Nr5gRateMatchedCode as BlockEncoder>::encode`
(`gf2-block-encoder-encode`), which fuses 3GPP TS 38.212 [ThreeGpp2017]
Section 5.4.2.1 bit selection into the encode as a gather over a precomputed
column list (`gf2-transmitted-gather`); the rate-matched encode itself is
private (`gf2-encode-rate-matched-private`), so no narrower comparison exists
on gf2's public surface.

Both comparators split the operation in two public halves and the adapters
compose them:

- AFF3CT [Cassagne2019] runs `Encoder::encode` on `Encoder_LDPC_QC_fast`
  (`aff3ct-encoder-public-encode`, `aff3ct-qc-fast-encode`) and then
  `Puncturer::puncture` on `Puncturer_5G`
  (`aff3ct-puncturer-public-puncture`, `aff3ct-puncture-selection`).
- srsRAN [Srsran2026] runs `ldpc_encoder::encode` (`srsran-encoder-entry`) and
  then `ldpc_rate_matcher::rate_match` (`srsran-rate-matcher-entry`).

The consumer representation is gf2's `BitVec`. Each adapter converts into its
project's representation and back inside its own timed call, so every
conversion is charged to the arm that needs it, as the contract's
whole-consumer rule requires. AFF3CT works on one `int32_t` per bit; srsRAN
works on bits packed most significant bit first inside `uint8_t` words, so its
adapter's conversion is a per-byte bit reversal against gf2's
least-significant-bit-first `u64` packing.

## Configuration grid

The grid is `survey/nr-encode/src/lib.rs`'s `CONFIGURATIONS` and the derived
parameters are `survey/nr-encode-parameters.json`, produced by
`derive-nr-encode-parameters`. It spans:

- both base graphs, with the rates TS 38.212 Section 7.2.2 assigns them;
- at least one lifting size from each of the eight lifting sets of TS 38.212
  Table 5.3.2-1;
- codes with filler bits and codes whose message exactly fills the systematic
  block, for both base graphs;
- two pairs of message lengths one bit apart that cross a lifting-size
  boundary: a base-graph-1 pair at a rate where AFF3CT also selects base graph
  1, which also crosses from a filler-free code to one with fillers, and a
  base-graph-2 pair across the `560 < K <= 640` lifting constant;
- redundancy versions 1 and 2 alongside the version-0 default.

Modulation is fixed at one bit per symbol. TS 38.212 Section 5.4.2.2
interleaving is then the identity and srsRAN's rate matcher reduces to
selection plus packing (`srsran-interleave-identity`), which is the operation
gf2 implements; a higher modulation order would add an interleave gf2 has no
counterpart for.

## Which configurations each project implements as specified

- **Base-graph selection.** gf2 and srsRAN take the base graph from the caller
  (`srsran-encoder-caller-base-graph`). AFF3CT derives it from `K` and `K/N`
  (`aff3ct-bg-selection`), so it cannot be asked for a base graph its own rule
  rejects. On this grid its rule routes the three rate-1/2 base-graph-1
  configurations to base graph 2 (`aff3ct-bg2-branch`).
- **Lifting size.** All three multiply the base-graph column counts by the
  chosen lifting size (`aff3ct-mother-dimensions`, `aff3ct-mother-length`,
  `srsran-encoder-caller-lifting`). gf2 additionally requires enough
  transmitted bits when choosing it (`gf2-z-selection`). For
  `560 < K <= 640` the lifting `Kb` differs: gf2 uses 9 (`gf2-kb-560-640`,
  `gf2-kb-560-640-value`), the value TS 38.212 Section 5.2.2 specifies, and
  AFF3CT uses 8 (`aff3ct-kb-560-640`, `aff3ct-kb-560-640-value`), which selects
  a larger lifting size and a different mother code.
- **Filler bits.** gf2 zero-pads the systematic block (`gf2-filler-pad`);
  AFF3CT's selection skips the positions between `K` and `K_LDPC`
  (`aff3ct-puncture-filler-skip`); srsRAN skips a filler range at the end of
  the systematic section, indexed relative to its shortened buffer
  (`srsran-rate-matcher-filler-range`, `srsran-rate-matcher-systematic`).
- **Redundancy-version starting offset.** srsRAN implements all four versions
  (`srsran-rate-matcher-rv-range`) through the TS 38.212 Table 5.4.2.1-2 shift
  factors (`srsran-rate-matcher-shift-bg1`, `srsran-rate-matcher-shift-bg2`,
  `srsran-rate-matcher-k0`, `srsran-rate-matcher-start`). gf2 and AFF3CT carry
  no redundancy-version parameter at all
  (`gf2-no-redundancy-version`, `aff3ct-no-redundancy-version`), so both
  implement the version-0 offset only.

A configuration is a matched arm only when the comparator derives gf2's base
graph, lifting size and mother dimensions and reproduces gf2's codeword on
every validation message. The rest are recorded with the differing parameter
and never timed.

## Comparator builds

`survey/nr-encode-build.sh` verifies the pins and builds the arms;
`survey/build-evidence.json` records what it observed.

- AFF3CT v4.7.0 at commit `e8a65c50`, MIT, with its `conf` submodule at
  `ecae10cd` supplying the per lifting-set generator matrices
  (`aff3ct-encoder-5g-g-path`). The shim links the static library the
  `c077a88b` build identity pins. The selected backend is scalar: the 5G
  puncturer and the fast quasi-cyclic encoder carry no intrinsic path.
- srsRAN Project 25.10 at commit `d2f4b70d`, AGPL-3.0-or-later. Its own CMake
  configuration stops on this host because MbedTLS is absent, so the shim
  compiles the seven LDPC and `srsvec` translation units the encoder and rate
  matcher need; none of the layers that require MbedTLS is on that closure.
  The shim reproduces `ldpc_encoder_factory_sw("auto")`
  (`srsran-encoder-backend-auto`) and reports the backend it selected.
  The linked binary stays in the git-ignored `.agents/ext/12fdeb5b/` of the
  checkout that builds it and is not committed or distributed; the repository
  commits only the survey's own shim source.

## Arms

| Arm | Build identity | Operation |
|---|---|---|
| `gf2-native` | `native` | gf2's `BlockEncoder::encode`, `-C target-cpu=native` |
| `gf2-native-control` | `native` | the same executable as a second arm, for the identity cell |
| `gf2-portable` | `conservative-portable` | the same encode, `-C target-cpu=x86-64` |
| `srsran-external` | `external` | srsRAN encode plus rate match behind the whole-consumer adapter |
| `aff3ct-external` | `external` | AFF3CT encode plus puncture behind the whole-consumer adapter |

Every cell's baseline is the established gf2 arm and its candidate is the arm
whose lead needs attribution, so a speedup above 1 favours the candidate.

## Cells and campaign order

All cells are `comparator-gap`, `whole-consumer`, `single-core-latency`,
`single-core`, one declared worker, `warm` cache state, conversion costs
included. The family adopts nothing, so it declares no worthwhile speedup;
the material-gap threshold is the margin it decides on.

The order is smoke, then pilot, then freeze, then confirmation:

1. **Smoke** (`addendum-nr-encode-smoke.json`, five exploratory cells, six
   pairs each). It reaches a result line from all five arms, exercises the
   largest and smallest configurations, and sizes the pilot's wall clock. It
   is not a performance result about gf2.
2. **Pilot** (`addendum-nr-encode-pilot.json`, eight exploratory cells, 24
   pairs each). It measures the family's resolution across both base graphs,
   both comparators, the build control and the identity floor.
3. **Freeze.** The confirmatory addendum takes its `measurement_resolution`
   from the committed pilot receipt, pinned by path and digest, computed at
   the confirmation's corrected alpha rather than the pilot's own, the rule
   `eda07788`'s NR family adopted after the DVB-T2 family's wider-interval
   contradiction.
4. **Confirmation** (six confirmatory cells, 24 fresh pairs each). Six is the
   cap for a family with no ledger history under P-20.

The confirmatory cells span both base graphs, filler and filler-free codes and
both comparators: three srsRAN gap cells, two AFF3CT gap cells and the
portable-versus-native build control. The exact cell list freezes with the
confirmatory addendum, after the pilot receipt is committed.

## What this survey does not measure

- Redundancy versions other than 0 have no gf2 counterpart. They are recorded
  as non-equivalent with the differing parameter and carry no cell.
- The configurations where AFF3CT's own derivation differs from gf2's are
  recorded, not timed, on the AFF3CT arm; srsRAN serves them because it takes
  the parameters from the caller.
- Bit selection alone is not a cell. gf2 exposes no forward selection entry
  point, so timing a harness re-implementation would time the harness.
- The survey proposes no production change, so it declares no worthwhile
  speedup and adopts nothing.
