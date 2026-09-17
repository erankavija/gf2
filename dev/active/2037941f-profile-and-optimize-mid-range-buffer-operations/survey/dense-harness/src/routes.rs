//! Production routes under measurement and the facts observed about them.
//!
//! Every route reaches a public gf2 entry point or the detected kernel bundle
//! behind an optimisation barrier. The harness reads no private layout and
//! changes no production selection.

use crate::cells::{Cache, Question, SIMD_LANE_MIN_WORDS};
use gf2_core::matrix::{matvec_route, MatvecRoute};
use gf2_core::{BitMatrix, BitVec};
use gf2_kernels_simd::LogicalFns;
use std::hint::black_box;
use std::io;
use std::time::Duration;
use tuning_campaign_support::timing::{
    execution_windows_fixed_or_calibrated, TimingProgress, TimingSample,
};

/// Environment variable selecting one gf2 route in `dense-arm`.
pub const ROUTE_VAR: &str = "GF2_DENSE_ROUTE";

/// Retention bound of [`OutputSink`]; a longer window releases inside itself
/// rather than growing without limit.
pub const MAX_RETAINED_OUTPUTS: usize = 1 << 18;

/// One measured gf2 route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// First byte-identical isolated fused-parity identity arm.
    AndPopcntA,
    /// Second byte-identical isolated fused-parity identity arm.
    AndPopcntB,
    /// First byte-identical allocated `matvec` identity arm.
    MatvecA,
    /// Second byte-identical allocated `matvec` identity arm.
    MatvecB,
    /// Allocated `matvec` from the executable built without the `simd` feature.
    MatvecScalarReference,
    /// gf2 side of the external M4RI comparison.
    M4riPeerGf2,
}

impl Route {
    /// Every route, in declaration order.
    pub const ALL: [Self; 6] = [
        Self::AndPopcntA,
        Self::AndPopcntB,
        Self::MatvecA,
        Self::MatvecB,
        Self::MatvecScalarReference,
        Self::M4riPeerGf2,
    ];

    /// The arm name the plan gives this route.
    pub fn id(self) -> &'static str {
        match self {
            Self::AndPopcntA => "and-popcnt-a",
            Self::AndPopcntB => "and-popcnt-b",
            Self::MatvecA => "matvec-a",
            Self::MatvecB => "matvec-b",
            Self::MatvecScalarReference => "matvec-scalar-reference",
            Self::M4riPeerGf2 => "m4ri-peer-gf2",
        }
    }

    /// The question whose cells this route serves.
    pub fn question(self) -> Question {
        match self {
            Self::AndPopcntA | Self::AndPopcntB => Question::IsolatedFusedParity,
            Self::MatvecA | Self::MatvecB | Self::MatvecScalarReference => {
                Question::AllocatedMatvec
            }
            Self::M4riPeerGf2 => Question::MatvecVsM4ri,
        }
    }

    /// Whether this route's executable carries the `simd` feature.
    pub fn simd_build(self) -> bool {
        self != Self::MatvecScalarReference
    }

    /// Resolves a route name.
    pub fn parse(name: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|route| route.id() == name)
            .ok_or_else(|| format!("unknown {ROUTE_VAR} {name:?}"))
    }

    /// Resolves the route the campaign selected through the environment.
    pub fn from_environment() -> Result<Self, String> {
        match std::env::var(ROUTE_VAR) {
            Ok(name) => Self::parse(&name),
            Err(error) => Err(format!("{ROUTE_VAR} is unavailable: {error}")),
        }
    }

    /// Refuses a route the running executable's build cannot serve.
    ///
    /// The reference arm is the ordinary configuration with `simd` removed, so
    /// a build that carries the feature cannot stand in for it and a build that
    /// lacks it cannot stand in for the selected production route.
    pub fn check_build(self) -> Result<(), String> {
        if self.simd_build() == cfg!(feature = "simd") {
            return Ok(());
        }
        Err(format!(
            "{} needs the {} build; this executable carries simd={}",
            self.id(),
            if self.simd_build() { "simd" } else { "scalar-reference" },
            cfg!(feature = "simd")
        ))
    }
}

/// The detected kernel bundle the production `matvec` dispatches through.
pub fn fused_bundle() -> Option<LogicalFns> {
    gf2_kernels_simd::detect()
}

/// One isolated call to the bundle's fused AND-population-count entry.
///
/// The barrier around the bundle keeps the call indirect, so the arm measures
/// the dispatched entry a consumer reaches rather than a devirtualised copy.
#[inline]
pub fn fused_and_popcnt(bundle: &LogicalFns, row: &[u64], vector: &[u64]) -> u64 {
    (black_box(bundle).and_popcnt_fn)(black_box(row), black_box(vector))
}

/// One public `matvec`, including its output allocation and appends.
#[inline]
pub fn public_matvec(matrix: &BitMatrix, vector: &BitVec) -> BitVec {
    black_box(matrix).matvec(black_box(vector))
}

/// Output observation shared by every arm.
///
/// It reads the first and last written word, so the writes are observable
/// without adding a second full pass over the output to the measured cost.
#[inline]
pub fn observe_output(output: &BitVec) -> u64 {
    let words = output.words();
    match words {
        [] => black_box(0),
        [only] => black_box(*only),
        [first, .., last] => black_box(first ^ last),
    }
}

/// Retains each timed call's output so its release falls outside the window.
#[derive(Default)]
pub struct OutputSink {
    held: Vec<BitVec>,
}

