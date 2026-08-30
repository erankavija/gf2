//! Independent importance trajectories for compressed permanental rank.
//!
//! Each trajectory opens one private canonical sampler stream, draws the next
//! row uniformly from the current contraction nullspace, advances the shared
//! [`CompressedRankState`], and accumulates the exact likelihood exponent.
//! A trajectory can be restarted at any prefix boundary by replaying its same
//! immutable address. Scheduling and worker counts never enter stream identity.

use std::fmt;

use gf2_algebra::permanent::{CompressedRankState, SupportedPrimeField, Vector3};
use gf2_core::gfp::Fp;
use gf2_stats::sampler::{
    FieldOrder, MatrixAddress, MatrixSampler, SamplerError, StreamIndex, StreamPurpose,
    STREAM_INDEX_LIMIT,
};

/// Closed schemas, canonical bytes, checksums, lineage, and atomic publication.
pub mod artifact;

/// Frozen campaign root shared with the manifested permanent campaign.
pub const RARE_EVENT_ROOT_SEED: u64 = 0x7a81_6262_0000_0001;
/// Manifested stream-purpose tag for rare-event work.
pub const RARE_EVENT_PURPOSE_TAG: u8 = 4;
/// Target-run count.
pub const TARGET_RUNS: u16 = 32;
/// Target trajectories per run.
pub const TARGET_TRAJECTORIES_PER_RUN: u32 = 16_384;
/// Target checkpoint block size.
pub const TARGET_BLOCK_SIZE: u32 = 256;
/// Coverage replicates per field.
pub const COVERAGE_REPLICATES: u16 = 200;
/// Coverage independent runs per replicate.
pub const COVERAGE_RUNS: u16 = 32;
/// Coverage trajectories per run and checkpoint block.
pub const COVERAGE_TRAJECTORIES_PER_RUN: u32 = 4_096;
/// First coverage stream index.
pub const COVERAGE_INDEX_START: u64 = 1 << 54;
/// First deterministic-test stream index.
pub const TEST_INDEX_START: u64 = 1 << 55;
/// First reserved, unallocated stream index.
pub const RESERVED_INDEX_START: u64 = 3 << 54;

/// Returns one canonical target matrix address.
pub fn target_address(run: u16, trajectory: u32) -> Result<MatrixAddress, RareEventError> {
    if run >= TARGET_RUNS || trajectory >= TARGET_TRAJECTORIES_PER_RUN {
        return Err(RareEventError::AddressOutOfRange);
    }
    matrix_address(
        FieldOrder::F3,
        1_024,
        (u64::from(run) << 14) | u64::from(trajectory),
    )
}

/// Returns one canonical coverage matrix address.
pub fn coverage_address(
    field_order: u8,
    replicate: u16,
    run: u16,
    trajectory: u32,
) -> Result<MatrixAddress, RareEventError> {
    let field_order = field_order_from_u8(field_order)?;
    if replicate >= COVERAGE_REPLICATES
        || run >= COVERAGE_RUNS
        || trajectory >= COVERAGE_TRAJECTORIES_PER_RUN
    {
        return Err(RareEventError::AddressOutOfRange);
    }
    matrix_address(
        field_order,
        3,
        COVERAGE_INDEX_START
            | (u64::from(replicate) << 17)
            | (u64::from(run) << 12)
            | u64::from(trajectory),
    )
}

/// Returns one canonical deterministic-test matrix address.
pub fn deterministic_test_address(
    field_order: u8,
    dimension: usize,
    case: u16,
) -> Result<MatrixAddress, RareEventError> {
    let field_order = field_order_from_u8(field_order)?;
    let slot = match field_order {
        FieldOrder::F3 => 0,
        FieldOrder::F5 => 1,
        FieldOrder::F7 => 2,
    };
    matrix_address(
        field_order,
        dimension,
        TEST_INDEX_START | (slot << 16) | u64::from(case),
    )
}

fn matrix_address(
    field_order: FieldOrder,
    dimension: usize,
    raw_index: u64,
) -> Result<MatrixAddress, RareEventError> {
    if raw_index >= STREAM_INDEX_LIMIT
        || (RESERVED_INDEX_START..STREAM_INDEX_LIMIT).contains(&raw_index)
    {
        return Err(RareEventError::AddressOutOfRange);
    }
    Ok(MatrixAddress::new(
        RARE_EVENT_ROOT_SEED,
        field_order,
        dimension,
        StreamPurpose::RareEvent,
        StreamIndex::new(raw_index).map_err(|_| RareEventError::AddressOutOfRange)?,
    ))
}

