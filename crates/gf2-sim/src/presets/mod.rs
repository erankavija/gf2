//! Typestate preset builders for standard pipelines.
//!
//! A preset assembles one standard's BICM chain over
//! [`Chain`](crate::graph::Chain) and emits a validated
//! [`Pipeline`](crate::Pipeline). Each builder is generic over a zero-sized
//! state type: a required setter exists only on the state that accepts it next
//! and returns the builder in the following state, so an out-of-order call is
//! a compile error and only the terminal `Ready` state exposes `build()`.
//! Optional setters live on `Ready`. [`dvb_t2`] builds the DVB-T2 BICM chain
//! (`@/citation/Etsi2015`) and [`nr_5g`] the 5G NR LDPC chain
//! (`@/citation/ThreeGpp2017`).

pub mod dvb_t2;
pub mod nr_5g;
