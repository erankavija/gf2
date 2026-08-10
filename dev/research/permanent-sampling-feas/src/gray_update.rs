//! Dependency-chained Gray add/subtract micro-measurement.
//!
//! This mode isolates the update at the centre of a Ryser Gray walk.  Each
//! iteration reads one packed accumulator, adds or subtracts one sampled
//! column, and stores that same accumulator for the next iteration. Sampling,
//! accumulator construction, and CSV output are outside the timed CPU span.
//! A matched compiler-barrier control has the same iteration geometry and is
//! subtracted before the per-operation result is reported. HIP rows use the
//! same paired device-event method, excluding allocation, upload, submission,
//! and host repetition-policy overhead.
//!
//! Clean-device functional evidence may validly produce an explicit censored
//! row when paired subtraction is nonpositive; that confirms the execution
//! path without publishing a performance result.

use std::hint::black_box;
use std::time::Instant;

use gf2_algebra::packed::{Bipedal3, Packed5, Packed7, PackedField};
use gf2_core::gfp::Fp;

use crate::backend::{support, Backend, Support};
use crate::protocol::{
    run_timed_repetitions, Outcome, MAX_CELL_SECONDS, MIN_REPS, MIN_TIMED_SECONDS,
};
use crate::sampler::{MatrixSampler, MeasurementPurpose};
use crate::schedule::{scheduled_backend, SchedulePhase};

/// CSV columns emitted by the `gray-update` command.
pub const GRAY_UPDATE_CSV_HEADER: &str = "q,n,backend,outcome,steps,reps,update_s,compiler_barrier_baseline_s,net_per_operation_s,duration_basis,timed_purpose,timed_index_first,overhead_exclusion,note";

/// One dependency-chained Gray-update measurement request.
#[derive(Clone, Copy, Debug)]
pub struct GrayUpdateSpec {
    /// Field order.
    pub q: u64,
    /// Active row lanes in the accumulator.
    pub n: usize,
    /// Chained add/subtract operations in one repetition.
    pub steps: u64,
    /// Candidate selected exclusively through the canonical schedule.
    pub backend: Backend,
    /// Root of the deterministic sampler address.
    pub seed_root: u64,
    /// First stream index for both distinct mode purposes.
    pub seed_index: u64,
}

/// One output row from the dependency-chained Gray-update mode.
#[derive(Clone, Debug)]
pub struct GrayUpdateResult {
    /// Field order.
    pub q: u64,
    /// Active accumulator lanes.
    pub n: usize,
    /// Stable candidate name.
    pub backend: &'static str,
    /// Whether the row measured, is unsupported, or has no reportable net span.
    pub outcome: Outcome,
    /// Chained updates in each repetition.
    pub steps: u64,
    /// Timed repetitions completed under the canonical policy.
    pub reps: usize,
    /// Sum of isolated update-chain spans over timed repetitions.
    pub update_s: Option<f64>,
    /// Sum of matched compiler-barrier baseline spans over timed repetitions.
    pub compiler_barrier_baseline_s: Option<f64>,
    /// `(update_s - compiler_barrier_baseline_s) / (steps * reps)`, when positive.
    pub net_per_operation_s: Option<f64>,
    /// Clock domain of both paired spans.
    pub duration_basis: &'static str,
    /// Named sampler purpose of timed repetitions.
    pub timed_purpose: MeasurementPurpose,
    /// First deterministic timed stream index.
    pub timed_index_first: u64,
    /// Exact setup and loop costs removed from the reported net duration.
    pub overhead_exclusion: &'static str,
    /// Explicit unsupported, censoring, or method note.
    pub note: String,
}

impl GrayUpdateResult {
    /// Render this row without substituting an absent duration with zero.
    #[must_use]
    pub fn to_csv_row(&self) -> String {
        let update = self
            .update_s
            .map_or_else(String::new, |value| format!("{value:.9}"));
        let baseline = self
            .compiler_barrier_baseline_s
            .map_or_else(String::new, |value| format!("{value:.9}"));
        let net = self
            .net_per_operation_s
            .map_or_else(String::new, |value| format!("{value:.12}"));
        format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            self.q,
            self.n,
            self.backend,
            self.outcome.name(),
            self.steps,
            self.reps,
            update,
            baseline,
            net,
            self.duration_basis,
            self.timed_purpose.name(),
            self.timed_index_first,
            self.overhead_exclusion.replace(',', ";"),
            self.note.replace(',', ";"),
        )
    }
}

