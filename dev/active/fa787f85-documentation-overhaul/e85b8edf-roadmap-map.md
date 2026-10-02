# Root roadmap map: Planned, Research Goals, Long-Term Vision

Source: `61c3f0a6a^:ROADMAP.md`. Every item of the three sections carries one
disposition: `tracked` (issue short ID), `delivered` (code path), `obsolete`
(reason), or `newly filed` (issue title). Tracker states as of 2026-10-02.
Other root sections belong to sibling records.

## Planned (lines 95-106)

| Line | Item | Disposition | Evidence |
|---|---|---|---|
| 99 | M17 GPU belief-propagation prototype | delivered | `crates/gf2-kernels-hip/hip/ldpc_bp.hip`, `crates/gf2-sim/src/gpu/ldpc_bp.rs`; epic `806eb14e` |
| 99 | M17 FPGA belief-propagation prototype | tracked | `adc75ba7` |
| 100 | M18 QAM soft-decision demapping | delivered | `crates/gf2-coding/src/modem/gray_qam_mapper.rs`, `crates/gf2-coding/src/modem/fast_gray_qam_demapper.rs` |
| 101 | M19 End-to-end DVB-T2 FEC + BICM simulation | delivered | `crates/gf2-coding/src/dvb_t2_bicm_harness.rs`; FER curves under `dev/benchmarks/dvb_t2_awgn/`; container `f0448e55` remains open for SDR dependencies |
| 102 | M20 Benchmarking vs Magma/Sage/AFF3CT | tracked | `d77176e5` |
| 103 | M21 GRAND universal decoder | delivered | `crates/gf2-coding/src/grand/orbgrand.rs`, `crates/gf2-coding/src/grand/sogrand.rs` |
| 104 | M22 5G polar CRC-aided SCL decoder | tracked | `b81c239c` |
| 105 | M23 Neural-aided BP | tracked | `9a5662ff` |
| 106 | M24 SDR / GNU Radio blocks | tracked | `21922c59` |

## Research Goals (lines 108-126)

| Line | Item | Disposition | Evidence |
|---|---|---|---|
| 111 | Compete with Magma/Sage on binary field operations | tracked | `d77176e5` |
| 112 | Primitive polynomial testing matches or exceeds CAS | delivered | `crates/gf2-core/src/primitive_polys.rs`; comparison in `crates/gf2-core/docs/BENCHMARKS.md` |
| 113 | GF(2^m) arithmetic via zero-cost abstractions and SIMD | delivered | `crates/gf2-kernels-simd/src/gf2m.rs`, `crates/gf2-kernels-simd/src/gf2m_gemm.rs` |
| 114 | Top-tier Polynomial Systems Solving benchmarks | obsolete | Polynomial system solving (Groebner-basis, XL-style) lies outside the Mission scope in `AGENTS.md`: field arithmetic, dense and sparse linear algebra, and coding theory. The competitive program compares exactly those operation families (`d77176e5`, `64c88ae4`), and the SOTA target matrix (`4c0d0202`) assigns each in-scope family a reference owner or an explicit exclusion |
| 117 | State-of-the-art decoding algorithms (umbrella of lines 118-121) | tracked | `0fc3c9d0`; per-algorithm rows below |
| 118 | GRAND for short codes | delivered | `crates/gf2-coding/src/grand/` |
| 119 | Neural-aided BP for LDPC | tracked | `9a5662ff` |
| 120 | Spatially-coupled LDPC with sliding-window decoding | tracked | `5f4afdf5` |
| 121 | Polar codes with CRC-aided SCL | tracked | `b81c239c` |
| 124 | Novel constructions documented and validated | tracked | `55087229` (PAC codes); delivered constructions `8b1609a8`, `d0e2af0c` |
| 125 | FER curves vs theoretical bounds | tracked | `aed96ef9` (finite-blocklength bounds); curves delivered under `dev/benchmarks/dvb_t2_awgn/` |
| 126 | Reproducible open benchmarks | delivered | `dev/benchmarks/`, container harness `a03b2556`, pinned artefact `7cd9afdb` |

## Long-Term Vision (lines 157-162)

| Line | Item | Disposition | Evidence |
|---|---|---|---|
| 159 | Competitive CAS for binary field research | tracked | `d77176e5`, release `2caf738d` |
| 160 | Publication-worthy novel constructions | tracked | `55087229` |
| 161 | Industry-standard open FEC benchmark suite | tracked | `1362381c` |
| 162 | Educational tool with pedagogical examples | obsolete | Elementary teaching material is outside the researcher audience (brief D-06); `315f4de5` is rejected on that ground, and the tutorial set is fixed by brief D-45 |

## Newly filed

None. Every in-scope item has a tracker issue, delivered code, or an obsolete reason.
