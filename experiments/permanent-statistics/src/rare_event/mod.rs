//! Importance trajectories for compressed permanental rank: uniform draws from
//! the contraction nullspace with exact likelihood exponents and deterministic
//! addressed replay across prefix boundaries and worker counts.

use std::fmt;

use gf2_algebra::permanent::{CompressedRankState, SupportedPrimeField, Vector3};
use gf2_core::gfp::Fp;
use gf2_stats::sampler::{MatrixAddress, MatrixSampler, SamplerError, StreamPurpose};

pub mod runner;

/// A proposal construction or trajectory validation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RareEventError {
    /// The address does not select rare-event sampling.
    PurposeMismatch,
    /// Three times the row count cannot be represented by the likelihood exponent.
    LengthOutOfRange,
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
            Self::PurposeMismatch => {
                formatter.write_str("address purpose must be rare-event sampling")
            }
            Self::LengthOutOfRange => {
                formatter.write_str("three times the row count must fit in u32")
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
/// Rejects a mismatched field, dimension, or purpose, and row counts whose
/// maximum exponent exceeds `u32`. Repeated or zero rows are retained.
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
    validate_trajectory_address::<Q>(rows, address)?;
    let mut cursor = TrajectoryCursor::<Q>::new(address, 0)?;
    cursor.advance(rows);
    Ok(outcome(rows, address, cursor.state, cursor.exponent))
}

/// A stopped addressed trajectory prefix used to test and implement restart.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrajectoryPrefixCheckpoint {
    /// Number of prefix rows consumed.
    pub completed_prefix: usize,
    /// Exact likelihood exponent accumulated through that prefix.
    pub exponent: u32,
    /// Canonical compressed state at the stop boundary.
    pub state: [u8; 22],
}

/// Stops a trajectory after exactly `completed_prefix` rows.
///
/// # Errors
///
/// Returns the address errors of [`sample_trajectory`] or a boundary after the
/// terminal row. Work is linear in the completed prefix length.
pub fn checkpoint_trajectory<const Q: u64>(
    rows: usize,
    address: MatrixAddress,
    completed_prefix: usize,
) -> Result<TrajectoryPrefixCheckpoint, RareEventError>
where
    Fp<Q>: SupportedPrimeField,
{
    validate_trajectory_address::<Q>(rows, address)?;
    let cursor = TrajectoryCursor::<Q>::new(address, completed_prefix)?;
    Ok(TrajectoryPrefixCheckpoint {
        completed_prefix,
        exponent: cursor.exponent,
        state: cursor.state.to_canonical_bytes(),
    })
}

/// Restarts one trajectory from its immutable address and replays through a boundary.
///
/// The sampler reconstructs its state by replaying the completed prefix once.
/// Completed trajectories in the runner are retained as exponent histograms.
///
/// # Errors
///
/// Returns the same errors as [`checkpoint_trajectory`]. Work is linear in
/// `rows`, including the prefix replay.
pub fn resume_trajectory<const Q: u64>(
    rows: usize,
    address: MatrixAddress,
    completed_prefix: usize,
) -> Result<TrajectoryOutcome, RareEventError>
where
    Fp<Q>: SupportedPrimeField,
{
    validate_trajectory_address::<Q>(rows, address)?;
    let mut cursor = TrajectoryCursor::<Q>::new(address, completed_prefix)?;
    cursor.advance(rows - completed_prefix);
    Ok(outcome(rows, address, cursor.state, cursor.exponent))
}

/// Samples addresses and returns outcomes in canonical input order.
///
/// Addresses are assigned round-robin to distinct logical worker lanes, then
/// reassembled by original address position. This makes scheduling paths
/// distinct while preserving one canonical reduction order.
///
/// # Errors
///
/// Rejects zero workers and the address errors of [`sample_trajectory`]. Work
/// is linear in `rows * addresses.len() + workers`.
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
    let mut outcomes = vec![None; addresses.len()];
    for worker in 0..workers {
        for index in (worker..addresses.len()).step_by(workers) {
            outcomes[index] = Some(sample_trajectory::<Q>(rows, addresses[index])?);
        }
    }
    Ok(outcomes
        .into_iter()
        .map(|outcome| outcome.expect("round-robin lanes cover every address"))
        .collect())
}

fn validate_boundary(rows: usize, completed_prefix: usize) -> Result<(), RareEventError> {
    if completed_prefix > rows {
        return Err(RareEventError::ResumeAfterTerminal {
            boundary: completed_prefix,
            rows,
        });
    }
    Ok(())
}

fn validate_trajectory_address<const Q: u64>(
    rows: usize,
    address: MatrixAddress,
) -> Result<(), RareEventError>
where
    Fp<Q>: SupportedPrimeField,
{
    if address.dimension() != rows {
        return Err(RareEventError::DimensionMismatch {
            requested: rows,
            addressed: address.dimension(),
        });
    }
    if address.purpose() != StreamPurpose::RareEvent {
        return Err(RareEventError::PurposeMismatch);
    }
    if rows > (u32::MAX / 3) as usize {
        return Err(RareEventError::LengthOutOfRange);
    }
    MatrixSampler::<Q>::new(address)?;
    Ok(())
}

struct TrajectoryCursor<const Q: u64>
where
    Fp<Q>: SupportedPrimeField,
{
    sampler: MatrixSampler<Q>,
    state: CompressedRankState<Fp<Q>>,
    exponent: u32,
}

impl<const Q: u64> TrajectoryCursor<Q>
where
    Fp<Q>: SupportedPrimeField,
{
    fn new(address: MatrixAddress, prefix: usize) -> Result<Self, RareEventError> {
        validate_trajectory_address::<Q>(address.dimension(), address)?;
        validate_boundary(address.dimension(), prefix)?;
        let mut cursor = Self {
            sampler: MatrixSampler::new(address)?,
            state: CompressedRankState::initial(),
            exponent: 0,
        };
        cursor.advance(prefix);
        Ok(cursor)
    }

    fn advance(&mut self, rows: usize) {
        for _ in 0..rows {
            let nullspace = self.state.contraction_span().orthogonal_complement();
            let basis = nullspace.basis_residues();
            let mut residues = [0_u8; 3];
            for basis_row in basis.iter().take(nullspace.dimension()) {
                let coefficient = self.sampler.next_entry().value() as u8;
                for coordinate in 0..3 {
                    residues[coordinate] =
                        (residues[coordinate] + coefficient * basis_row[coordinate]) % Q as u8;
                }
            }
            self.exponent += self.state.contraction_span().dimension() as u32;
            let row = Vector3::from_residues(residues).expect("sampled residues are canonical");
            self.state = self
                .state
                .successor(row)
                .expect("a nullspace row is admissible");
        }
    }
}

fn outcome<F: SupportedPrimeField>(
    rows: usize,
    address: MatrixAddress,
    state: CompressedRankState<F>,
    exponent: u32,
) -> TrajectoryOutcome {
    TrajectoryOutcome {
        exponent,
        rows_completed: rows,
        stream_index: address.stream().get(),
        terminal_state: state.to_canonical_bytes(),
    }
}
