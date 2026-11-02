use super::dataset::ShardRecord;
use super::{invalid, CampaignError, Cell, ShardSpec};
use gf2_algebra::packed::{Bipedal3Matrix, Packed5Matrix, Packed7Matrix};
use gf2_algebra::permanent::bipedal5::permanent_bipedal5_singleword;
use gf2_algebra::permanent::bipedal7::permanent_bipedal7_singleword;
use gf2_algebra::permanent::{
    permanent_bipedal3_parallel, permanent_bipedal3_singleword, permanent_ryser,
};
use gf2_core::{
    field::{matrix::FieldMatrix, FieldVec},
    gfp::Fp,
};
use gf2_stats::sampler::{FieldOrder, MatrixAddress, MatrixSampler, StreamIndex, StreamPurpose};
use rayon::{prelude::*, ThreadPool, ThreadPoolBuilder};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Implementation named by a cell; availability is checked independently.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    /// Serial specialized kernel.
    Scalar,
    /// Independent matrices on a local Rayon pool.
    BatchParallel,
    /// Parallel F3 permanent within one matrix.
    IntraMatrixParallel,
    /// Generic finite-field reference algorithm.
    GenericRyser,
    /// HIP batch dispatch.
    Accelerator,
}
impl std::str::FromStr for Backend {
    type Err = CampaignError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_value(serde_json::Value::String(s.into())).map_err(Into::into)
    }
}
/// Concrete kernel selected by the cell's mathematical domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessorPath {
    /// F3 single-word kernel.
    Bipedal3SingleWord,
    /// F5 single-word kernel.
    Bipedal5SingleWord,
    /// F7 single-word kernel.
    Bipedal7SingleWord,
    /// F3 intra-matrix parallel kernel.
    Bipedal3IntraMatrixParallel,
    /// Generic reference.
    GenericRyser,
    /// HIP batch.
    Accelerator,
}
/// Pure mathematical domain rule, independent of the host and build.
pub fn backend_supports_cell(backend: Backend, q: u8, n: u16) -> bool {
    match backend {
        Backend::Scalar | Backend::BatchParallel => match q {
            3 | 5 => n <= 63,
            7 => n <= 16,
            _ => false,
        },
        Backend::IntraMatrixParallel => q == 3 && n <= 63,
        Backend::GenericRyser | Backend::Accelerator => matches!(q, 3 | 5 | 7) && n <= 63,
    }
}

/// Resolves the mathematical domain and build feature without probing the host.
pub fn resolve_processor_path(
    q: u8,
    n: u16,
    backend: Backend,
) -> Result<ProcessorPath, CampaignError> {
    if !backend_supports_cell(backend, q, n) {
        return Err(CampaignError::BackendUnavailable);
    }
    Ok(match backend {
        Backend::Scalar | Backend::BatchParallel => match q {
            3 => ProcessorPath::Bipedal3SingleWord,
            5 => ProcessorPath::Bipedal5SingleWord,
            7 => ProcessorPath::Bipedal7SingleWord,
            _ => unreachable!(),
        },
        Backend::IntraMatrixParallel => ProcessorPath::Bipedal3IntraMatrixParallel,
        Backend::GenericRyser => ProcessorPath::GenericRyser,
        Backend::Accelerator => {
            #[cfg(feature = "hip")]
            {
                ProcessorPath::Accelerator
            }
            #[cfg(not(feature = "hip"))]
            {
                return Err(CampaignError::BackendUnavailable);
            }
        }
    })
}
/// Whether the backend can execute on this build and host.
pub fn backend_available(backend: Backend) -> bool {
    if backend != Backend::Accelerator {
        return true;
    }
    #[cfg(feature = "hip")]
    {
        gf2_algebra::gpu::has_usable_device()
    }
    #[cfg(not(feature = "hip"))]
    {
        false
    }
}
/// Maximum launch length under a measured per-matrix cost; always at least one.
pub fn launch_size(per_matrix: Duration, cap: Duration, remaining: u64) -> usize {
    if remaining == 0 {
        return 1;
    }
    let remaining_as_usize = remaining.min(usize::MAX as u64) as usize;
    if per_matrix.is_zero() {
        return remaining_as_usize.max(1);
    }
    let quotient = cap.as_nanos() / per_matrix.as_nanos();
    let requested = quotient.min(usize::MAX as u128) as usize;
    requested.clamp(1, remaining_as_usize.max(1))
}

