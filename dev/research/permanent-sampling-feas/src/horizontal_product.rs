//! Branch-aware horizontal-product micro-measurement.
//!
//! The permanent's row-product has two semantically distinct outcomes: a row
//! sum of zero makes the product zero, while an all-nonzero row-sum vector
//! runs the representation's complete reduction. This module retains their
//! unconditioned occurrence frequencies separately from their conditioned
//! device-event timing. Sampling and grouping are outside every reported
//! device span, so grouping a branch for timing cannot change its frequency.

use crate::backend::Backend;
use crate::protocol::Outcome;
#[cfg(feature = "hip")]
use crate::protocol::{capped_before_minimums_note, run_timed_repetitions};
#[cfg(feature = "hip")]
use crate::sampler::MatrixSampler;
use crate::sampler::MeasurementPurpose;
use crate::schedule::{scheduled_backend, SchedulePhase};

/// CSV columns emitted by the `horizontal-product` command.
pub const HORIZONTAL_PRODUCT_CSV_HEADER: &str = "q,n,backend,outcome,samples_per_rep,reps,timed_samples_total,zero_fast_observed_numerator,zero_fast_observed_denominator,nonzero_slow_observed_numerator,nonzero_slow_observed_denominator,zero_fast_observed_frequency,zero_fast_expected_frequency,nonzero_slow_observed_frequency,nonzero_slow_expected_frequency,zero_fast_s,zero_fast_compiler_barrier_baseline_s,zero_fast_net_per_operation_s,zero_fast_timed_operations,nonzero_slow_s,nonzero_slow_compiler_barrier_baseline_s,nonzero_slow_net_per_operation_s,nonzero_slow_timed_operations,duration_basis,timed_purpose,timed_index_first,overhead_exclusion,note";

/// One horizontal-product micro-measurement request.
#[derive(Clone, Copy, Debug)]
pub struct HorizontalProductSpec {
    /// Field order of the sampled row sums.
    pub q: u64,
    /// Number of active row lanes in every sample.
    pub n: usize,
    /// Independent, unconditioned row-sum vectors per canonical repetition.
    pub samples: usize,
    /// Candidate selected exclusively through the canonical schedule.
    pub backend: Backend,
    /// Root of the deterministic sampler address.
    pub seed_root: u64,
    /// First stream index for both distinct mode purposes.
    pub seed_index: u64,
}

/// One output row from the horizontal-product micro-measurement mode.
#[derive(Clone, Debug)]
pub struct HorizontalProductResult {
    /// Field order.
    pub q: u64,
    /// Active row lanes.
    pub n: usize,
    /// Stable candidate name.
    pub backend: &'static str,
    /// Whether the row measured, is unsupported, or is censored.
    pub outcome: Outcome,
    /// Independent vectors requested in each repetition.
    pub samples_per_rep: usize,
    /// Timed repetitions completed under the canonical policy.
    pub reps: usize,
    /// All independent, unconditioned samples from timed repetitions only.
    pub timed_samples_total: u64,
    /// Timed samples whose product is zero.
    pub zero_fast_samples: u64,
    /// Timed samples whose product is nonzero.
    pub nonzero_slow_samples: u64,
    /// Observed zero-product frequency from timed samples only.
    pub zero_fast_observed_frequency: Option<f64>,
    /// Exact marginal zero-product expectation, projected to a stable `f64`
    /// when this mode supports the field order.
    pub zero_fast_expected_frequency: Option<f64>,
    /// Observed nonzero-product frequency from timed samples only.
    pub nonzero_slow_observed_frequency: Option<f64>,
    /// Exact marginal nonzero-product expectation, projected to a stable `f64`
    /// when this mode supports the field order.
    pub nonzero_slow_expected_frequency: Option<f64>,
    /// Sum of fast-branch device-event spans.
    pub zero_fast_s: Option<f64>,
    /// Sum of same-geometry fast-branch compiler-barrier spans.
    pub zero_fast_compiler_barrier_baseline_s: Option<f64>,
    /// Positive fast-branch net time divided by actual fast timed operations.
    pub zero_fast_net_per_operation_s: Option<f64>,
    /// Exact fast-branch operation denominator for its reported rate.
    pub zero_fast_timed_operations: u64,
    /// Sum of slow-branch device-event spans.
    pub nonzero_slow_s: Option<f64>,
    /// Sum of same-geometry slow-branch compiler-barrier spans.
    pub nonzero_slow_compiler_barrier_baseline_s: Option<f64>,
    /// Positive slow-branch net time divided by actual slow timed operations.
    pub nonzero_slow_net_per_operation_s: Option<f64>,
    /// Exact slow-branch operation denominator for its reported rate.
    pub nonzero_slow_timed_operations: u64,
    /// Clock domain of every reported duration.
    pub duration_basis: &'static str,
    /// Named sampler purpose of timed repetitions.
    pub timed_purpose: MeasurementPurpose,
    /// First deterministic timed stream index.
    pub timed_index_first: u64,
    /// Exact costs absent from every reported net duration.
    pub overhead_exclusion: &'static str,
    /// Explicit unsupported, censoring, or method note.
    pub note: String,
}

