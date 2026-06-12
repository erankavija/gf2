//! Typestate preset builders for standard pipelines (design doc §9).
//!
//! A **preset** is a fluent builder that assembles one standard's BICM chain
//! over the low-level graph API ([`Chain`](crate::graph::Chain)) and emits a
//! validated [`Pipeline`](crate::Pipeline). The presets own no coding-theory
//! math — they are thin wrappers that pick the canonical stage order, splice
//! the channel between the forward and inverse halves, connect the stages, and
//! call [`Chain::build`](crate::graph::Chain::build).
//!
//! # The typestate pattern
//!
//! Each builder is generic over a zero-sized **state** type. The required
//! configuration methods are defined only on the state that should accept them
//! next, and each consumes `self` and returns the builder in the *next* state.
//! Calling a method out of order (for example `.decoder(...)` before
//! `.modcod(...)`) is a **compile-time** error because the method does not
//! exist on the current state — the builder cannot reach the terminal `Ready`
//! state (the only one exposing `build()`) without supplying every required
//! stage exactly once, in order. Optional setters (seed, parallelism,
//! checkpoint directory, GPU enable) live on `Ready` and do not advance the
//! state. This makes a half-configured pipeline unrepresentable rather than a
//! runtime error.
//!
//! # The two presets
//!
//! * [`dvb_t2`] — [`Pipeline::dvb_t2`](crate::Pipeline::dvb_t2): the DVB-T2
//!   BICM preset (BCH + LDPC concatenation, ETSI EN 302 755). Order:
//!   `modcod → decoder → demap → channel`. Driven by the sweep-level
//!   [`Pipeline::run`](crate::Pipeline::run).
//! * [`nr_5g`] — [`Pipeline::nr_5g`](crate::Pipeline::nr_5g): the 5G NR LDPC
//!   preset (3GPP TS 38.212, single inner code, no outer code). Order:
//!   `base_graph → lifting_size → rate → decoder → demap → channel` (with an
//!   optional `lifting_set` refinement alongside `lifting_size`). Driven
//!   per-batch by [`TopologyExecutor::run`](crate::TopologyExecutor::run) —
//!   there is no NR sweep-level `Pipeline::run`.
//!
//! See the module docs of each submodule for the exact in-scope parameter
//! tuples, and the crate-level worked examples
//! (`examples/dvb_t2_quickstart.rs`, `examples/nr_5g_quickstart.rs`).

// owned by 81d05bab (typestate builder framework)

pub mod dvb_t2;
pub mod nr_5g;