enum PackedMatrix {
    F3(Bipedal3Matrix),
    F5(Packed5Matrix),
    F7(Packed7Matrix),
}

impl PackedMatrix {
    fn new<const Q: u64>(entries: &[Fp<Q>], n: usize) -> Self {
        match Q {
            3 => {
                let values: Vec<Fp<3>> =
                    entries.iter().map(|value| Fp::new(value.value())).collect();
                Self::F3(Bipedal3Matrix::from_row_major(&values, n, n))
            }
            5 => {
                let values: Vec<Fp<5>> =
                    entries.iter().map(|value| Fp::new(value.value())).collect();
                Self::F5(Packed5Matrix::from_row_major(&values, n, n))
            }
            7 => {
                let values: Vec<Fp<7>> =
                    entries.iter().map(|value| Fp::new(value.value())).collect();
                Self::F7(Packed7Matrix::from_row_major(&values, n, n))
            }
            _ => unreachable!("campaign q is validated before packing"),
        }
    }
}

/// Reusable production evaluator for sampling and exact validation.
pub struct Kernel {
    q: u8,
    n: usize,
    processor_path: ProcessorPath,
    pool: Option<ThreadPool>,
}

impl Kernel {
    /// Checks availability and builds a bounded local pool.
    pub fn new(
        q: u8,
        n: u16,
        backend: Backend,
        worker_count: usize,
    ) -> Result<Self, CampaignError> {
        if worker_count == 0 {
            return invalid("workers must be positive");
        }
        if !backend_available(backend) {
            return Err(CampaignError::BackendUnavailable);
        }
        let processor_path = resolve_processor_path(q, n, backend)?;
        let pool = if backend == Backend::BatchParallel {
            Some(
                ThreadPoolBuilder::new()
                    .num_threads(worker_count)
                    .build()
                    .map_err(|error| {
                        CampaignError::Invalid(format!("cannot build batch thread pool: {error}"))
                    })?,
            )
        } else {
            None
        };
        Ok(Self {
            q,
            n: usize::from(n),
            processor_path,
            pool,
        })
    }

    /// Evaluates caller-supplied matrices in input order; refuses shape or field mismatch.
    ///
    /// # Panics
    /// HIP dispatch failures propagate from the accelerator kernel.
    ///
    /// Packing uses `O(batch_len * n^2)` temporary storage.
    pub fn permanents<const Q: u64>(
        &self,
        matrices: &[Vec<Fp<Q>>],
    ) -> Result<Vec<u64>, CampaignError> {
        if Q != u64::from(self.q) {
            return Err(CampaignError::Invalid(format!(
                "evaluator field q={} cannot consume F_{Q} matrices",
                self.q
            )));
        }
        let entry_count = self.n.checked_mul(self.n).ok_or_else(|| {
            CampaignError::Invalid("matrix dimension overflows entry count".to_owned())
        })?;
        if let Some(index) = matrices
            .iter()
            .position(|entries| entries.len() != entry_count)
        {
            return Err(CampaignError::Invalid(format!(
                "matrix {index} has {} entries, expected {entry_count}",
                matrices[index].len()
            )));
        }
        #[cfg(feature = "hip")]
        if self.processor_path == ProcessorPath::Accelerator {
            if !gf2_algebra::gpu::has_usable_device() {
                return Err(CampaignError::BackendUnavailable);
            }
            return dispatch_accelerator(matrices, self.n);
        }

        let evaluate_one = |entries: &Vec<Fp<Q>>| {
            let packed = if self.processor_path == ProcessorPath::GenericRyser {
                None
            } else {
                Some(PackedMatrix::new(entries, self.n))
            };
            evaluate_permanent(self.processor_path, entries, packed.as_ref(), self.n)
        };
        if let Some(pool) = &self.pool {
            pool.install(|| matrices.par_iter().map(evaluate_one).collect())
        } else {
            matrices.iter().map(evaluate_one).collect()
        }
    }
}