/// A proposal construction or trajectory validation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RareEventError {
    /// A semantic address lies outside its fixed partition.
    AddressOutOfRange,
    /// A field outside `3`, `5`, and `7` was supplied.
    UnsupportedField(u8),
    /// The sampler address field differs from the trajectory field.
    SamplerFieldMismatch,
    /// The sampler address dimension differs from the requested trajectory length.
    DimensionMismatch {
        /// Requested trajectory row count.
        requested: usize,
        /// Dimension encoded in the canonical matrix address.
        addressed: usize,
    },
    /// A resume boundary lies after the trajectory terminal row.
    ResumeAfterTerminal {
        /// Requested completed-prefix boundary.
        boundary: usize,
        /// Terminal row count.
        rows: usize,
    },
    /// A worker count of zero was supplied.
    ZeroWorkers,
}

impl fmt::Display for RareEventError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AddressOutOfRange => {
                formatter.write_str("rare-event address is outside its fixed subdomain")
            }
            Self::UnsupportedField(order) => {
                write!(formatter, "field order {order} is not supported")
            }
            Self::SamplerFieldMismatch => {
                formatter.write_str("trajectory field differs from its sampler address")
            }
            Self::DimensionMismatch {
                requested,
                addressed,
            } => write!(
                formatter,
                "trajectory has {requested} rows but its address dimension is {addressed}"
            ),
            Self::ResumeAfterTerminal { boundary, rows } => write!(
                formatter,
                "resume boundary {boundary} exceeds terminal row {rows}"
            ),
            Self::ZeroWorkers => formatter.write_str("worker count must be positive"),
        }
    }
}

impl std::error::Error for RareEventError {}

impl From<SamplerError> for RareEventError {
    fn from(_: SamplerError) -> Self {
        Self::SamplerFieldMismatch
    }
}

/// Exact support and likelihood increment for one proposal step.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalSupport<F: SupportedPrimeField> {
    rows: Vec<Vector3<F>>,
    likelihood_exponent: u8,
}

impl<F: SupportedPrimeField> ProposalSupport<F> {
    /// Enumerates the canonical nullspace support of `state`.
    ///
    /// Rows are in coefficient-lexicographic order. The support contains
    /// exactly `q^(3-dim(V))` rows, including zero, and the target-to-proposal
    /// likelihood increment is exactly `q^-dim(V)`.
    #[must_use]
    pub fn for_state(state: &CompressedRankState<F>) -> Self {
        let nullspace = state.contraction_span().orthogonal_complement();
        let basis = nullspace.basis_residues();
        let dimension = nullspace.dimension();
        let count = usize::from(F::ORDER).pow(dimension as u32);
        let mut rows = Vec::with_capacity(count);
        for encoded in 0..count {
            let mut remaining = encoded;
            let mut residues = [0_u8; 3];
            for basis_row in basis.iter().take(dimension) {
                let coefficient = (remaining % usize::from(F::ORDER)) as u8;
                remaining /= usize::from(F::ORDER);
                for coordinate in 0..3 {
                    residues[coordinate] =
                        (residues[coordinate] + coefficient * basis_row[coordinate]) % F::ORDER;
                }
            }
            rows.push(Vector3::from_residues(residues).expect("proposal residues are canonical"));
        }
        Self {
            rows,
            likelihood_exponent: state.contraction_span().dimension() as u8,
        }
    }

    /// Returns every supported row exactly once.
    #[must_use]
    pub fn rows(&self) -> &[Vector3<F>] {
        &self.rows
    }

    /// Returns `dim(V)`, so the exact increment is `q^-likelihood_exponent`.
    #[must_use]
    pub const fn likelihood_exponent(&self) -> u8 {
        self.likelihood_exponent
    }
}