/// Run one selected candidate under the harness's canonical timing policy.
///
/// Unsupported candidates are returned as explicit rows.  This is important
/// for prototype registry entries: adding an entry to that registry adds a
/// row here through [`crate::schedule::scheduled_backends`] without giving a
/// missing evaluator a silent omission.
#[must_use]
pub fn run_gray_update(spec: GrayUpdateSpec) -> GrayUpdateResult {
    let timed_purpose = scheduled_backend(spec.backend, SchedulePhase::GrayUpdateTimed).purpose();
    let mut result = GrayUpdateResult {
        q: spec.q,
        n: spec.n,
        backend: spec.backend.name(),
        outcome: Outcome::Unsupported,
        steps: spec.steps,
        reps: 0,
        update_s: None,
        compiler_barrier_baseline_s: None,
        net_per_operation_s: None,
        duration_basis: "unavailable",
        timed_purpose,
        timed_index_first: spec.seed_index,
        overhead_exclusion: "no duration: candidate did not run",
        note: String::new(),
    };

    if spec.steps == 0 {
        result.note = "unsupported: --steps must be nonzero".to_string();
        return result;
    }
    if let Err(reason) = gray_update_support(spec) {
        result.note = reason;
        return result;
    }
    if !has_gray_update_evaluator(spec.backend) {
        result.note = format!(
            "unsupported: {} has no isolated dependency-chained Gray-update evaluator",
            spec.backend.name()
        );
        return result;
    }

    let mut spans = Vec::new();
    let policy = match run_timed_repetitions(
        spec.seed_index,
        |index| {
            let purpose =
                scheduled_backend(spec.backend, SchedulePhase::GrayUpdateWarmup).purpose();
            one_repetition(spec, purpose, index).map(|_| ())
        },
        || Ok(()),
        |index| one_repetition(spec, timed_purpose, index).map(|span| spans.push(span)),
    ) {
        Ok(policy) => policy,
        Err(GrayUpdateUnavailable(reason)) => {
            result.note = reason;
            return result;
        }
    };
    result.reps = policy.reps;
    let update_s: f64 = spans.iter().map(|span| span.update_s).sum();
    let compiler_barrier_baseline_s: f64 = spans
        .iter()
        .map(|span| span.compiler_barrier_baseline_s)
        .sum();
    result.update_s = Some(update_s);
    result.compiler_barrier_baseline_s = Some(compiler_barrier_baseline_s);
    result.duration_basis = if uses_gpu_evaluator(spec.backend) {
        "device_event_kernel"
    } else {
        "host_clock_update_chain"
    };
    result.overhead_exclusion = if uses_gpu_evaluator(spec.backend) {
        "net subtracts the same-geometry compiler-barrier kernel event span; device events also exclude allocation, transfer, submission, and host repetition-policy overhead"
    } else {
        "net subtracts the same-geometry compiler-barrier host span; sampling, packing, repetition policy, and CSV output are excluded"
    };
    if policy.capped_before_minimums {
        result.outcome = Outcome::Censored;
        result.note = format!(
            "unavailable: {MAX_CELL_SECONDS:.0} s cap ended timing before both minimums ({MIN_REPS} repetitions and {MIN_TIMED_SECONDS:.0} s); no net per-operation duration is reported"
        );
        return result;
    }
    let Some(net_per_operation_s) = net_per_operation(
        update_s,
        compiler_barrier_baseline_s,
        spec.steps,
        policy.reps,
    ) else {
        result.outcome = Outcome::Censored;
        result.note = format!(
            "unavailable: paired update span {update_s:.9} s minus compiler-barrier baseline {compiler_barrier_baseline_s:.9} s is nonpositive; no net per-operation duration is reported"
        );
        return result;
    };
    result.outcome = Outcome::Measured;
    result.net_per_operation_s = Some(net_per_operation_s);
    result.note = "net per-operation duration = (sum update spans - sum same-geometry compiler-barrier spans) / (steps * reps); canonical warm-up/repetition/censoring policy applied".to_string();
    result
}

