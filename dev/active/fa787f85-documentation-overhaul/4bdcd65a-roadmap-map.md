# Root roadmap map: Open Research Questions and Publication & Validation

Source: `61c3f0a6a^:ROADMAP.md` (sections "Open Research Questions", lines 128-155, and "Publication & Validation", lines 164-182). Citations below are `61c3f0a6a^:ROADMAP.md:<line>`. Issue states come from the tracker query of 2026-10-02.

Dispositions: `tracked` (named issue), `delivered` (named code or evidence path), `obsolete` (reason), `newly filed` (issue title in `4bdcd65a-file-issues.sh`).

## Open Research Questions

| Line | Item | Disposition |
|---|---|---|
| 131 | GPU LDPC memory- vs compute-bound | delivered: `dev/archive/806eb14e-hip-gpu-prototype/active/806eb14e-feasibility-report.md`; issues `43fb19e2`, `24c11004`, `86a363aa` (done) |
| 132 | FPGA feasibility of functional Rust to HDL | tracked: `adc75ba7` |
| 133 | GPU vs multi-core CPU crossover for LDPC | delivered: `9c37ec8c` (crossover measurement), `86a363aa` (decision); kernel `crates/gf2-kernels-hip/hip/ldpc_bp.hip` |
| 136 | GRAND vs algebraic decoding for short codes | newly filed: "Compare ORBGRAND with OSD and algebraic decoding on short codes" |
| 137 | Normalized/offset min-sum gains | delivered: `crates/gf2-coding/src/ldpc/min_sum.rs`; `c3ea6855` |
| 138 | Quantized LLRs, 3-8 bit | tracked: `d69b964e`, `f63a2464` |
| 139 | Structured LDPC encoding without dense matrices | delivered: `crates/gf2-coding/src/ldpc/encoding/ira.rs`; `82dd7384` |
| 140 | Neural-aided BP iteration reduction | tracked: `9a5662ff` |
| 143 | End-to-end DVB-T2 latency budget | newly filed: "Measure the DVB-T2 receive-chain stage latency budget" |
| 144 | Rust vs GNU Radio C++ throughput | tracked: `dfca71b8` (benchmark against GNU Radio FEC blocks) |
| 145 | Real-signal validation vs test vectors | tracked: `dfca71b8`, `fcb09e6f` |
| 148 | Rust+SIMD vs Magma/Sage crossover | tracked: `d77176e5`; measured parts `53c5a8c0` (done) |
| 149 | FEC decoder gap to AFF3CT/IT++ | delivered: `18e69a1a`, `c077a88b`, `3be770d5` (done); continuation tracked: `1a379447`, `1362381c` |
| 150 | Karatsuba vs FFT for m > 64 | delivered: `53c5a8c0` (crossovers), `crates/gf2-core/src/gf2m/wide.rs` (`6fb4abad`); additive NTT tracked: `24701af9`, `c7cfd37e` |
| 153 | Shannon gap of practical LDPC decoders | delivered: `e4849f07` (FER curves vs reference), `325e5c89`; bounds tracked: `aed96ef9` |
| 154 | Finite-length polar vs LDPC, N < 10K | tracked: `21eb03e8` (blocked on polar decoders `b94e2821`, `b81c239c`) |
| 155 | SC-LDPC threshold saturation gains | tracked: `5f4afdf5` |

## Publication & Validation

| Line | Item | Disposition |
|---|---|---|
| 167 | Technical reports on implementations | delivered: `01ae4c20`, `24c11004`, `a9ab0a4f` (done); `dev/benchmarks/` |
| 168 | Open-source reproducible benchmark suites | delivered: `6ed7f050`; `dev/benchmarks/` |
| 169 | Conference targets (ISIT, ICC, Globecom) | obsolete: venue selection is an author decision with no repository artifact; results reach papers through the committed receipts under `dev/benchmarks/` |
| 170 | Journal targets (IEEE Trans. IT, Trans. Comm) | obsolete: same reason as line 169 |
| 173 | DVB-T2 bit-exact compliance | delivered: `4cdaf1c5` (202/202 chain vectors); `crates/gf2-coding/docs/DVB_T2.md` |
| 174 | 5G NR polar vs 3GPP test vectors | tracked: `b81c239c` (5G NR LDPC vectors delivered: `dd22a099`, `acf9b11a`) |
| 175 | Decode real-world DVB-T2 captures | tracked: `fcb09e6f`, `dfca71b8` |
| 176 | Compete with commercial SDR implementations | obsolete: no commercial comparator is available to the project; open-source baselines are covered by `18e69a1a`, `1a379447` |
| 179 | All benchmarks reproducible with published code | delivered: `a03b2556` (container image), `7cd9afdb` (pinned artefact) |
| 180 | Claims backed by methodology documentation | delivered: `5102d87a`, `1d6043e8` (acceptance protocol) |
| 181 | Comparison with commercial tools (Magma) when licensing allows | tracked: `d77176e5` |
| 182 | Data and FER curves available for verification | delivered: `152388f4`; `dev/benchmarks/dvb_t2_awgn` |
