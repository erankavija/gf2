//! Production routes under measurement and the facts observed about them.
//!
//! Every route calls a public gf2 entry point. The harness reads no private
//! layout, copies no row into an aligned private representation and changes no
//! production selection.

use crate::cells::{Cache, NrTarget, Question, ROW_PAIRS};
use crate::fixture::AlignedSlab;
use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedCode;
use gf2_coding::ldpc::QuasiCyclicLdpc;
use gf2_core::kernels::ops::{resolve_xor_inplace, xor_inplace, XorInplaceFn};
use gf2_core::kernels::select_backend_for_size;
use gf2_core::BitMatrix;
use std::hint::black_box;
use std::io;
use std::time::Duration;
use tuning_campaign_support::protocol::sha256_hex;
use tuning_campaign_support::timing::{
    execution_windows_fixed_or_calibrated, TimingProgress, TimingSample,
};

/// Environment variable selecting one gf2 route in `logical-arm`.
pub const ROUTE_VAR: &str = "GF2_LOGICAL_ROUTE";

/// One measured gf2 route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// First byte-identical public isolated-XOR identity arm.
    PublicXorA,
    /// Second byte-identical public isolated-XOR identity arm.
    PublicXorB,
    /// Isolated XOR through a function pointer resolved outside timing.
    ResolvedXor,
    /// First byte-identical public row-XOR identity arm.
    RowXorA,
    /// Second byte-identical public row-XOR identity arm.
    RowXorB,
    /// First public NR constructor arm.
    NrConstructA,
    /// Second public NR constructor arm.
    NrConstructB,
    /// Operation-equivalent gf2 side of the ISA-L scalar gap.
    IsalPeerGf2,
}

impl Route {
    /// Every route, in declaration order.
    pub const ALL: [Self; 8] = [
        Self::PublicXorA,
        Self::PublicXorB,
        Self::ResolvedXor,
        Self::RowXorA,
        Self::RowXorB,
        Self::NrConstructA,
        Self::NrConstructB,
        Self::IsalPeerGf2,
    ];

    /// The arm name the plan gives this route.
    pub fn id(self) -> &'static str {
        match self {
            Self::PublicXorA => "public-xor-a",
            Self::PublicXorB => "public-xor-b",
            Self::ResolvedXor => "resolved-xor",
            Self::RowXorA => "row-xor-a",
            Self::RowXorB => "row-xor-b",
            Self::NrConstructA => "nr-construct-a",
            Self::NrConstructB => "nr-construct-b",
            Self::IsalPeerGf2 => "isal-peer-gf2",
        }
    }

    /// The question whose cells this route serves.
    pub fn question(self) -> Question {
        match self {
            Self::PublicXorA | Self::PublicXorB | Self::ResolvedXor => Question::IsolatedXor,
            Self::RowXorA | Self::RowXorB => Question::PublicRowXor,
            Self::NrConstructA | Self::NrConstructB => Question::NrConstruction,
            Self::IsalPeerGf2 => Question::IsalBaseGap,
        }
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
}

/// The backend `xor_inplace` resolves for a word count, observed at run time.
pub fn backend_name(words: usize) -> &'static str {
    select_backend_for_size(words).name()
}

/// Resolves the in-place XOR once, for the hoisted attribution route.
pub fn resolved_xor(words: usize) -> XorInplaceFn {
    resolve_xor_inplace(words)
}

/// One isolated logical operation through the public API.
#[inline]
pub fn public_xor(dst: &mut [u64], src: &[u64]) {
    xor_inplace(black_box(dst), black_box(src));
}

/// One isolated logical operation through a pointer resolved outside timing.
#[inline]
pub fn resolved_xor_apply(resolved: XorInplaceFn, dst: &mut [u64], src: &[u64]) {
    resolved(black_box(dst), black_box(src));
}

/// One public row XOR of the directed pair at `index` in the frozen cycle.
#[inline]
pub fn public_row_xor(matrix: &mut BitMatrix, index: usize) {
    let (dst, src) = ROW_PAIRS[index % ROW_PAIRS.len()];
    black_box(matrix).row_xor(dst, src);
}