fn has_gray_update_evaluator(backend: Backend) -> bool {
    matches!(backend, Backend::Scalar) || uses_gpu_evaluator(backend)
}

fn uses_gpu_evaluator(backend: Backend) -> bool {
    backend == Backend::Gpu || registered_f3_bipedal_evaluator(backend)
}

#[cfg(feature = "prototype-registry")]
fn registered_f3_bipedal_evaluator(backend: Backend) -> bool {
    matches!(
        backend,
        Backend::Prototype(
            permanent_wave_gpu::MeasurementPath::WaveGf3
                | permanent_wave_gpu::MeasurementPath::FoldGf3
        )
    )
}

#[cfg(not(feature = "prototype-registry"))]
fn registered_f3_bipedal_evaluator(_: Backend) -> bool {
    false
}

fn gray_update_support(spec: GrayUpdateSpec) -> Result<(), String> {
    if spec.backend == Backend::Scalar && spec.q == 7 && spec.n > 16 {
        return Err("packed F_7 Gray-update representation has 16 lanes; n exceeds 16".to_string());
    }
    if registered_f3_bipedal_evaluator(spec.backend) {
        if spec.q != 3 || spec.n > 63 {
            return Err("registered F_3 Bipedal3 path supports q = 3 and n <= 63".to_string());
        }
        if !cfg!(feature = "hip") {
            return Err(
                "registered F_3 Bipedal3 path requires the hip feature for event-timed evaluation"
                    .to_string(),
            );
        }
        return Ok(());
    }
    match support(spec.backend, spec.q, spec.n) {
        Support::Supported => Ok(()),
        Support::Unsupported(reason) => Err(reason),
    }
}

#[derive(Clone, Copy, Debug)]
struct RepetitionSpans {
    update_s: f64,
    compiler_barrier_baseline_s: f64,
}

/// An event-timed GPU evaluation that the harness cannot perform safely.
///
/// The row remains explicit rather than substituting a CPU or host-clock
/// duration for a device-event measurement.
#[derive(Debug)]
struct GrayUpdateUnavailable(String);

fn net_per_operation(
    update_s: f64,
    compiler_barrier_baseline_s: f64,
    steps: u64,
    reps: usize,
) -> Option<f64> {
    let net_s = update_s - compiler_barrier_baseline_s;
    (net_s > 0.0 && steps > 0 && reps > 0).then(|| net_s / (steps as f64 * reps as f64))
}

fn one_repetition(
    spec: GrayUpdateSpec,
    purpose: MeasurementPurpose,
    index: u64,
) -> Result<RepetitionSpans, GrayUpdateUnavailable> {
    let mut sampler = MatrixSampler::new(spec.seed_root, spec.q, spec.n, purpose, index);
    match spec.backend {
        Backend::Scalar => match spec.q {
            3 => Ok(packed_repetition::<Bipedal3, 3>(
                &mut sampler,
                spec.n,
                spec.steps,
            )),
            5 => Ok(packed_repetition::<Packed5, 5>(
                &mut sampler,
                spec.n,
                spec.steps,
            )),
            7 if spec.n <= <Packed7 as PackedField<Fp<7>>>::LANES => {
                Ok(packed_repetition::<Packed7, 7>(
                    &mut sampler,
                    spec.n,
                    spec.steps,
                ))
            }
            7 => unreachable!("support must reject Packed7 rows above its lane bound"),
            _ => unreachable!("support must reject unknown field orders"),
        },
        Backend::Gpu => gpu_repetition(&mut sampler, spec.q, spec.n, spec.steps),
        #[cfg(feature = "prototype-registry")]
        Backend::Prototype(
            permanent_wave_gpu::MeasurementPath::WaveGf3
            | permanent_wave_gpu::MeasurementPath::FoldGf3,
        ) => gpu_repetition(&mut sampler, spec.q, spec.n, spec.steps),
        _ => unreachable!("has_gray_update_evaluator filtered this backend"),
    }
}

