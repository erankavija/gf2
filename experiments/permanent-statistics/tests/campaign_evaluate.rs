use gf2_core::gfp::Fp;
use permanent_statistics::campaign::{backend_supports_cell, Backend, Kernel};

#[test]
fn supporting_cpu_backends_share_the_per_matrix_contract() {
    for backend in [
        Backend::Scalar,
        Backend::BatchParallel,
        Backend::IntraMatrixParallel,
        Backend::GenericRyser,
    ] {
        let kernel = Kernel::new(3, 2, backend, 2).unwrap();
        let matrices = vec![vec![Fp::<3>::new(1); 4], vec![Fp::<3>::new(0); 4]];
        assert_eq!(kernel.permanents(&matrices).unwrap(), vec![2, 0]);
    }
    assert!(backend_supports_cell(Backend::BatchParallel, 7, 16));
    assert!(!backend_supports_cell(Backend::BatchParallel, 7, 17));
}

mod common;
use gf2_stats::sampler::StreamPurpose;
use permanent_statistics::campaign::{
    evaluate_determinant, evaluate_shard, launch_size, resolve_processor_path, ProcessorPath,
    ShardSpec,
};
use std::time::Duration;

fn field_contract<const Q: u64>() {
    let config = common::config();
    let cell = config
        .cells()
        .into_iter()
        .find(|c| u64::from(c.q) == Q)
        .unwrap();
    let mut reference = Vec::new();
    let baseline = evaluate_shard::<Q, _>(
        config.root_seed,
        StreamPurpose::CampaignCell,
        &cell,
        cell.shard(0).unwrap(),
        Backend::GenericRyser,
        1,
        500,
        &mut |m, p, d| {
            assert_eq!(d, Some(evaluate_determinant(m, cell.n as usize)));
            reference.push((m.to_vec(), p, d));
        },
    )
    .unwrap();
    for backend in common::cpu_backends() {
        if !backend_supports_cell(backend, cell.q, cell.n) {
            continue;
        }
        for workers in [1, 2, 4] {
            let mut observed = Vec::new();
            let actual = evaluate_shard::<Q, _>(
                config.root_seed,
                StreamPurpose::CampaignCell,
                &cell,
                cell.shard(0).unwrap(),
                backend,
                workers,
                500,
                &mut |m, p, d| {
                    observed.push((m.to_vec(), p, d));
                },
            )
            .unwrap();
            assert_eq!(observed, reference);
            assert_eq!(actual.permanent_histogram, baseline.permanent_histogram);
            assert_eq!(
                actual.determinant_zero_count,
                baseline.determinant_zero_count
            );
        }
    }
    let mut without = cell.clone();
    without.determinant_companion = false;
    let actual = evaluate_shard::<Q, _>(
        config.root_seed,
        StreamPurpose::CampaignCell,
        &without,
        without.shard(0).unwrap(),
        Backend::BatchParallel,
        2,
        500,
        &mut |_, _, d| assert!(d.is_none()),
    )
    .unwrap();
    assert_eq!(actual.permanent_histogram, baseline.permanent_histogram);
    assert!(actual.determinant_zero_count.is_none());
}
#[test]
fn shared_shard_observer_and_companion_contract() {
    field_contract::<3>();
    field_contract::<5>();
    field_contract::<7>();
}

