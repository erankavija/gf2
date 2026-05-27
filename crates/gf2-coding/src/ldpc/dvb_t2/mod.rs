//! DVB-T2 LDPC code construction from standard tables.
//!
//! This module implements DVB-T2 LDPC codes by directly building sparse
//! parity-check matrices from ETSI EN 302 755 standard tables, and exposes
//! the full DVB-T2 BICM (Bit-Interleaved Coded Modulation) component pieces.
//!
//! DVB-T2 supports two frame sizes:
//! - **Short frames**: n=16200, Z=360
//! - **Normal frames**: n=64800, Z=360
//!
//! Both support 6 code rates: 1/2, 3/5, 2/3, 3/4, 4/5, 5/6
//!
//! # Canonical BICM chain composition
//!
//! The three BICM component pieces compose as follows (ETSI EN 302 755 §6):
//!
//! ```text
//! BBFRAME → BCH encode → LDPC encode → bit interleave → QAM map
//!                                                             ↓
//!                                                           channel
//!                                                             ↓
//!         BCH decode ← LDPC decode ← bit deinterleave ← QAM demap
//! ```
//!
//! ## Forward (transmit) path
//!
//! ```no_run
//! use gf2_coding::ldpc::dvb_t2::{
//!     concat::DvbT2Concat,
//!     bit_interleaver::{DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation},
//!     FrameSize,
//! };
//! use gf2_coding::modem::{BatchMapper, ModemSpec};
//! use gf2_coding::CodeRate;
//! use gf2_core::BitVec;
//!
//! let frame_size = FrameSize::Normal;
//! let code_rate = CodeRate::Rate1_2;
//!
//! // 1. Construct the BCH+LDPC concatenated codec.
//! let concat = DvbT2Concat::new(frame_size, code_rate).unwrap();
//!
//! // 2. Construct the column-row bit interleaver for 16-QAM.
//! let modcod = DvbT2Modcod::new(frame_size, code_rate, DvbT2Modulation::Qam16);
//! let interleaver = DvbT2BitInterleaver::new(modcod);
//!
//! // 3. Construct the Gray-QAM mapper (16-QAM preset).
//! let spec = ModemSpec::<f32>::gray_square_qam(16);
//! let mapper = spec.preferred_mapper();
//!
//! // 4. BCH + LDPC encode: BBFRAME → FECFRAME (n_ldpc bits).
//! //    NOTE: the first call initialises the LDPC encoder (O(n²/64) for
//! //    Normal frames, ~2-10 s). Subsequent calls are O(nnz).
//! let bbframe = BitVec::zeros(concat.k_bch());
//! let fecframe = concat.encode(&bbframe); // n_ldpc bits
//!
//! // 5. Bit interleave: FECFRAME → interleaved bits (same length, reordered).
//! let interleaved = interleaver.interleave(&fecframe);
//!
//! // 6. QAM map: interleaved bits → I/Q symbols.
//! //    `mapper.map_bits` expects bits in symbol-major, MSB-first order.
//! let m = spec.bits_per_symbol() as usize; // 4 for 16-QAM
//! let num_symbols = interleaved.len() / m;
//! let bits: Vec<bool> = (0..interleaved.len()).map(|i| interleaved.get(i)).collect();
//! let mut tx_i = vec![0.0_f32; num_symbols];
//! let mut tx_q = vec![0.0_f32; num_symbols];
//! mapper.map_bits(&bits, &mut tx_i, &mut tx_q);
//! ```
//!
//! ## Inverse (receive) path
//!
//! ```no_run
//! use gf2_coding::ldpc::dvb_t2::{
//!     concat::DvbT2Concat,
//!     bit_interleaver::{DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation},
//!     FrameSize,
//! };
//! use gf2_coding::modem::{BatchSoftDemapper, DemapInput, DemapMethod, ModemSpec};
//! use gf2_coding::llr::Llr;
//! use gf2_coding::CodeRate;
//!
//! # let frame_size = FrameSize::Normal;
//! # let code_rate = CodeRate::Rate1_2;
//! # let concat = DvbT2Concat::new(frame_size, code_rate).unwrap();
//! # let modcod = DvbT2Modcod::new(frame_size, code_rate, DvbT2Modulation::Qam16);
//! # let interleaver = DvbT2BitInterleaver::new(modcod);
//! # let spec = ModemSpec::<f32>::gray_square_qam(16);
//! # let m = spec.bits_per_symbol() as usize;
//! # let num_symbols = concat.n_ldpc() / m;
//! # let rx_i = vec![0.0_f32; num_symbols];
//! # let rx_q = vec![0.0_f32; num_symbols];
//! let demapper = spec.preferred_soft_demapper();
//!
//! // 7. QAM soft demap: received I/Q symbols → LLRs (interleaved order).
//! //    out_llrs[s * m + k] is the LLR for bit k of symbol s (MSB-first).
//! //    LLR sign convention: positive = bit 0 more likely.
//! let noise_var = vec![0.1_f32; num_symbols]; // N0 = 2*sigma^2
//! let mut out_llrs = vec![Llr::new(0.0); concat.n_ldpc()];
//! demapper.demap_llrs(
//!     DemapInput {
//!         rx_i: &rx_i,
//!         rx_q: &rx_q,
//!         gain_i: None,
//!         gain_q: None,
//!         noise_var: &noise_var,
//!         method: DemapMethod::MaxLog,
//!     },
//!     &mut out_llrs,
//! );
//!
//! // 8. Bit deinterleave LLRs: interleaved order → FECFRAME order.
//! //    fecframe_llrs[i] is the LLR for the bit at FECFRAME position i.
//! let fecframe_llrs = interleaver.deinterleave_llrs(&out_llrs);
//!
//! // 9. LDPC + BCH decode: FECFRAME LLRs → recovered BBFRAME.
//! let bbframe_out = concat.decode_soft(&fecframe_llrs).unwrap();
//! assert_eq!(bbframe_out.len(), concat.k_bch());
//! ```
//!
//! ## LLR sign convention
//!
//! Positive LLR means bit 0 is more likely; negative LLR means bit 1 is
//! more likely. Both `BatchSoftDemapper::demap_llrs` output and
//! `DvbT2Concat::decode_soft` input follow this convention.
//!
//! ## Noise variance convention for the demapper
//!
//! `DemapInput::noise_var` expects the **total per-symbol complex AWGN
//! noise variance** `N0 = 2 * sigma^2`. For a real AWGN channel with
//! independent Gaussian noise of variance `sigma^2` on each of I and Q,
//! pass `2 * sigma^2`. See [`crate::modem::awgn_link`] for the canonical
//! `Eb/N0 → sigma^2` conversion helper.
//!
//! ## In-scope configurations
//!
//! The three in-scope configurations for this BICM composition are:
//! Normal × {Rate 1/2, 2/3, 3/4} × {16-QAM, 64-QAM} (6 combinations total).
//! All six pass the roundtrip integration test in
//! `crates/gf2-coding/tests/dvb_t2_bicm_chain.rs` (slow tier, requires
//! the LDPC encoder).
//!
//! A complete runnable example lives in
//! `crates/gf2-coding/examples/dvb_t2_bicm_chain.rs`.
//!
//! # Usage
//!
//! Use the factory methods on `LdpcCode`:
//! ```
//! use gf2_coding::ldpc::LdpcCode;
//! use gf2_coding::CodeRate;
//!
//! let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
//! assert_eq!(code.n(), 64800);
//! ```

pub mod bit_interleaver;
pub(crate) mod builder;
pub mod concat;
pub(crate) mod dvb_t2_matrices;
pub(crate) mod params;

pub use bit_interleaver::{DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation};
pub use params::{DvbParams, FrameSize};