impl HorizontalProductResult {
    /// Render this row without encoding an unavailable span or rate as zero.
    #[must_use]
    pub fn to_csv_row(&self) -> String {
        [
            self.q.to_string(),
            self.n.to_string(),
            self.backend.to_string(),
            self.outcome.name().to_string(),
            self.samples_per_rep.to_string(),
            self.reps.to_string(),
            self.timed_samples_total.to_string(),
            self.zero_fast_samples.to_string(),
            self.timed_samples_total.to_string(),
            self.nonzero_slow_samples.to_string(),
            self.timed_samples_total.to_string(),
            optional_frequency(self.zero_fast_observed_frequency),
            optional_frequency(self.zero_fast_expected_frequency),
            optional_frequency(self.nonzero_slow_observed_frequency),
            optional_frequency(self.nonzero_slow_expected_frequency),
            optional_seconds(self.zero_fast_s),
            optional_seconds(self.zero_fast_compiler_barrier_baseline_s),
            optional_rate(self.zero_fast_net_per_operation_s),
            self.zero_fast_timed_operations.to_string(),
            optional_seconds(self.nonzero_slow_s),
            optional_seconds(self.nonzero_slow_compiler_barrier_baseline_s),
            optional_rate(self.nonzero_slow_net_per_operation_s),
            self.nonzero_slow_timed_operations.to_string(),
            self.duration_basis.to_string(),
            self.timed_purpose.name().to_string(),
            self.timed_index_first.to_string(),
            self.overhead_exclusion.replace(',', ";"),
            self.note.replace(',', ";"),
        ]
        .join(",")
    }
}

fn optional_seconds(value: Option<f64>) -> String {
    value.map_or_else(String::new, |seconds| format!("{seconds:.9}"))
}

fn optional_rate(value: Option<f64>) -> String {
    value.map_or_else(String::new, |seconds| format!("{seconds:.12}"))
}

fn optional_frequency(value: Option<f64>) -> String {
    value.map_or_else(String::new, |frequency| format!("{frequency:.12}"))
}