#[test]
fn processor_domains_and_paths_have_one_rule() {
    for (backend, q, n, path) in [
        (
            Backend::Scalar,
            3,
            63,
            Some(ProcessorPath::Bipedal3SingleWord),
        ),
        (
            Backend::BatchParallel,
            5,
            63,
            Some(ProcessorPath::Bipedal5SingleWord),
        ),
        (
            Backend::BatchParallel,
            7,
            16,
            Some(ProcessorPath::Bipedal7SingleWord),
        ),
        (Backend::Scalar, 7, 17, None),
        (
            Backend::IntraMatrixParallel,
            3,
            63,
            Some(ProcessorPath::Bipedal3IntraMatrixParallel),
        ),
        (Backend::IntraMatrixParallel, 5, 2, None),
        (
            Backend::GenericRyser,
            7,
            63,
            Some(ProcessorPath::GenericRyser),
        ),
        (
            Backend::Accelerator,
            7,
            63,
            Some(ProcessorPath::Accelerator),
        ),
        (Backend::Scalar, 3, 64, None),
        (Backend::GenericRyser, 2, 2, None),
    ] {
        assert_eq!(backend_supports_cell(backend, q, n), path.is_some());
        if backend == Backend::Accelerator && !cfg!(feature = "hip") {
            assert!(resolve_processor_path(q, n, backend).is_err());
        } else {
            assert_eq!(resolve_processor_path(q, n, backend).ok(), path);
        }
    }
}
#[test]
fn launch_bound_splits_the_tail_without_losing_work() {
    let mut remaining = 13;
    let mut launches = Vec::new();
    while remaining > 0 {
        let size = launch_size(
            Duration::from_micros(100),
            Duration::from_micros(400),
            remaining,
        );
        launches.push(size);
        remaining -= size as u64;
    }
    assert_eq!(launches, vec![4, 4, 4, 1]);
    assert_eq!(launch_size(Duration::ZERO, Duration::ZERO, 13), 13);
    assert_eq!(launch_size(Duration::from_secs(1), Duration::ZERO, 13), 1);
    assert_eq!(launch_size(Duration::ZERO, Duration::ZERO, 0), 1);
}
fn exhaustive<const Q: u64>() {
    let mut singular = 0;
    let kernels: Vec<_> = common::cpu_backends()
        .into_iter()
        .filter(|&b| backend_supports_cell(b, Q as u8, 2))
        .map(|b| Kernel::new(Q as u8, 2, b, 1).unwrap())
        .collect();
    let probability = gf2_algebra::permanent::try_visit_permanent_anchor_matrices::<Q, (), _>(
        2,
        |entries, oracle| {
            singular += u64::from(evaluate_determinant(entries, 2) == 0);
            for kernel in &kernels {
                assert_eq!(
                    kernel.permanents(&[entries.to_vec()]).unwrap()[0],
                    oracle.value()
                );
            }
            Ok(())
        },
    )
    .unwrap();
    let expected = gf2_algebra::permanent::determinant_singular_probability(Q, 2);
    assert_eq!(
        singular.to_string().parse::<u128>().unwrap()
            * expected.matrix_count().to_string().parse::<u128>().unwrap(),
        probability
            .matrix_count()
            .to_string()
            .parse::<u128>()
            .unwrap()
            * expected.zero_count().to_string().parse::<u128>().unwrap()
    );
}
#[test]
fn exhaustive_small_field_permanents_and_determinants_match_independent_counts() {
    exhaustive::<3>();
    exhaustive::<5>();
    exhaustive::<7>();
}
#[test]
fn evaluator_refuses_shape_field_and_zero_workers() {
    assert!(Kernel::new(3, 2, Backend::BatchParallel, 0).is_err());
    let kernel = Kernel::new(3, 2, Backend::Scalar, 1).unwrap();
    assert!(kernel.permanents(&[vec![Fp::<3>::new(0); 3]]).is_err());
    assert!(kernel.permanents(&[vec![Fp::<5>::new(0); 4]]).is_err());
    let config = common::config();
    let cell = &config.cells()[0];
    assert!(evaluate_shard::<5, _>(
        1,
        StreamPurpose::CampaignCell,
        cell,
        ShardSpec {
            shard_id: 0,
            stream_index: 0,
            matrix_count: 1
        },
        Backend::Scalar,
        1,
        500,
        &mut |_, _, _| {}
    )
    .is_err());
}

#[test]
fn accelerator_launch_split_uses_the_same_stream_observer_and_tally_on_a_cpu_dispatch() {
    use permanent_statistics::campaign::evaluate_shard_with_dispatch;
    let config = common::config();
    let mut cell = config.cells()[2].clone();
    cell.matrix_count = 13;
    cell.shard_size = 13;
    cell.accelerator_per_matrix_us = Some(100_000.0);
    let spec = cell.shard(0).unwrap();
    let mut expected_observed = Vec::new();
    let expected = evaluate_shard::<7, _>(
        config.root_seed,
        StreamPurpose::CampaignCell,
        &cell,
        spec,
        Backend::GenericRyser,
        1,
        500,
        &mut |m, p, d| {
            expected_observed.push((m.to_vec(), p, d));
        },
    )
    .unwrap();
    let kernel = Kernel::new(7, 2, Backend::GenericRyser, 1).unwrap();
    let mut launches = Vec::new();
    let mut observed = Vec::new();
    let mut actual = evaluate_shard_with_dispatch::<7, _, _>(
        config.root_seed,
        StreamPurpose::CampaignCell,
        &cell,
        spec,
        Backend::Accelerator,
        500,
        &mut |m, p, d| {
            observed.push((m.to_vec(), p, d));
        },
        &mut |matrices| {
            launches.push(matrices.len());
            kernel.permanents(matrices)
        },
    )
    .unwrap();
    assert_eq!(launches, vec![5, 5, 3]);
    assert_eq!(observed, expected_observed);
    actual.executed_backend = Backend::GenericRyser;
    assert_eq!(actual, expected);
    assert!(evaluate_shard_with_dispatch::<7, _, _>(
        config.root_seed,
        StreamPurpose::CampaignCell,
        &cell,
        spec,
        Backend::Accelerator,
        500,
        &mut |_, _, _| {},
        &mut |_| Ok(vec![7; 5])
    )
    .is_err());
}
