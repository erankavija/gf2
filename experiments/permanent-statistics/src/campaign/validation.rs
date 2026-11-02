//! Independent exhaustive and sampled validation on separate streams.
use super::{
    backend_available, backend_supports_cell, evaluate_determinant, evaluate_shard, invalid,
    Anchor, Backend, CampaignConfig, CampaignError, Cell, Kernel, ShardSpec,
};
use crate::execution_log::ExecutionLog;
use crate::sampler::StreamPurpose;
use experiments_common::{publish_atomic, RunRecord, Sha256Digest};
use gf2_algebra::permanent::{
    determinant_singular_probability, try_visit_permanent_anchor_matrices,
};
use gf2_core::gfp::Fp;
use gf2_stats::binomial::two_sided_test;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Exact comparison status for one registered implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendStatus {
    /// Outside the backend's mathematical domain.
    Unsupported,
    /// Every compared value agrees.
    Passed,
    /// At least one value differs.
    Failed,
}
/// Per-matrix agreement with the independent fixed-expansion oracle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BackendAgreement {
    /// Registered implementation.
    pub backend: Backend,
    /// Mathematical support and result.
    pub status: BackendStatus,
    /// Matrices compared.
    pub compared: u64,
    /// Disagreements, preserved even when the anchor fails.
    pub mismatches: u64,
}
/// Two fresh draws of the same validation stream.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Regeneration {
    /// First serial, then configured workers.
    pub workers: [usize; 2],
    /// Address-ordered entry and permanent digests.
    pub sha256: [Sha256Digest; 2],
    /// Digest equality.
    pub identical: bool,
}
/// Exact-null statistical check of independent random draws.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    /// Fixed draw count.
    pub matrix_count: u64,
    /// Permanent-zero count.
    pub zero_count: u64,
    /// Probability-ordering exact test evidence.
    pub log_p_value: f64,
}
/// Terminal anchor evidence; a dispatch failure retains completed checks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnchorResult {
    /// Field order.
    pub q: u8,
    /// Matrix order.
    pub n: u16,
    /// Exhaustive matrix count.
    pub enumerated: u64,
    /// Independent oracle zeros.
    pub oracle_zero_count: u64,
    /// Exhaustive determinant zeros.
    pub determinant_zero_count: u64,
    /// Every registered backend, including unsupported entries.
    pub backends: Vec<BackendAgreement>,
    /// Fresh-stream comparison, only after exact checks pass.
    pub regeneration: Option<Regeneration>,
    /// Exact-null draw check, only after exact checks pass.
    pub sample: Option<Sample>,
    /// Dispatch diagnostic for a terminal mechanical failure.
    pub failure: Option<String>,
    /// All anchor checks pass.
    pub verdict: bool,
}
/// Durable validation result and its full producing inputs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Receipt {
    /// First observed run record; source revision is only a rebuild pointer.
    pub record: RunRecord,
    /// Complete preregistration.
    pub configuration: CampaignConfig,
    /// Completed anchors in registration order.
    pub anchors: Vec<AnchorResult>,
    /// All anchors completed and passed.
    pub verdict: bool,
}
/// Result of a bounded validation invocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationStatus {
    /// Every anchor passed.
    Passed,
    /// A terminal anchor failed.
    Failed,
    /// Registered anchors remain.
    Incomplete,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Event {
    Start { record: Box<RunRecord> },
    AnchorTerminal { anchor: AnchorResult },
    InvocationEnd { status: String },
}
/// Probes every applicable backend on an unsampled zero matrix before opening a stream.
/// Refuses invalid parameters, unavailable implementations, and dispatch errors.
pub fn preflight_required_backends(
    config: &CampaignConfig,
    workers: usize,
) -> Result<(), CampaignError> {
    config.validate()?;
    if workers == 0 {
        return invalid("positive workers required");
    }
    for anchor in &config.validation.anchors {
        for backend in config
            .validation
            .selectable_backends
            .iter()
            .copied()
            .chain([config.validation.sample_backend])
        {
            if !backend_supports_cell(backend, anchor.q, anchor.n) {
                continue;
            }
            if !backend_available(backend) {
                return Err(CampaignError::BackendUnavailable);
            }
            super::guard_dispatch(|| match anchor.q {
                3 => probe_backend::<3>(anchor, backend, workers),
                5 => probe_backend::<5>(anchor, backend, workers),
                7 => probe_backend::<7>(anchor, backend, workers),
                _ => unreachable!(),
            })?;
        }
    }
    Ok(())
}
fn probe_backend<const Q: u64>(
    anchor: &Anchor,
    backend: Backend,
    workers: usize,
) -> Result<(), CampaignError> {
    let kernel = Kernel::new(anchor.q, anchor.n, backend, workers)?;
    let matrices = vec![vec![Fp::<Q>::new(0); usize::from(anchor.n).pow(2)]];
    if kernel.permanents(&matrices)?.len() != 1 {
        return invalid("backend probe returned an inconsistent batch");
    }
    Ok(())
}
fn validation_cell(config: &CampaignConfig, anchor: &Anchor, count: u64) -> Cell {
    Cell {
        q: anchor.q,
        n: anchor.n,
        matrix_count: count,
        shard_size: count,
        backend: config.validation.sample_backend,
        determinant_companion: true,
        backend_evidence: config.reference_data.reproduction_targets.clone(),
        ordinal: 0,
        accelerator_per_matrix_us: None,
    }
}
fn draws<const Q: u64>(
    config: &CampaignConfig,
    anchor: &Anchor,
    count: u64,
    workers: usize,
    mut observer: impl FnMut(&[Fp<Q>], u64, Option<u64>),
) -> Result<super::dataset::ShardRecord, CampaignError> {
    #[cfg(test)]
    tests::before_draw()?;
    let cell = validation_cell(config, anchor, count);
    evaluate_shard::<Q, _>(
        config.validation.root_seed,
        StreamPurpose::Validation,
        &cell,
        ShardSpec {
            shard_id: 0,
            stream_index: anchor.stream_index,
            matrix_count: count,
        },
        config.validation.sample_backend,
        workers,
        1,
        &mut observer,
    )
}
fn exact_anchor<const Q: u64>(
    config: &CampaignConfig,
    anchor: &Anchor,
    workers: usize,
) -> Result<AnchorResult, CampaignError> {
    let mut backends = Vec::new();
    let mut kernels = Vec::new();
    for &backend in &config.validation.selectable_backends {
        let supported = backend_supports_cell(backend, anchor.q, anchor.n);
        kernels.push(if supported {
            Some(Kernel::new(anchor.q, anchor.n, backend, workers)?)
        } else {
            None
        });
        backends.push(BackendAgreement {
            backend,
            status: if supported {
                BackendStatus::Passed
            } else {
                BackendStatus::Unsupported
            },
            compared: 0,
            mismatches: 0,
        });
    }
    let mut matrices = Vec::new();
    let mut expected = Vec::new();
    let mut determinant_zero_count = 0;
    let mut compare =
        |matrices: &mut Vec<Vec<Fp<Q>>>, expected: &mut Vec<u64>| -> Result<(), CampaignError> {
            for (kernel, agreement) in kernels.iter_mut().zip(&mut backends) {
                if let Some(evaluator) = kernel {
                    match super::guard_dispatch(|| evaluator.permanents(matrices)) {
                        Ok(values) if values.len() == expected.len() => {
                            agreement.compared += values.len() as u64;
                            agreement.mismatches += values
                                .iter()
                                .zip(expected.iter())
                                .filter(|(a, b)| a != b)
                                .count() as u64;
                            if agreement.mismatches > 0 {
                                agreement.status = BackendStatus::Failed;
                            }
                        }
                        _ => {
                            agreement.status = BackendStatus::Failed;
                            *kernel = None;
                        }
                    }
                }
            }
            matrices.clear();
            expected.clear();
            Ok(())
        };
    let oracle = try_visit_permanent_anchor_matrices::<Q, CampaignError, _>(
        anchor.n as usize,
        |entries, value| {
            determinant_zero_count +=
                u64::from(evaluate_determinant(entries, anchor.n as usize) == 0);
            matrices.push(entries.to_vec());
            expected.push(value.value());
            if matrices.len() == config.validation.exact_batch_matrix_count {
                compare(&mut matrices, &mut expected)?;
            }
            Ok(())
        },
    )?;
    if !matrices.is_empty() {
        compare(&mut matrices, &mut expected)?;
    }
    let enumerated = oracle
        .matrix_count()
        .to_string()
        .parse::<u64>()
        .map_err(|e| CampaignError::Invalid(e.to_string()))?;
    let oracle_zero_count = oracle
        .zero_count()
        .to_string()
        .parse::<u64>()
        .map_err(|e| CampaignError::Invalid(e.to_string()))?;
    let determinant = determinant_singular_probability(Q, anchor.n as usize);
    let determinant_exact = u128::from(determinant_zero_count)
        * determinant
            .matrix_count()
            .to_string()
            .parse::<u128>()
            .unwrap()
        == u128::from(enumerated)
            * determinant
                .zero_count()
                .to_string()
                .parse::<u128>()
                .unwrap();
    let exact_pass = oracle_zero_count == anchor.permanent_zero_count
        && determinant_exact
        && backends.iter().all(|a| a.status != BackendStatus::Failed);
    let mut result = AnchorResult {
        q: anchor.q,
        n: anchor.n,
        enumerated,
        oracle_zero_count,
        determinant_zero_count,
        backends,
        regeneration: None,
        sample: None,
        failure: None,
        verdict: exact_pass,
    };
    if !exact_pass {
        return Ok(result);
    }
    let sampled = super::guard_dispatch(|| {
        let mut digests = Vec::new();
        for w in [1, workers] {
            let mut bytes = Vec::new();
            draws::<Q>(
                config,
                anchor,
                config.validation.regeneration_matrix_count,
                w,
                |entries, p, _| {
                    bytes.extend(entries.iter().map(|e| e.value() as u8));
                    bytes.push(p as u8);
                },
            )?;
            digests.push(Sha256Digest::of(&bytes));
        }
        let identical = digests[0] == digests[1];
        result.regeneration = Some(Regeneration {
            workers: [1, workers],
            sha256: [digests[0].clone(), digests[1].clone()],
            identical,
        });
        let sample = draws::<Q>(
            config,
            anchor,
            config.validation.sample_matrix_count,
            workers,
            |_, _, _| {},
        )?;
        let test = two_sided_test(
            sample.permanent_zero_count,
            sample.matrix_count,
            oracle_zero_count as f64 / enumerated as f64,
        );
        result.sample = Some(Sample {
            matrix_count: sample.matrix_count,
            zero_count: sample.permanent_zero_count,
            log_p_value: test.log_p_value(),
        });
        result.verdict = identical && !test.rejects_at(config.validation.level);
        Ok(())
    });
    if let Err(error) = sampled {
        result.failure = Some(error.to_string());
        result.verdict = false;
    }
    Ok(result)
}
/// Resumes at anchor boundaries; a terminal failure remains failed and is never redrawn.
/// A killed anchor without a terminal outcome restarts at its identical address. Campaign
/// addresses are never opened. Refuses a changed closed identity or backend unavailability.
pub fn run_validation(
    config: &CampaignConfig,
    record: &RunRecord,
    directory: &Path,
    workers: usize,
    max_anchors: Option<usize>,
) -> Result<ValidationStatus, CampaignError> {
    config.validate()?;
    if workers == 0 || max_anchors == Some(0) {
        return invalid("positive workers and anchor budget required");
    }
    if record.identity.configuration_sha256 != config.digest()?
        || record.identity.seed != config.validation.root_seed
    {
        return invalid("validation identity differs from configuration");
    }
    preflight_required_backends(config, workers)?;
    let mut log = ExecutionLog::open(&directory.join("execution.jsonl"))?;
    let result = (|| {
        let mut anchors = Vec::new();
        let mut first_record = None;
        for event in ExecutionLog::read::<Event>(&directory.join("execution.jsonl"))? {
            match event {
                Event::Start { record: previous } => {
                    if previous.identity != record.identity {
                        return invalid("validation run identity differs");
                    }
                    first_record.get_or_insert(*previous);
                }
                Event::AnchorTerminal { anchor } => {
                    let expected =
                        config
                            .validation
                            .anchors
                            .get(anchors.len())
                            .ok_or_else(|| {
                                CampaignError::Invalid("unexpected terminal anchor".into())
                            })?;
                    if (anchor.q, anchor.n) != (expected.q, expected.n) || first_record.is_none() {
                        return invalid("terminal anchor out of order");
                    }
                    anchors.push(anchor);
                }
                Event::InvocationEnd { .. } => {}
            }
        }
        log.append(&Event::Start {
            record: Box::new(record.clone()),
        })?;
        let budget = max_anchors.unwrap_or(usize::MAX);
        for anchor in config
            .validation
            .anchors
            .iter()
            .skip(anchors.len())
            .take(budget)
        {
            let result = super::guard_dispatch(|| match anchor.q {
                3 => exact_anchor::<3>(config, anchor, workers),
                5 => exact_anchor::<5>(config, anchor, workers),
                7 => exact_anchor::<7>(config, anchor, workers),
                _ => unreachable!(),
            })
            .unwrap_or_else(|error| AnchorResult {
                q: anchor.q,
                n: anchor.n,
                enumerated: 0,
                oracle_zero_count: 0,
                determinant_zero_count: 0,
                backends: Vec::new(),
                regeneration: None,
                sample: None,
                failure: Some(error.to_string()),
                verdict: false,
            });
            log.append(&Event::AnchorTerminal {
                anchor: result.clone(),
            })?;
            anchors.push(result);
        }
        let complete = anchors.len() == config.validation.anchors.len();
        let failed = anchors.iter().any(|a| !a.verdict);
        let receipt = Receipt {
            record: first_record.unwrap_or_else(|| record.clone()),
            configuration: config.clone(),
            anchors,
            verdict: complete && !failed,
        };
        publish_atomic(
            &directory.join("receipt.json"),
            &serde_json::to_vec_pretty(&receipt)?,
        )?;
        Ok(if !complete {
            ValidationStatus::Incomplete
        } else if failed {
            ValidationStatus::Failed
        } else {
            ValidationStatus::Passed
        })
    })();
    log.append(&Event::InvocationEnd {
        status: match &result {
            Ok(s) => format!("{s:?}"),
            Err(e) => format!("failed: {e}"),
        },
    })?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell as LocalCell;

    thread_local! {
        static DRAW_FAILURE: LocalCell<Option<(usize, bool)>> = const { LocalCell::new(None) };
    }

    pub(super) fn before_draw() -> Result<(), CampaignError> {
        DRAW_FAILURE.with(|fault| match fault.get() {
            Some((0, panic)) => {
                fault.set(None);
                if panic {
                    panic!("injected draw failure");
                }
                invalid("injected draw failure")
            }
            Some((remaining, panic)) => {
                fault.set(Some((remaining - 1, panic)));
                Ok(())
            }
            None => Ok(()),
        })
    }

    #[test]
    fn regeneration_and_sampling_dispatch_failures_are_terminal_on_resume() {
        let mut config: CampaignConfig =
            serde_json::from_str(include_str!("../../config/small-campaign.json")).unwrap();
        config.validation.anchors.truncate(1);
        let root =
            experiments_common::RepositoryRoot::containing(Path::new(env!("CARGO_MANIFEST_DIR")))
                .unwrap();
        let record = RunRecord::observe(
            &root,
            config.digest().unwrap(),
            config.validation.root_seed,
            crate::sampler_rng(),
        )
        .unwrap();
        for draw in 0..3 {
            for panic in [false, true] {
                let directory = tempfile::tempdir().unwrap();
                DRAW_FAILURE.with(|fault| fault.set(Some((draw, panic))));
                assert_eq!(
                    run_validation(&config, &record, directory.path(), 2, None).unwrap(),
                    ValidationStatus::Failed,
                    "draw {draw}, panic {panic}"
                );
                let path = directory.path().join("receipt.json");
                let bytes = std::fs::read(&path).unwrap();
                let receipt: Receipt = serde_json::from_slice(&bytes).unwrap();
                let anchor = &receipt.anchors[0];
                assert!(!anchor.verdict);
                assert_eq!(anchor.enumerated, 3);
                assert_eq!(anchor.oracle_zero_count, 1);
                assert!(anchor
                    .failure
                    .as_ref()
                    .unwrap()
                    .contains("injected draw failure"));
                assert_eq!(anchor.regeneration.is_some(), draw == 2);
                assert!(anchor.sample.is_none());
                assert_eq!(
                    run_validation(&config, &record, directory.path(), 2, None).unwrap(),
                    ValidationStatus::Failed
                );
                assert_eq!(std::fs::read(&path).unwrap(), bytes);
                let terminals =
                    ExecutionLog::read::<Event>(&directory.path().join("execution.jsonl"))
                        .unwrap()
                        .into_iter()
                        .filter(|event| matches!(event, Event::AnchorTerminal { .. }))
                        .count();
                assert_eq!(terminals, 1);
            }
        }
    }
}
