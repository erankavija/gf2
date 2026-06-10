//! Build-time graph errors at `Pipeline::build()` (issue `de160fc5`,
//! criterion 2; design doc §9).
//!
//! A cyclic graph yields [`BuildError::Cyclic`] and a disconnected graph
//! yields [`BuildError::Disconnected`] — both at
//! [`Chain::build`](gf2_sim::graph::Chain::build), the **only** public
//! constructor of a runnable [`Pipeline`](gf2_sim::Pipeline), so neither shape
//! can ever reach execution (amendment 2026-06-10a). This file adds the
//! integration-tier coverage for `Disconnected` (previously unit-tested only
//! in `graph/mod.rs`) and re-exercises `Cyclic` as the named criterion
//! surface; the original `Cyclic` guard in `tests/cyclic_chain.rs` (issue
//! `c09d3e95`) remains in place.

use gf2_sim::error::BuildError;
use gf2_sim::graph::Chain;
use gf2_sim::stage::{erase, BatchSize, ExecutionClass, Stage};
use gf2_sim::StageError;

/// A tiny one-frame batch newtype so the graph has something typed to carry.
#[derive(Clone)]
struct Frames(Vec<u8>);
impl BatchSize for Frames {
    fn batch_size(&self) -> usize {
        self.0.len()
    }
}

/// CPU identity over [`Frames`]; its matching input/output types let edges be
/// recorded in either direction (which is exactly what a cycle needs).
struct Id;
impl Stage<Frames, Frames> for Id {
    type Scratch = ();
    type CpuFallback = Self;
    fn process(&self, i: &Frames, _: &mut ()) -> Result<Frames, StageError> {
        Ok(i.clone())
    }
    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

#[test]
fn test_cyclic_graph_yields_build_error_cyclic() {
    // a → b → c → a: a 3-cycle. Every connect() type-checks (same batch type
    // throughout), so the cycle is only detectable at build().
    let mut chain = Chain::new();
    let a = chain.add(erase(Id));
    let b = chain.add(erase(Id));
    let c = chain.add(erase(Id));
    chain.connect(a, b).expect("type-compatible");
    chain.connect(b, c).expect("type-compatible");
    chain.connect(c, a).expect("type-compatible");

    match chain.build() {
        Err(BuildError::Cyclic { involved }) => {
            assert_eq!(
                involved,
                vec![a, b, c],
                "all three stages lie on the cycle and are reported"
            );
        }
        Err(other) => panic!("expected BuildError::Cyclic, got {other:?}"),
        Ok(_) => panic!("expected BuildError::Cyclic, got a built pipeline"),
    }
}

#[test]
fn test_partial_cycle_yields_build_error_cyclic_with_only_cycle_members() {
    // d → a → b → a: stage d is acyclic, the {a, b} pair cycles. Only the
    // cycle members are reported.
    let mut chain = Chain::new();
    let d = chain.add(erase(Id));
    let a = chain.add(erase(Id));
    let b = chain.add(erase(Id));
    chain.connect(d, a).expect("type-compatible");
    chain.connect(a, b).expect("type-compatible");
    chain.connect(b, a).expect("type-compatible");

    match chain.build() {
        Err(BuildError::Cyclic { involved }) => {
            assert_eq!(involved, vec![a, b], "only the cycle members are reported");
        }
        Err(other) => panic!("expected BuildError::Cyclic, got {other:?}"),
        Ok(_) => panic!("expected BuildError::Cyclic, got a built pipeline"),
    }
}

#[test]
fn test_disconnected_graph_yields_build_error_disconnected() {
    // Two disjoint components: {a → b} and {c → d}. build() must reject with
    // the stages outside the lowest-id component listed.
    let mut chain = Chain::new();
    let a = chain.add(erase(Id));
    let b = chain.add(erase(Id));
    let c = chain.add(erase(Id));
    let d = chain.add(erase(Id));
    chain.connect(a, b).expect("type-compatible");
    chain.connect(c, d).expect("type-compatible");

    match chain.build() {
        Err(BuildError::Disconnected { stages }) => {
            assert_eq!(
                stages,
                vec![c, d],
                "the component outside the one containing the lowest id is reported"
            );
        }
        Err(other) => panic!("expected BuildError::Disconnected, got {other:?}"),
        Ok(_) => panic!("expected BuildError::Disconnected, got a built pipeline"),
    }
}

#[test]
fn test_isolated_stage_yields_build_error_disconnected() {
    // A connected pair plus one isolated stage (no edges at all): still a
    // disconnected graph.
    let mut chain = Chain::new();
    let a = chain.add(erase(Id));
    let b = chain.add(erase(Id));
    let lone = chain.add(erase(Id));
    chain.connect(a, b).expect("type-compatible");

    match chain.build() {
        Err(BuildError::Disconnected { stages }) => {
            assert_eq!(stages, vec![lone], "the isolated stage is reported");
        }
        Err(other) => panic!("expected BuildError::Disconnected, got {other:?}"),
        Ok(_) => panic!("expected BuildError::Disconnected, got a built pipeline"),
    }
}