/// Run one selected candidate under the harness's canonical timing policy.
///
/// The default schedule intentionally includes candidates that cannot expose a
/// separated zero/nonzero product branch. They produce a named unavailable row
/// rather than a host-clock substitute, a synthetic branch, or a silently
/// omitted registry entry.
#[must_use]
pub fn run_horizontal_product(spec: HorizontalProductSpec) -> HorizontalProductResult {
    let timed_purpose =
        scheduled_backend(spec.backend, SchedulePhase::HorizontalProductTimed).purpose();
    let mut result = HorizontalProductResult {
        q: spec.q,
        n: spec.n,
        backend: spec.backend.name(),
        outcome: Outcome::Unsupported,
        samples_per_rep: spec.samples,
        reps: 0,
        timed_samples_total: 0,
        zero_fast_samples: 0,
        nonzero_slow_samples: 0,
        zero_fast_observed_frequency: None,
        zero_fast_expected_frequency: None,
        nonzero_slow_observed_frequency: None,
        nonzero_slow_expected_frequency: None,
        zero_fast_s: None,
        zero_fast_compiler_barrier_baseline_s: None,
        zero_fast_net_per_operation_s: None,
        zero_fast_timed_operations: 0,
        nonzero_slow_s: None,
        nonzero_slow_compiler_barrier_baseline_s: None,
        nonzero_slow_net_per_operation_s: None,
        nonzero_slow_timed_operations: 0,
        duration_basis: "unavailable",
        timed_purpose,
        timed_index_first: spec.seed_index,
        overhead_exclusion: "no duration: candidate did not run",
        note: String::new(),
    };

    if spec.samples == 0 {
        result.note = "unsupported: --samples must be nonzero".to_string();
        return result;
    }
    if !(1..=63).contains(&spec.n) {
        result.note = "unsupported: horizontal-product active lanes must lie in 1..=63".to_string();
        return result;
    }
    if !matches!(spec.q, 3 | 5 | 7) {
        result.note = format!(
            "unsupported: no horizontal-product circuit for q = {}",
            spec.q
        );
        return result;
    }
    let (zero_expected, nonzero_expected) = expected_frequencies(spec.q, spec.n);
    result.zero_fast_expected_frequency = Some(zero_expected);
    result.nonzero_slow_expected_frequency = Some(nonzero_expected);

    #[cfg(not(feature = "hip"))]
    {
        result.note = "unsupported: horizontal-product branch timing requires the hip feature for device-event measurement; no host-clock substitute was used".to_string();
        result
    }

    #[cfg(feature = "hip")]
    {
        let circuit = match circuit_for(spec.backend, spec.q) {
            Ok(circuit) => circuit,
            Err(reason) => {
                result.note = reason;
                return result;
            }
        };
        if !circuit.has_observable_branches() {
            result.note = format!(
                "unavailable: {} uses the {} reduction, whose zero result is observed only after its complete reduction; emitting separate branch timings would invent a different circuit",
                spec.backend.name(),
                circuit.name(),
            );
            return result;
        }

        let mut repetitions = Vec::new();
        let policy = match run_timed_repetitions(
            spec.seed_index,
            |index| {
                one_repetition(
                    spec,
                    circuit,
                    scheduled_backend(spec.backend, SchedulePhase::HorizontalProductWarmup)
                        .purpose(),
                    index,
                )
                .map(|_| ())
            },
            || Ok(()),
            |index| {
                one_repetition(spec, circuit, timed_purpose, index)
                    .map(|span| repetitions.push(span))
            },
        ) {
            Ok(policy) => policy,
            Err(HorizontalProductUnavailable(reason)) => {
                result.note = reason;
                return result;
            }
        };
        result.reps = policy.reps;
        let totals = RepetitionTotals::from_repetitions(&repetitions);
        result.timed_samples_total = totals.timed_samples_total;
        result.zero_fast_samples = totals.zero_fast_samples;
        result.nonzero_slow_samples = totals.nonzero_slow_samples;
        result.zero_fast_observed_frequency =
            frequency(totals.zero_fast_samples, totals.timed_samples_total);
        result.nonzero_slow_observed_frequency =
            frequency(totals.nonzero_slow_samples, totals.timed_samples_total);
        result.zero_fast_s = totals.zero_fast.raw_s;
        result.zero_fast_compiler_barrier_baseline_s = totals.zero_fast.baseline_s;
        result.zero_fast_timed_operations = totals.zero_fast.operations;
        result.nonzero_slow_s = totals.nonzero_slow.raw_s;
        result.nonzero_slow_compiler_barrier_baseline_s = totals.nonzero_slow.baseline_s;
        result.nonzero_slow_timed_operations = totals.nonzero_slow.operations;
        result.duration_basis = "device_event_kernel";
        result.overhead_exclusion = "net subtracts the same-geometry compiler-barrier device-event span for each observed branch; sampler construction, branch grouping, allocation, upload, download, submission, and host repetition-policy overhead are excluded";

        if policy.capped_before_minimums {
            result.outcome = Outcome::Censored;
            result.note = capped_before_minimums_note(
                "no branch net duration is reported; raw paired device-event spans remain diagnostic",
            );
            return result;
        }

        result.zero_fast_net_per_operation_s = net_per_operation(
            totals.zero_fast.raw_s,
            totals.zero_fast.baseline_s,
            totals.zero_fast.operations,
        );
        result.nonzero_slow_net_per_operation_s = net_per_operation(
            totals.nonzero_slow.raw_s,
            totals.nonzero_slow.baseline_s,
            totals.nonzero_slow.operations,
        );
        result.outcome = if result.zero_fast_net_per_operation_s.is_some()
            || result.nonzero_slow_net_per_operation_s.is_some()
        {
            Outcome::Measured
        } else {
            Outcome::Censored
        };
        result.note = measurement_note(&result);
        result
    }
}