/// One whole public NR construction.
#[inline]
pub fn nr_construct(target: &NrTarget) -> Nr5gRateMatchedCode {
    QuasiCyclicLdpc::nr_5g_rate_matched(
        black_box(target.base_graph),
        black_box(target.target_n),
        black_box(target.target_k),
    )
}

/// A fresh 64-byte-aligned destination for one comparison call.
///
/// Both sides of the ISA-L comparison arrange their destination through this
/// helper, so the allocation cost inside a timed call is matched. The block is
/// zeroed because writing through a reference to uninitialized memory is not
/// sound in Rust; both arms pay the same zeroing.
#[inline]
pub fn fresh_destination(words: usize) -> AlignedSlab {
    AlignedSlab::zeroed(words)
}

/// The gf2 side of the ISA-L comparison, inside one timed call.
///
/// Destination arrangement, the copy of source zero, the public in-place XOR
/// with source one and output observation are all inside the call, exactly as
/// the comparator specification's cost boundary requires.
#[inline]
pub fn gap_gf2_operation(src0: &[u64], src1: &[u64]) -> u64 {
    let mut destination = fresh_destination(src0.len());
    let words = destination.as_mut_slice();
    words.copy_from_slice(black_box(src0));
    xor_inplace(words, black_box(src1));
    observe_output(words)
}

/// Output observation shared by both sides of a comparison.
///
/// It reads the first and last written word, so the writes are observable
/// without adding a second full pass over the buffer to the measured cost.
#[inline]
pub fn observe_output(words: &[u64]) -> u64 {
    black_box(words[0] ^ words[words.len() - 1])
}

/// Facts read from a returned public NR object, outside timing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NrFacts {
    /// Lifting factor the public constructor selected.
    pub lifting_factor: usize,
    /// Dense parity-check rows.
    pub dense_rows: usize,
    /// Dense parity-check columns.
    pub dense_cols: usize,
    /// Dense row stride in words.
    pub stride_words: usize,
    /// Non-zeros of the sparse parity-check structure.
    pub nnz: usize,
    /// Stable digest of that sparse structure.
    pub structure_digest: String,
}

/// Reads the observable parameters and structure digest of a returned code.
pub fn observe_nr(code: &Nr5gRateMatchedCode) -> NrFacts {
    let mother = code.mother_code();
    let parity = mother.parity_check_matrix();
    let rows = parity.rows();
    let cols = parity.cols();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(rows as u64).to_le_bytes());
    bytes.extend_from_slice(&(cols as u64).to_le_bytes());
    for row in 0..rows {
        let mut columns: Vec<u32> = parity.row_iter(row).map(|col| col as u32).collect();
        columns.sort_unstable();
        bytes.extend_from_slice(&(columns.len() as u32).to_le_bytes());
        for column in columns {
            bytes.extend_from_slice(&column.to_le_bytes());
        }
    }
    NrFacts {
        lifting_factor: code.params().lifting_factor,
        dense_rows: rows,
        dense_cols: cols,
        stride_words: cols.div_ceil(64),
        nnz: parity.nnz(),
        structure_digest: sha256_hex(&bytes),
    }
}

/// Verifies a returned public object against its frozen route declaration.
///
/// A mismatch makes the cell unavailable; the harness never substitutes
/// another target.
pub fn verify_nr(code: &Nr5gRateMatchedCode, target: &NrTarget) -> Result<NrFacts, String> {
    let facts = observe_nr(code);
    let declared = (
        target.lifting_factor,
        target.dense_rows,
        target.dense_cols,
        target.stride_words,
    );
    let observed = (
        facts.lifting_factor,
        facts.dense_rows,
        facts.dense_cols,
        facts.stride_words,
    );
    if declared != observed {
        return Err(format!(
            "{}: observed (Z, rows, cols, stride) {observed:?} differs from the frozen {declared:?}",
            target.suffix
        ));
    }
    if code.n() != target.target_n || code.k() != target.target_k {
        return Err(format!(
            "{}: observed (n, k) ({}, {}) differs from the frozen ({}, {})",
            target.suffix,
            code.n(),
            code.k(),
            target.target_n,
            target.target_k
        ));
    }
    Ok(facts)
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