fn packed_repetition<P, const Q: u64>(
    sampler: &mut MatrixSampler,
    n: usize,
    steps: u64,
) -> RepetitionSpans
where
    P: PackedField<Fp<Q>>,
{
    assert!(
        n <= P::LANES,
        "packed Gray update exceeds the representation lanes"
    );
    let mut column = P::zero();
    for lane in 0..n {
        column = column.with_lane(lane, sampler.next_entry::<Q>());
    }
    let update_started = Instant::now();
    let accumulator = packed_chain(column, steps);
    black_box(accumulator);
    let update_s = update_started.elapsed().as_secs_f64();
    let baseline_started = Instant::now();
    for step in 0..steps {
        for lane in 0..1 {
            if step & 1 == 0 {
                black_box((step, lane, 0_u8));
            } else {
                black_box((step, lane, 1_u8));
            }
        }
    }
    RepetitionSpans {
        update_s,
        compiler_barrier_baseline_s: baseline_started.elapsed().as_secs_f64(),
    }
}

fn packed_chain<P, const Q: u64>(column: P, steps: u64) -> P
where
    P: PackedField<Fp<Q>>,
{
    let mut accumulator = P::zero();
    for step in 0..steps {
        accumulator = if step & 1 == 0 {
            accumulator.add(column)
        } else {
            accumulator.sub(column)
        };
    }
    accumulator
}

#[cfg(feature = "hip")]
fn gpu_repetition(
    sampler: &mut MatrixSampler,
    q: u64,
    n: usize,
    steps: u64,
) -> Result<RepetitionSpans, GrayUpdateUnavailable> {
    use gf2_kernels_hip::permanent::{
        measure_gray_update_kernel, GrayUpdateChecksum, GrayUpdateOperand, PermanentField,
    };

    let mut byte_column = Vec::with_capacity(n);
    let operand = if q == 3 {
        let mut mag = 0_u64;
        let mut sgn = 0_u64;
        for lane in 0..n {
            match sampler.next_entry::<3>().value() {
                0 => {}
                1 => mag |= 1_u64 << lane,
                2 => {
                    mag |= 1_u64 << lane;
                    sgn |= 1_u64 << lane;
                }
                _ => unreachable!("F_3 sampler returns canonical values"),
            }
        }
        GrayUpdateOperand::Bipedal3 { mag, sgn, n }
    } else {
        for _ in 0..n {
            byte_column.push(match q {
                5 => sampler.next_entry::<5>().value() as u8,
                7 => sampler.next_entry::<7>().value() as u8,
                _ => unreachable!("support must reject unknown field orders"),
            });
        }
        GrayUpdateOperand::Bytes(&byte_column)
    };
    let expected_checksum = match &operand {
        GrayUpdateOperand::Bipedal3 { mag, sgn, .. } => {
            let (mag, sgn) = bipedal3_raw_chain(*mag, *sgn, steps);
            GrayUpdateChecksum::Bipedal3 { mag, sgn }
        }
        GrayUpdateOperand::Bytes(column) => GrayUpdateChecksum::Bytes(if steps & 1 == 0 {
            0
        } else {
            column.iter().map(|&value| u64::from(value)).sum()
        }),
    };
    let field = match q {
        3 => PermanentField::F3,
        5 => PermanentField::F5,
        7 => PermanentField::F7,
        _ => unreachable!("support must reject unknown field orders"),
    };
    let timings = match measure_gray_update_kernel(field, operand, steps) {
        Ok(timings) => timings,
        Err(error) => match gray_update_unavailable_reason(&error) {
            Some(reason) => return Err(GrayUpdateUnavailable(reason)),
            None => panic!("fatal event-timed Gray-update kernel failure: {error}"),
        },
    };
    assert_eq!(
        timings.update_checksum, expected_checksum,
        "the device Gray-update chain must produce its sampled odd/even final state"
    );
    Ok(RepetitionSpans {
        update_s: timings.update.as_secs_f64(),
        compiler_barrier_baseline_s: timings.compiler_barrier_baseline.as_secs_f64(),
    })
}