/// Return the exact marginal expectations, projected to stable floating-point
/// values. The mathematical contract remains $1 - ((q-1)/q)^n$ and
/// $((q-1)/q)^n$; `exp_m1` avoids cancellation in the zero-path projection.
///
/// # Panics
///
/// Panics if `q <= 1`, because those values do not define a field order for
/// this expectation.
#[must_use]
pub fn expected_frequencies(q: u64, n: usize) -> (f64, f64) {
    assert!(q > 1, "horizontal-product expectation requires q > 1");
    let log_nonzero = n as f64 * (-1.0 / q as f64).ln_1p();
    (-log_nonzero.exp_m1(), log_nonzero.exp())
}

#[cfg(feature = "hip")]
fn frequency(numerator: u64, denominator: u64) -> Option<f64> {
    (denominator > 0).then(|| numerator as f64 / denominator as f64)
}

#[cfg(any(feature = "hip", test))]
fn net_per_operation(raw_s: Option<f64>, baseline_s: Option<f64>, operations: u64) -> Option<f64> {
    let net_s = raw_s? - baseline_s?;
    (net_s > 0.0 && operations > 0).then(|| net_s / operations as f64)
}

#[cfg(feature = "hip")]
fn measurement_note(result: &HorizontalProductResult) -> String {
    let mut notes = vec![
        "observed frequencies use all and only unconditioned timed samples; warm-up samples are excluded".to_string(),
        "expected frequencies are complements: zero fast = 1 - ((q-1)/q)^n and nonzero slow = ((q-1)/q)^n".to_string(),
    ];
    if result.zero_fast_timed_operations == 0 {
        notes.push("zero fast timing unavailable: no zero-product timed sample occurred, so its rate is not encoded as zero".to_string());
    } else if result.zero_fast_net_per_operation_s.is_none() {
        notes.push("zero fast timing unavailable: raw device span minus its same-geometry baseline was nonpositive, so no false positive rate is reported".to_string());
    }
    if result.nonzero_slow_timed_operations == 0 {
        notes.push("nonzero slow timing unavailable: no nonzero-product timed sample occurred, so its rate is not encoded as zero".to_string());
    } else if result.nonzero_slow_net_per_operation_s.is_none() {
        notes.push("nonzero slow timing unavailable: raw device span minus its same-geometry baseline was nonpositive, so no false positive rate is reported".to_string());
    }
    notes.join("; ")
}

#[cfg(feature = "hip")]
#[derive(Clone, Copy, Debug, Default)]
struct BranchTotals {
    raw_s: Option<f64>,
    baseline_s: Option<f64>,
    operations: u64,
}

#[cfg(feature = "hip")]
impl BranchTotals {
    fn add(&mut self, raw_s: f64, baseline_s: f64, operations: u64) {
        self.raw_s = Some(self.raw_s.unwrap_or(0.0) + raw_s);
        self.baseline_s = Some(self.baseline_s.unwrap_or(0.0) + baseline_s);
        self.operations += operations;
    }
}

#[cfg(feature = "hip")]
#[derive(Clone, Copy, Debug, Default)]
struct RepetitionTotals {
    timed_samples_total: u64,
    zero_fast_samples: u64,
    nonzero_slow_samples: u64,
    zero_fast: BranchTotals,
    nonzero_slow: BranchTotals,
}

#[cfg(feature = "hip")]
impl RepetitionTotals {
    fn from_repetitions(repetitions: &[RepetitionSpans]) -> Self {
        let mut totals = Self::default();
        for repetition in repetitions {
            totals.timed_samples_total += repetition.timed_samples_total;
            totals.zero_fast_samples += repetition.zero_fast_samples;
            totals.nonzero_slow_samples += repetition.nonzero_slow_samples;
            if let Some(span) = repetition.zero_fast {
                totals
                    .zero_fast
                    .add(span.raw_s, span.baseline_s, span.operations);
            }
            if let Some(span) = repetition.nonzero_slow {
                totals
                    .nonzero_slow
                    .add(span.raw_s, span.baseline_s, span.operations);
            }
        }
        totals
    }
}

#[cfg(feature = "hip")]
#[derive(Clone, Copy, Debug)]
struct BranchSpans {
    raw_s: f64,
    baseline_s: f64,
    operations: u64,
}

#[cfg(feature = "hip")]
#[derive(Clone, Copy, Debug)]
struct RepetitionSpans {
    timed_samples_total: u64,
    zero_fast_samples: u64,
    nonzero_slow_samples: u64,
    zero_fast: Option<BranchSpans>,
    nonzero_slow: Option<BranchSpans>,
}