fn evaluate_permanent<const Q: u64>(
    processor_path: ProcessorPath,
    row_major: &[Fp<Q>],
    packed: Option<&PackedMatrix>,
    n: usize,
) -> Result<u64, CampaignError> {
    let value = match processor_path {
        ProcessorPath::Bipedal3SingleWord => match packed {
            Some(PackedMatrix::F3(matrix)) => permanent_bipedal3_singleword(matrix).value(),
            _ => {
                return Err(CampaignError::Invalid(
                    "packed matrix and F_3 processor path do not agree".to_owned(),
                ));
            }
        },
        ProcessorPath::Bipedal5SingleWord => match packed {
            Some(PackedMatrix::F5(matrix)) => permanent_bipedal5_singleword(matrix).value(),
            _ => {
                return Err(CampaignError::Invalid(
                    "packed matrix and F_5 processor path do not agree".to_owned(),
                ));
            }
        },
        ProcessorPath::Bipedal7SingleWord => match packed {
            Some(PackedMatrix::F7(matrix)) => permanent_bipedal7_singleword(matrix).value(),
            _ => {
                return Err(CampaignError::Invalid(
                    "packed matrix and F_7 processor path do not agree".to_owned(),
                ));
            }
        },
        ProcessorPath::Bipedal3IntraMatrixParallel => match packed {
            Some(PackedMatrix::F3(matrix)) => permanent_bipedal3_parallel(matrix).value(),
            _ => {
                return Err(CampaignError::Invalid(
                    "packed matrix and F_3 processor path do not agree".to_owned(),
                ));
            }
        },
        ProcessorPath::GenericRyser => permanent_ryser(row_major, n).value(),
        ProcessorPath::Accelerator => {
            return Err(CampaignError::Invalid(
                "accelerator path must use batched dispatch".to_owned(),
            ));
        }
    };
    Ok(value)
}

/// Determinant of a square row-major matrix in `O(n^3)` time.
///
/// # Panics
/// Panics unless `row_major` has `n * n` entries.
pub fn evaluate_determinant<const Q: u64>(row_major: &[Fp<Q>], n: usize) -> u64 {
    if n == 0 {
        return 1;
    }
    let rows: Vec<FieldVec<Fp<Q>>> = row_major.chunks(n).map(|row| row.to_vec().into()).collect();
    FieldMatrix::from_rows(rows).det().value()
}

#[cfg(feature = "hip")]
fn dispatch_accelerator<const Q: u64>(
    matrices: &[Vec<Fp<Q>>],
    n: usize,
) -> Result<Vec<u64>, CampaignError> {
    let packed: Vec<_> = matrices.iter().map(|m| PackedMatrix::new(m, n)).collect();
    macro_rules! dispatch {
        ($variant:ident, $function:ident) => {{
            let operands: Vec<_> = packed
                .into_iter()
                .map(|m| match m {
                    PackedMatrix::$variant(m) => m,
                    _ => unreachable!(),
                })
                .collect();
            gf2_algebra::gpu::$function(&operands)
                .into_iter()
                .map(|v| v.value())
                .collect()
        }};
    }
    Ok(match Q {
        3 => dispatch!(F3, permanent_batch_bipedal3),
        5 => dispatch!(F5, permanent_batch_bipedal5),
        7 => dispatch!(F7, permanent_batch_bipedal7),
        _ => return invalid("unsupported accelerator field"),
    })
}