/// Canonical terminal record for one importance trajectory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrajectoryOutcome {
    /// Sum of contraction-space dimensions before each transition.
    pub exponent: u32,
    /// Number of completed prefix rows.
    pub rows_completed: usize,
    /// Recomputed low-56-bit stream index.
    pub stream_index: u64,
    /// Canonical terminal compressed-state bytes.
    pub terminal_state: [u8; 22],
}

/// Samples one complete addressed trajectory.
///
/// # Errors
///
/// Rejects a mismatched field or dimension. Every valid invocation returns one
/// record; repeated or zero rows are retained like every other draw.
///
/// # Complexity
///
/// `O(rows)` fixed-three-coordinate field operations.
pub fn sample_trajectory<const Q: u64>(
    rows: usize,
    address: MatrixAddress,
) -> Result<TrajectoryOutcome, RareEventError>
where
    Fp<Q>: SupportedPrimeField,
{
    run_trajectory::<Q>(rows, address, 0)
}

/// Restarts one trajectory from its immutable address and replays through a boundary.
///
/// Replaying is intentional: only a complete published block is durable, so
/// an unpublished trajectory always restarts from its original stream address.
pub fn resume_trajectory<const Q: u64>(
    rows: usize,
    address: MatrixAddress,
    completed_prefix: usize,
) -> Result<TrajectoryOutcome, RareEventError>
where
    Fp<Q>: SupportedPrimeField,
{
    run_trajectory::<Q>(rows, address, completed_prefix)
}

/// Samples addresses and returns outcomes in canonical input order.
///
/// `workers` affects only assignment validation, not stream addresses or
/// reduction order. This makes results identical across schedules.
pub fn sample_trajectories_in_order<const Q: u64>(
    rows: usize,
    addresses: &[MatrixAddress],
    workers: usize,
) -> Result<Vec<TrajectoryOutcome>, RareEventError>
where
    Fp<Q>: SupportedPrimeField,
{
    if workers == 0 {
        return Err(RareEventError::ZeroWorkers);
    }
    addresses
        .iter()
        .copied()
        .map(|address| sample_trajectory::<Q>(rows, address))
        .collect()
}

fn run_trajectory<const Q: u64>(
    rows: usize,
    address: MatrixAddress,
    completed_prefix: usize,
) -> Result<TrajectoryOutcome, RareEventError>
where
    Fp<Q>: SupportedPrimeField,
{
    if completed_prefix > rows {
        return Err(RareEventError::ResumeAfterTerminal {
            boundary: completed_prefix,
            rows,
        });
    }
    if address.dimension() != rows {
        return Err(RareEventError::DimensionMismatch {
            requested: rows,
            addressed: address.dimension(),
        });
    }
    if address.purpose() != StreamPurpose::RareEvent {
        return Err(RareEventError::AddressOutOfRange);
    }
    let mut sampler = MatrixSampler::<Q>::new(address)?;
    let mut state = CompressedRankState::<Fp<Q>>::initial();
    let mut exponent = 0_u32;
    for prefix in 0..rows {
        let nullspace = state.contraction_span().orthogonal_complement();
        let basis = nullspace.basis_residues();
        let mut residues = [0_u8; 3];
        for basis_row in basis.iter().take(nullspace.dimension()) {
            let coefficient = sampler.next_entry().value() as u8;
            for coordinate in 0..3 {
                residues[coordinate] =
                    (residues[coordinate] + coefficient * basis_row[coordinate]) % Q as u8;
            }
        }
        exponent += state.contraction_span().dimension() as u32;
        let row = Vector3::from_residues(residues).expect("sampled residues are canonical");
        state = state.successor(row).expect("a nullspace row is admissible");
        if prefix + 1 == completed_prefix {
            // The boundary is observable for debugger/checkpoint fixtures; the
            // stream remains private and continuation uses the same state.
            debug_assert_eq!(prefix + 1, completed_prefix);
        }
    }
    Ok(TrajectoryOutcome {
        exponent,
        rows_completed: rows,
        stream_index: address.stream().get(),
        terminal_state: state.to_canonical_bytes(),
    })
}

fn field_order_from_u8(order: u8) -> Result<FieldOrder, RareEventError> {
    match order {
        3 => Ok(FieldOrder::F3),
        5 => Ok(FieldOrder::F5),
        7 => Ok(FieldOrder::F7),
        _ => Err(RareEventError::UnsupportedField(order)),
    }
}