/// A missing device-event boundary that must remain an explicit row.
#[cfg(feature = "hip")]
#[derive(Debug)]
struct HorizontalProductUnavailable(String);

#[cfg(feature = "hip")]
fn circuit_for(
    backend: Backend,
    q: u64,
) -> Result<gf2_kernels_hip::permanent::HorizontalProductCircuit, String> {
    use gf2_kernels_hip::permanent::HorizontalProductCircuit;

    let circuit = match backend {
        Backend::Gpu => match q {
            3 => HorizontalProductCircuit::Bipedal3Halving,
            5 => HorizontalProductCircuit::F5Byte,
            7 => HorizontalProductCircuit::F7Lookup,
            _ => unreachable!("field order was checked by the caller"),
        },
        #[cfg(feature = "prototype-registry")]
        Backend::Prototype(path) => match path {
            permanent_wave_gpu::MeasurementPath::WaveGf3 => {
                HorizontalProductCircuit::Bipedal3Halving
            }
            permanent_wave_gpu::MeasurementPath::FoldGf3 => {
                HorizontalProductCircuit::Bipedal3ZeroMaskSignPopcount
            }
            permanent_wave_gpu::MeasurementPath::F5ByteControl => HorizontalProductCircuit::F5Byte,
            permanent_wave_gpu::MeasurementPath::F5ThreePlane => {
                HorizontalProductCircuit::F5ThreePlane
            }
            permanent_wave_gpu::MeasurementPath::F7ThreePlaneAccumulator
            | permanent_wave_gpu::MeasurementPath::F7ThreePlanePermanent => {
                HorizontalProductCircuit::F7ThreePlane
            }
            permanent_wave_gpu::MeasurementPath::F7LookupTableControl => {
                HorizontalProductCircuit::F7Lookup
            }
        },
        _ => {
            return Err(format!(
                "unavailable: {} has no distinct device-event horizontal-product isolate; no generic or host-clock replacement was used",
                backend.name(),
            ));
        }
    };
    if circuit.field_order() != q {
        return Err(format!(
            "unsupported: {} uses the {} horizontal-product circuit over F_{}, not F_{}",
            backend.name(),
            circuit.name(),
            circuit.field_order(),
            q,
        ));
    }
    Ok(circuit)
}

#[cfg(feature = "hip")]
fn one_repetition(
    spec: HorizontalProductSpec,
    circuit: gf2_kernels_hip::permanent::HorizontalProductCircuit,
    purpose: MeasurementPurpose,
    index: u64,
) -> Result<RepetitionSpans, HorizontalProductUnavailable> {
    use gf2_kernels_hip::permanent::{measure_horizontal_product_kernel, HorizontalProductBranch};

    if circuit == gf2_kernels_hip::permanent::HorizontalProductCircuit::F7Lookup {
        gf2_algebra::gpu::initialise_permanent_gf7_luts().map_err(|code| {
            HorizontalProductUnavailable(format!(
                "unavailable: initialise the established F_7 lookup table for event-timed horizontal-product evaluation returned HIP error {code}; no host timing was substituted"
            ))
        })?;
    }
    let mut sampler = MatrixSampler::new(spec.seed_root, spec.q, spec.n, purpose, index);
    let values = sample_row_sums(&mut sampler, spec.q, spec.n, spec.samples);
    let mut fast_values = Vec::new();
    let mut slow_values = Vec::new();
    let mut fast_expected = Vec::new();
    let mut slow_expected = Vec::new();
    for sample in values.chunks_exact(spec.n) {
        let expected = scalar_product(sample, spec.q);
        if expected == 0 {
            fast_values.extend_from_slice(sample);
            fast_expected.push(expected);
        } else {
            slow_values.extend_from_slice(sample);
            slow_expected.push(expected);
        }
    }

    let zero_fast = if fast_expected.is_empty() {
        None
    } else {
        let timing = measure_horizontal_product_kernel(
            circuit,
            &fast_values,
            spec.n,
            HorizontalProductBranch::ZeroFast,
        )
        .map_err(|error| HorizontalProductUnavailable(horizontal_unavailable_reason(&error)))?;
        assert_eq!(
            timing.values, fast_expected,
            "device zero fast path must return the product of every sampled row-sum vector"
        );
        Some(BranchSpans {
            raw_s: timing.product.as_secs_f64(),
            baseline_s: timing.compiler_barrier_baseline.as_secs_f64(),
            operations: fast_expected.len() as u64,
        })
    };
    let nonzero_slow = if slow_expected.is_empty() {
        None
    } else {
        let timing = measure_horizontal_product_kernel(
            circuit,
            &slow_values,
            spec.n,
            HorizontalProductBranch::NonzeroSlow,
        )
        .map_err(|error| HorizontalProductUnavailable(horizontal_unavailable_reason(&error)))?;
        assert_eq!(
            timing.values, slow_expected,
            "device nonzero slow path must return the product of every sampled row-sum vector"
        );
        Some(BranchSpans {
            raw_s: timing.product.as_secs_f64(),
            baseline_s: timing.compiler_barrier_baseline.as_secs_f64(),
            operations: slow_expected.len() as u64,
        })
    };
    Ok(RepetitionSpans {
        timed_samples_total: spec.samples as u64,
        zero_fast_samples: fast_expected.len() as u64,
        nonzero_slow_samples: slow_expected.len() as u64,
        zero_fast,
        nonzero_slow,
    })
}

