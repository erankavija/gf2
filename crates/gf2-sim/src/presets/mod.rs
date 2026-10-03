//! Typestate preset builders for standard pipelines.
//!
//! A preset assembles one standard's BICM chain over
//! [`Chain`](crate::graph::Chain) and emits a validated
//! [`Pipeline`](crate::Pipeline). Each builder is generic over a zero-sized
//! state type: a required setter exists only on the state that accepts it next
//! and returns the builder in the following state, so an out-of-order call is
//! a compile error and only the terminal `Ready` state exposes `build()`.
//! Optional setters live on `Ready`.
//!
//! * [`dvb_t2`] — [`Pipeline::dvb_t2`](crate::Pipeline::dvb_t2): DVB-T2 BICM
//!   (BCH + LDPC concatenation, `@/citation/Etsi2015`). Order:
//!   `modcod → decoder → demap → channel`.
//! * [`nr_5g`] — [`Pipeline::nr_5g`](crate::Pipeline::nr_5g): 5G NR LDPC
//!   (`@/citation/ThreeGpp2017`). Order:
//!   `base_graph → lifting_size → rate → decoder → demap → channel`, with an
//!   optional `lifting_set` alongside `lifting_size`.

pub mod dvb_t2;
pub mod nr_5g;