/// Evaluates one stream through a reused bounded matrix buffer.
/// The observer runs in address order on the caller thread. Errors leave the
/// stream unaccepted so a resumed invocation draws the identical stream.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_shard<const Q: u64, O: FnMut(&[Fp<Q>], u64, Option<u64>)>(
    root_seed: u64,
    purpose: StreamPurpose,
    cell: &Cell,
    shard: ShardSpec,
    backend: Backend,
    workers: usize,
    launch_cap_ms: u64,
    observer: &mut O,
) -> Result<ShardRecord, CampaignError> {
    let kernel = Kernel::new(cell.q, cell.n, backend, workers)?;
    evaluate_shard_with_dispatch(
        root_seed,
        purpose,
        cell,
        shard,
        backend,
        launch_cap_ms,
        observer,
        &mut |matrices| kernel.permanents(matrices),
    )
}

/// Opens the same stream and batching loop with a caller-supplied ordered kernel.
/// The dispatcher returns one field residue per input matrix. Shape or residue
/// mismatches refuse the stream. This also permits CPU checks of accelerator
/// launch boundaries without a device.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_shard_with_dispatch<const Q: u64, O, D>(
    root_seed: u64,
    purpose: StreamPurpose,
    cell: &Cell,
    shard: ShardSpec,
    backend: Backend,
    launch_cap_ms: u64,
    observer: &mut O,
    dispatch: &mut D,
) -> Result<ShardRecord, CampaignError>
where
    O: FnMut(&[Fp<Q>], u64, Option<u64>),
    D: FnMut(&[Vec<Fp<Q>>]) -> Result<Vec<u64>, CampaignError>,
{
    if Q != u64::from(cell.q) {
        return invalid("sampler field differs from cell");
    }
    let field = match Q {
        3 => FieldOrder::F3,
        5 => FieldOrder::F5,
        7 => FieldOrder::F7,
        _ => return invalid("unsupported field"),
    };
    let stream =
        StreamIndex::new(shard.stream_index).map_err(|e| CampaignError::Invalid(e.to_string()))?;
    let address = MatrixAddress::new(root_seed, field, cell.n as usize, purpose, stream);
    let mut sampler =
        MatrixSampler::<Q>::new(address).map_err(|e| CampaignError::Invalid(e.to_string()))?;
    let entries = usize::from(cell.n).pow(2);
    let batch_size = match backend {
        Backend::BatchParallel => (16384 / entries.max(1)).clamp(1, 64),
        Backend::Accelerator => match cell.accelerator_per_matrix_us {
            Some(cost) => launch_size(
                Duration::try_from_secs_f64(cost / 1e6)
                    .map_err(|e| CampaignError::Invalid(e.to_string()))?,
                Duration::from_millis(launch_cap_ms),
                shard.matrix_count,
            ),
            None if purpose == StreamPurpose::Validation => (16384 / entries.max(1)).clamp(1, 64),
            None => return invalid("missing accelerator cost"),
        },
        _ => 1,
    }
    .min(shard.matrix_count as usize)
    .max(1);
    let mut matrices = vec![vec![Fp::<Q>::new(0); entries]; batch_size];
    let mut record = ShardRecord {
        shard_id: shard.shard_id,
        stream_index: shard.stream_index,
        matrix_count: shard.matrix_count,
        permanent_zero_count: 0,
        permanent_histogram: vec![0; Q as usize],
        determinant_zero_count: cell.determinant_companion.then_some(0),
        executed_backend: backend,
    };
    let mut remaining = shard.matrix_count;
    while remaining > 0 {
        let len = remaining.min(batch_size as u64) as usize;
        for entries in &mut matrices[..len] {
            sampler.fill_next_matrix(entries);
        }
        let values = dispatch(&matrices[..len])?;
        if values.len() != len || values.iter().any(|&v| v >= Q) {
            return invalid("dispatcher returned inconsistent residues");
        }
        for (entries, permanent) in matrices[..len].iter().zip(values) {
            let determinant = cell
                .determinant_companion
                .then(|| evaluate_determinant(entries, cell.n as usize));
            observer(entries, permanent, determinant);
            record.permanent_histogram[permanent as usize] += 1;
            record.permanent_zero_count += u64::from(permanent == 0);
            if let Some(count) = &mut record.determinant_zero_count {
                *count += u64::from(determinant == Some(0));
            }
        }
        remaining -= len as u64;
    }
    Ok(record)
}