#[cfg(feature = "hip")]
fn horizontal_unavailable_reason(error: &gf2_kernels_hip::HipError) -> String {
    use gf2_kernels_hip::HipError;

    match error {
        HipError::OutOfMemory { .. } | HipError::Hip { code: 2, .. } => format!(
            "unavailable: GPU resource failure prevents event-timed horizontal-product evaluation: {error}"
        ),
        HipError::NoDevice
        | HipError::UnsupportedArch { .. }
        | HipError::Hip {
            code: 100 | 101, ..
        } => format!(
            "unsupported: device unavailable for this event-timed horizontal-product cell; no CPU fallback or host timing was substituted: {error}"
        ),
        HipError::Hip { context, .. } if context.starts_with("hipEvent") => format!(
            "unavailable: GPU event instrumentation failed; no host timing was substituted: {error}"
        ),
        HipError::Hip { .. } | HipError::BlobLoad { .. } => {
            panic!("fatal event-timed horizontal-product kernel failure: {error}")
        }
    }
}

#[cfg(feature = "hip")]
fn sample_row_sums(sampler: &mut MatrixSampler, q: u64, n: usize, samples: usize) -> Vec<u8> {
    let mut values = Vec::with_capacity(n * samples);
    for _ in 0..samples {
        for _ in 0..n {
            values.push(match q {
                3 => sampler.next_entry::<3>().value() as u8,
                5 => sampler.next_entry::<5>().value() as u8,
                7 => sampler.next_entry::<7>().value() as u8,
                _ => unreachable!("field order was checked by the caller"),
            });
        }
    }
    values
}