impl OutputSink {
    /// Reserves room for one window's outputs, up to [`MAX_RETAINED_OUTPUTS`].
    pub fn reserve(&mut self, calls: u64) {
        let wanted = usize::try_from(calls).unwrap_or(MAX_RETAINED_OUTPUTS);
        self.held.reserve_exact(wanted.min(MAX_RETAINED_OUTPUTS));
    }

    /// Retains one output, releasing the batch first when the bound is reached.
    #[inline]
    pub fn keep(&mut self, output: BitVec) {
        if self.held.len() == self.held.capacity() {
            self.held.clear();
        }
        self.held.push(output);
    }

    /// Releases every retained output.
    pub fn release(&mut self) {
        self.held.clear();
    }

    /// Outputs the sink can retain before it releases inside a window.
    pub fn capacity(&self) -> usize {
        self.held.capacity()
    }
}

/// Facts read from a constructed public matrix and vector, outside timing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShapeFacts {
    /// Logical rows of the matrix.
    pub rows: usize,
    /// Logical columns of the matrix and length of the vector.
    pub columns: usize,
    /// Row stride in words.
    pub stride_words: usize,
    /// Route the production selector resolves for that stride.
    pub route: MatvecRoute,
    /// Whether the host detection resolved a kernel bundle.
    pub bundle: bool,
}

impl ShapeFacts {
    /// The protocol's name for the resolved lane.
    pub fn lane(self) -> &'static str {
        match self.route {
            MatvecRoute::Simd => "simd-lane",
            MatvecRoute::Scalar => "scalar-lane",
        }
    }
}

/// Verifies a constructed matrix and vector against their frozen declaration.
///
/// A mismatch makes the cell unavailable; the harness never substitutes
/// another shape.
pub fn verify_shape(
    matrix: &BitMatrix,
    vector: &BitVec,
    rows: usize,
    columns: usize,
) -> Result<ShapeFacts, String> {
    let observed = (matrix.rows(), matrix.cols(), vector.len());
    let declared = (rows, columns, columns);
    if observed != declared {
        return Err(format!(
            "observed (rows, cols, x.len) {observed:?} differs from the frozen {declared:?}"
        ));
    }
    let stride_words = matrix.stride_words();
    if stride_words != columns.div_ceil(64) {
        return Err(format!(
            "observed stride {stride_words} is not the frozen {}",
            columns.div_ceil(64)
        ));
    }
    Ok(ShapeFacts {
        rows,
        columns,
        stride_words,
        route: matvec_route(stride_words),
        bundle: fused_bundle().is_some(),
    })
}

/// Refuses a cell whose observed lane is not the one its build and stride fix.
///
/// Every anchor stride of the addendum is at or above the selector's
/// conservative threshold, so a `simd` build must observe the SIMD lane there
/// and the reference build must observe the scalar lane everywhere.
pub fn verify_lane(facts: ShapeFacts, route: Route) -> Result<(), String> {
    let wanted = if route.simd_build() && facts.stride_words >= SIMD_LANE_MIN_WORDS {
        MatvecRoute::Simd
    } else {
        MatvecRoute::Scalar
    };
    if facts.route != wanted {
        return Err(format!(
            "stride {} resolved {:?} rather than {wanted:?}",
            facts.stride_words, facts.route
        ));
    }
    if wanted == MatvecRoute::Simd && !facts.bundle {
        return Err("the host detected no kernel bundle, so the SIMD lane is unavailable".into());
    }
    Ok(())
}

/// Everything one execution of the canonical timing protocol needs.
#[derive(Clone, Copy, Debug)]
pub struct WindowPlan {
    /// Cache policy the cell declares.
    pub cache: Cache,
    /// Frozen fixed call count, or `None` when the cell calibrates.
    pub cold_calls: Option<u64>,
    /// Timing windows in this execution.
    pub windows: u32,
    /// Target length of one window, in milliseconds.
    pub window_target_ms: u32,
    /// Fixture banks the timing loop rotates through.
    pub banks: usize,
    /// Items inside one bank.
    pub items: usize,
}

/// Runs the canonical timing protocol under one cache policy.
///
/// `body(bank, item)` is the measured operation. The timing loop rotates banks
/// once per call; the item index advances once per full rotation, so a
/// streaming execution walks its whole working set. A warm policy runs one
/// untimed pass of the measured operation over its one-item working set before
/// calibration; a streaming policy runs no measured operation beforehand; a
/// cold policy calibrates nothing and uses the frozen call count.
///
/// A plan declaring zero windows is the non-timed arrangement pass the
/// deterministic smoke drives: the cache policy's declared untimed pass runs,
/// the timing protocol is never entered, and the execution reports no sample.
pub fn run_windows(
    plan: WindowPlan,
    body: &mut dyn FnMut(usize, usize),
    progress: impl FnMut(TimingProgress) -> io::Result<()>,
) -> io::Result<Vec<TimingSample>> {
    if plan.cold_calls != plan.cache.cold_calls() {
        return Err(io::Error::other(format!(
            "cell declares cache state {} with cold_calls {:?}",
            plan.cache.id(),
            plan.cold_calls
        )));
    }
    if plan.cache == Cache::Warm {
        body(0, 0);
    }
    if plan.windows == 0 {
        return Ok(Vec::new());
    }
    let (banks, items) = (plan.banks, plan.items);
    let mut call = 0_u64;
    let mut rotated = |bank: usize| {
        let item = (call / banks as u64) as usize % items;
        call += 1;
        body(bank % banks, item);
    };
    execution_windows_fixed_or_calibrated(
        0,
        u64::from(plan.windows),
        Duration::from_millis(u64::from(plan.window_target_ms)),
        plan.cold_calls,
        &mut rotated,
        progress,
    )
}
