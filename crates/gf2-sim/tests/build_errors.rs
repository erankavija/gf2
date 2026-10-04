//! [`Chain::build`](gf2_sim::graph::Chain::build) rejects a cyclic graph with
//! [`BuildError::Cyclic`] and a disconnected graph with
//! [`BuildError::Disconnected`].

use gf2_sim::error::BuildError;
use gf2_sim::graph::Chain;
use gf2_sim::stage::{erase, BatchSize, ExecutionClass, Stage};
use gf2_sim::StageError;

#[derive(Clone)]
struct Frames(Vec<u8>);
impl BatchSize for Frames {
    fn batch_size(&self) -> usize {
        self.0.len()
    }
}

/// CPU identity over [`Frames`]; matching input and output types let edges
/// run in either direction.
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