#[cfg(feature = "hip")]
fn scalar_product(sample: &[u8], q: u64) -> u64 {
    sample
        .iter()
        .fold(1_u64, |product, &value| product * u64::from(value) % q)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_marginal_projection_keeps_the_two_paths_complementary() {
        for (q, n) in [(3, 12), (3, 63), (5, 63), (7, 63)] {
            let (fast, slow) = expected_frequencies(q, n);
            let structural_slow = ((q - 1) as f64 / q as f64).powi(n as i32);
            assert!(
                (fast - (1.0 - structural_slow)).abs() < 1e-15,
                "zero expectation must project 1 - ((q - 1) / q)^n for q = {q}, n = {n}"
            );
            assert!(
                (slow - structural_slow).abs() < 1e-15,
                "nonzero expectation must project ((q - 1) / q)^n for q = {q}, n = {n}"
            );
            assert!((fast + slow - 1.0).abs() < 1e-15);
        }
    }

    fn assert_unsupported_field_row(q: u64) {
        let row = run_horizontal_product(HorizontalProductSpec {
            q,
            n: 12,
            samples: 1,
            backend: Backend::Gpu,
            seed_root: 7,
            seed_index: 0,
        });
        assert_eq!(row.outcome, Outcome::Unsupported);
        assert_eq!(row.reps, 0);
        assert_eq!(row.timed_samples_total, 0);
        assert!(row.zero_fast_expected_frequency.is_none());
        assert!(row.nonzero_slow_expected_frequency.is_none());
        assert_eq!(row.duration_basis, "unavailable");
        assert!(row.note.contains(&format!("q = {q}")));

        let csv = row.to_csv_row();
        let columns = csv.split(',').collect::<Vec<_>>();
        assert_eq!(
            columns.len(),
            HORIZONTAL_PRODUCT_CSV_HEADER.split(',').count()
        );
        assert!(
            columns[12].is_empty(),
            "unsupported field has no zero expectation"
        );
        assert!(
            columns[14].is_empty(),
            "unsupported field has no nonzero expectation"
        );
    }

    #[test]
    fn zero_field_is_an_explicit_unsupported_row_without_expectation_projection() {
        assert_unsupported_field_row(0);
    }

    #[test]
    fn unit_field_is_an_explicit_unsupported_row_without_expectation_projection() {
        assert_unsupported_field_row(1);
    }

    #[test]
    fn unsupported_prime_field_is_an_explicit_unsupported_row_without_expectation_projection() {
        assert_unsupported_field_row(11);
    }

    #[test]
    fn rates_need_a_strictly_positive_matched_subtraction_and_actual_operations() {
        assert_eq!(net_per_operation(Some(0.2), Some(0.2), 8), None);
        assert_eq!(net_per_operation(Some(0.19), Some(0.2), 8), None);
        assert_eq!(net_per_operation(Some(0.25), Some(0.05), 0), None);
        assert_eq!(net_per_operation(Some(0.25), Some(0.05), 8), Some(0.025));
    }

    #[test]
    fn csv_names_each_observed_frequency_numerator_and_denominator() {
        let row = HorizontalProductResult {
            q: 3,
            n: 12,
            backend: "fold-gf3",
            outcome: Outcome::Measured,
            samples_per_rep: 32,
            reps: 5,
            timed_samples_total: 160,
            zero_fast_samples: 159,
            nonzero_slow_samples: 1,
            zero_fast_observed_frequency: Some(159.0 / 160.0),
            zero_fast_expected_frequency: Some(0.99),
            nonzero_slow_observed_frequency: Some(1.0 / 160.0),
            nonzero_slow_expected_frequency: Some(0.01),
            zero_fast_s: Some(0.2),
            zero_fast_compiler_barrier_baseline_s: Some(0.05),
            zero_fast_net_per_operation_s: Some(0.15 / 159.0),
            zero_fast_timed_operations: 159,
            nonzero_slow_s: Some(0.02),
            nonzero_slow_compiler_barrier_baseline_s: Some(0.01),
            nonzero_slow_net_per_operation_s: Some(0.01),
            nonzero_slow_timed_operations: 1,
            duration_basis: "device_event_kernel",
            timed_purpose: MeasurementPurpose::HorizontalProductTimed,
            timed_index_first: 0,
            overhead_exclusion: "device events exclude host timing",
            note: String::new(),
        };
        let csv = row.to_csv_row();
        assert!(HORIZONTAL_PRODUCT_CSV_HEADER.contains("zero_fast_observed_numerator"));
        assert!(HORIZONTAL_PRODUCT_CSV_HEADER.contains("zero_fast_observed_denominator"));
        assert!(HORIZONTAL_PRODUCT_CSV_HEADER.contains("nonzero_slow_observed_numerator"));
        assert!(HORIZONTAL_PRODUCT_CSV_HEADER.contains("nonzero_slow_observed_denominator"));
        assert!(csv.contains("device_event_kernel"));
        assert_eq!(
            csv.split(',').count(),
            HORIZONTAL_PRODUCT_CSV_HEADER.split(',').count(),
            "CSV row width must remain aligned with its header"
        );
    }

    #[cfg(all(feature = "prototype-registry", not(feature = "hip")))]
    #[test]
    fn scheduled_prototype_without_hip_is_an_explicit_row() {
        let backend = crate::schedule::scheduled_backends(SchedulePhase::HorizontalProductTimed)
            .find(|scheduled| scheduled.backend().prototype_path().is_some())
            .expect("prototype registry supplies a candidate")
            .backend();
        let row = run_horizontal_product(HorizontalProductSpec {
            q: 3,
            n: 12,
            samples: 1,
            backend,
            seed_root: 7,
            seed_index: 0,
        });
        assert_eq!(row.outcome, Outcome::Unsupported);
        assert!(row.note.contains("requires the hip feature"));
        assert!(row.zero_fast_net_per_operation_s.is_none());
        assert!(row.nonzero_slow_net_per_operation_s.is_none());
    }
}