/// Classify failures which leave this row without a valid device-event span.
///
/// This is an output policy, not a CPU fallback: a missing device, resource,
/// or timing boundary stays an explicit unsupported row. Kernel launch,
/// synchronization, transfer, blob-load, and checksum failures are correctness
/// failures and therefore remain fatal to the command.
#[cfg(feature = "hip")]
fn gray_update_unavailable_reason(error: &gf2_kernels_hip::HipError) -> Option<String> {
    use gf2_kernels_hip::HipError;

    match error {
        HipError::OutOfMemory { .. }
        | HipError::Hip { code: 2, .. } => Some(format!(
            "unavailable: GPU resource failure prevents event-timed Gray-update evaluation: {error}"
        )),
        HipError::NoDevice | HipError::UnsupportedArch { .. }
        | HipError::Hip {
            code: 100 | 101,
            context: "hipGetDevice",
        } => Some(format!(
            "unsupported: device unavailable for this event-timed GPU cell; no CPU fallback or host timing was substituted: {error}"
        )),
        HipError::Hip { context, .. } if context.starts_with("hipEvent") => Some(format!(
            "unavailable: GPU event instrumentation failed; no host timing was substituted: {error}"
        )),
        HipError::Hip { .. } | HipError::BlobLoad { .. } => None,
    }
}

#[cfg(feature = "hip")]
fn bipedal3_raw_chain(column_mag: u64, column_sgn: u64, steps: u64) -> (u64, u64) {
    let (mut mag, mut sgn) = (0_u64, 0_u64);
    for step in 0..steps {
        if step & 1 == 0 {
            let transition = mag ^ sgn ^ column_sgn;
            let carry = column_mag & transition;
            mag = carry | (mag ^ column_mag);
            sgn ^= carry;
        } else {
            let transition = sgn ^ column_sgn;
            let carry = mag & transition;
            mag = carry | (mag ^ column_mag);
            sgn = carry ^ (column_mag ^ column_sgn);
        }
    }
    (mag, sgn)
}

