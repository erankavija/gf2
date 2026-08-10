//! Measurement harness for the permanent-zero-fraction campaign feasibility
//! study (JIT issue `b488f02c`).
//!
//! The study asks whether an empirical test of the Ghasemi–Gross–Kopparty
//! conjecture — $\Pr[\mathrm{per}(A) = 0] = 1/q + o(1)$ for uniform
//! $A \in \mathbb{F}_q^{n \times n}$ — is reachable on the available hardware.
//! Answering it needs measured throughput of the campaign's whole hot path,
//! not of the permanent kernels alone, so this crate:
//!
//! - samples uniform $\mathbb{F}_q$ matrices by exact rejection
//!   ([`sampler`]), because the in-tree fixture generator is modulo-biased;
//! - validates that every backend returns identical per-matrix permanents on
//!   shared inputs ([`equivalence`]) before any timing counts as evidence;
//! - times generation, evaluation, reduction, and shard storage separately and
//!   as a composite, while GPU rows also retain their event-measured kernel,
//!   transfer, and submission spans in distinct columns ([`protocol`]);
//! - converts the composite rates into required sample counts and wall-clock
//!   under a stated budget ([`stats`]), measured against the published $q = 3$
//!   numerics the campaign has to beat to claim novelty ([`prior`]).
//!
//! The crate is a research prototype outside the Cargo workspace; nothing in
//! the production crates depends on it.

pub mod backend;
pub mod env;
pub mod equivalence;
pub mod prior;
pub mod protocol;
pub mod sampler;
pub mod schedule;
pub mod stats;