#[cfg(not(feature = "hip"))]
fn gpu_repetition(
    _: &mut MatrixSampler,
    _: u64,
    _: usize,
    _: u64,
) -> Result<RepetitionSpans, GrayUpdateUnavailable> {
    unreachable!("support rejects GPU when the hip feature is disabled")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schedule::{scheduled_backends, SchedulePhase};

    #[cfg(feature = "prototype-registry")]
    #[test]
    fn gray_update_schedule_reaches_every_registered_candidate() {
        let scheduled: Vec<_> = scheduled_backends(SchedulePhase::GrayUpdateTimed)
            .map(|scheduled| scheduled.backend().name())
            .collect();
        let canonical: Vec<_> = Backend::ALL.into_iter().map(Backend::name).collect();
        assert_eq!(scheduled, canonical);
    }

    #[cfg(all(feature = "prototype-registry", not(feature = "hip")))]
    #[test]
    fn unsupported_prototype_remains_a_named_output_row() {
        let backend = scheduled_backends(SchedulePhase::GrayUpdateTimed)
            .find(|scheduled| scheduled.backend().prototype_path().is_some())
            .expect("prototype registry supplies a candidate")
            .backend();
        let row = run_gray_update(GrayUpdateSpec {
            q: 3,
            n: 12,
            steps: 1,
            backend,
            seed_root: 7,
            seed_index: 0,
        });
        assert_eq!(row.outcome, Outcome::Unsupported);
        assert!(row.note.contains("requires the hip feature"));
        assert!(row.net_per_operation_s.is_none());
    }

    #[test]
    fn csv_states_the_duration_basis_and_overhead_contract() {
        let row = GrayUpdateResult {
            q: 3,
            n: 12,
            backend: "gpu_hip",
            outcome: Outcome::Measured,
            steps: 8,
            reps: 5,
            update_s: Some(0.25),
            compiler_barrier_baseline_s: Some(0.05),
            net_per_operation_s: Some(0.025),
            duration_basis: "device_event_kernel",
            timed_purpose: MeasurementPurpose::GrayUpdateTimed,
            timed_index_first: 0,
            overhead_exclusion: "device events exclude host-loop overhead",
            note: String::new(),
        };
        let csv = row.to_csv_row();
        assert!(csv.contains("device_event_kernel"));
        assert!(csv.contains("exclude host-loop overhead"));
        assert!(csv.contains("gray_update_timed"));
        assert!(csv.contains("0.025000000000"));
        assert_eq!(
            csv.split(',').count(),
            GRAY_UPDATE_CSV_HEADER.split(',').count(),
            "CSV row width must remain aligned with its header"
        );
    }

    #[test]
    fn packed_chain_reads_the_accumulator_written_by_each_predecessor() {
        let column = <Bipedal3 as PackedField<Fp<3>>>::zero()
            .with_lane(0, Fp::<3>::new(1))
            .with_lane(1, Fp::<3>::new(2));
        let result = packed_chain::<Bipedal3, 3>(column, 3);
        assert_eq!(result.lane(0), Fp::<3>::new(1));
        assert_eq!(result.lane(1), Fp::<3>::new(2));
    }

    #[test]
    fn nonpositive_baseline_subtraction_has_no_reportable_net_duration() {
        assert_eq!(net_per_operation(0.2, 0.2, 8, 5), None);
        assert_eq!(net_per_operation(0.19, 0.2, 8, 5), None);
        assert_eq!(net_per_operation(0.25, 0.05, 8, 5), Some(0.005));
    }

    #[test]
    fn scalar_f7_rows_above_the_packed_lane_limit_are_explicitly_unsupported() {
        let reason = gray_update_support(GrayUpdateSpec {
            q: 7,
            n: 17,
            steps: 1,
            backend: Backend::Scalar,
            seed_root: 7,
            seed_index: 0,
        })
        .expect_err("F_7 packed micro-update has only sixteen lanes");
        assert!(reason.contains("16 lanes"));
    }

    #[cfg(feature = "hip")]
    #[test]
    fn gpu_resource_capability_and_event_failures_remain_explicit_rows() {
        use gf2_kernels_hip::HipError;

        let out_of_memory = gray_update_unavailable_reason(&HipError::OutOfMemory {
            device_id: 0,
            bytes_requested: 4096,
        })
        .expect("typed out-of-memory is unavailable, not a panic");
        assert!(out_of_memory.contains("resource failure"));

        let raw_out_of_memory = gray_update_unavailable_reason(&HipError::Hip {
            code: 2,
            context: "hipMalloc",
        })
        .expect("raw HIP out-of-memory is unavailable, not a panic");
        assert!(raw_out_of_memory.contains("resource failure"));

        let no_device = gray_update_unavailable_reason(&HipError::NoDevice)
            .expect("a missing device is an explicit unsupported cell");
        assert!(no_device.contains("device unavailable"));
        assert!(no_device.contains("no CPU fallback or host timing was substituted"));

        let unsupported_arch = gray_update_unavailable_reason(&HipError::UnsupportedArch {
            gcn_arch_name: "gfx999".to_string(),
        })
        .expect("unsupported architecture is an explicit unsupported cell");
        assert!(unsupported_arch.contains("device unavailable"));

        let event_failure = gray_update_unavailable_reason(&HipError::Hip {
            code: 700,
            context: "hipEventElapsedTime",
        })
        .expect("event instrumentation failure is unavailable, not a panic");
        assert!(event_failure.contains("event instrumentation failed"));
        assert!(event_failure.contains("no host timing was substituted"));

        assert!(gray_update_unavailable_reason(&HipError::Hip {
            code: 700,
            context: "launch_gray_update_micro",
        })
        .is_none());
        assert!(gray_update_unavailable_reason(&HipError::Hip {
            code: 700,
            context: "hipStreamSynchronize",
        })
        .is_none());
        assert!(gray_update_unavailable_reason(&HipError::BlobLoad {
            path: std::path::PathBuf::from("missing.co"),
            source: "missing".to_string(),
        })
        .is_none());
    }

    #[cfg(feature = "prototype-registry")]
    #[test]
    fn landed_registered_f3_paths_are_event_evaluable_only_with_hip() {
        for path in [
            permanent_wave_gpu::MeasurementPath::WaveGf3,
            permanent_wave_gpu::MeasurementPath::FoldGf3,
        ] {
            let spec = GrayUpdateSpec {
                q: 3,
                n: 12,
                steps: 1,
                backend: Backend::Prototype(path),
                seed_root: 7,
                seed_index: 0,
            };
            if cfg!(feature = "hip") {
                assert!(gray_update_support(spec).is_ok());
                assert!(uses_gpu_evaluator(spec.backend));
            } else {
                let reason =
                    gray_update_support(spec).expect_err("HIP is required for event timing");
                assert!(reason.contains("requires the hip feature"));
            }
        }
    }
}
