use gf2_stats::accumulator::{
    Accumulator, AccumulatorError, AccumulatorSnapshot, CellId, CommitPoint, DeterminantCounts,
    OfferOutcome, Shard, ShardCounts, ShardId,
};

fn cell(q: u64) -> CellId {
    CellId::new(q, 4).expect("test cell is valid")
}

fn counts(matrix_count: u64, histogram: Vec<u64>, determinant: DeterminantCounts) -> ShardCounts {
    ShardCounts::new(matrix_count, histogram, determinant).expect("test counts are valid")
}

fn shard(id: &str, matrix_count: u64, histogram: Vec<u64>) -> Shard {
    Shard::with_counts(
        ShardId::new(id),
        cell(3),
        counts(matrix_count, histogram, DeterminantCounts::NotEvaluated),
    )
}

fn evaluated_shard(id: &str, matrix_count: u64, histogram: Vec<u64>) -> Shard {
    Shard::with_counts(
        ShardId::new(id),
        cell(3),
        counts(
            matrix_count,
            histogram,
            DeterminantCounts::Evaluated {
                sample_count: matrix_count,
                zero_count: 0,
            },
        ),
    )
}

#[test]
fn interrupted_commit_leaves_pooled_state_unchanged_req_01() {
    let mut accumulator = Accumulator::new();
    accumulator.offer(shard("a", 2, vec![1, 0, 1])).unwrap();
    let before = accumulator.snapshot_bytes().unwrap();

    let result = accumulator.offer_with_interrupt(shard("b", 3, vec![1, 1, 1]), |point| {
        if point == CommitPoint::Applied {
            Err(AccumulatorError::CommitInterrupted { point })
        } else {
            Ok(())
        }
    });

    assert!(matches!(
        result,
        Err(AccumulatorError::CommitInterrupted {
            point: CommitPoint::Applied
        })
    ));
    assert_eq!(accumulator.snapshot_bytes().unwrap(), before);
}

#[test]
fn duplicate_shard_identity_is_rejected_without_counting_req_02() {
    let mut accumulator = Accumulator::new();
    accumulator.offer(shard("same", 2, vec![1, 0, 1])).unwrap();
    let result = accumulator.offer(shard("same", 7, vec![3, 2, 2]));

    assert!(matches!(
        result,
        Err(AccumulatorError::DuplicateShard { ref identity }) if identity == &ShardId::new("same")
    ));
    let state = accumulator.pooled_state();
    assert_eq!(state.cells[0].matrix_count, 2);
    assert_eq!(state.committed_shards, vec![ShardId::new("same")]);
}

#[test]
fn shuffled_offer_permutations_have_identical_snapshot_bytes_req_03() {
    let shards = [
        shard("c", 2, vec![1, 0, 1]),
        shard("a", 3, vec![1, 1, 1]),
        shard("b", 4, vec![2, 1, 1]),
    ];
    let permutations = [
        vec![0, 1, 2],
        vec![0, 2, 1],
        vec![1, 0, 2],
        vec![1, 2, 0],
        vec![2, 0, 1],
        vec![2, 1, 0],
    ];
    let expected = {
        let mut accumulator = Accumulator::new();
        for index in &permutations[0] {
            accumulator.offer(shards[*index].clone()).unwrap();
        }
        accumulator.snapshot_bytes().unwrap()
    };

    for permutation in &permutations[1..] {
        let mut accumulator = Accumulator::new();
        for index in permutation {
            accumulator.offer(shards[*index].clone()).unwrap();
        }
        assert_eq!(accumulator.snapshot_bytes().unwrap(), expected);
    }
}

#[test]
fn snapshot_restore_and_continue_matches_uninterrupted_run_req_04() {
    let shards = vec![
        evaluated_shard("a", 2, vec![1, 0, 1]),
        evaluated_shard("b", 3, vec![1, 1, 1]),
        evaluated_shard("c", 4, vec![2, 1, 1]),
    ];
    let mut uninterrupted = Accumulator::new();
    for shard in &shards {
        uninterrupted.offer(shard.clone()).unwrap();
    }

    let mut resumed = Accumulator::new();
    resumed.offer(shards[0].clone()).unwrap();
    let snapshot = resumed.snapshot_bytes().unwrap();
    let mut restored = Accumulator::from_snapshot_bytes(&snapshot).unwrap();
    for shard in &shards[1..] {
        restored.offer(shard.clone()).unwrap();
    }

    assert_eq!(
        restored.snapshot_bytes().unwrap(),
        uninterrupted.snapshot_bytes().unwrap()
    );
}

#[test]
fn pooled_cell_carries_matrix_zero_and_q_histogram_req_05() {
    let mut accumulator = Accumulator::new();
    accumulator.offer(shard("a", 3, vec![1, 1, 1])).unwrap();
    accumulator.offer(shard("b", 4, vec![2, 1, 1])).unwrap();

    let pooled = &accumulator.pooled_state().cells[0];
    assert_eq!(pooled.matrix_count, 7);
    assert_eq!(pooled.permanent_zero_count, 3);
    assert_eq!(pooled.permanent_histogram, vec![3, 2, 2]);
    assert_eq!(pooled.shard_ids, vec![ShardId::new("a"), ShardId::new("b")]);
}

#[test]
fn determinant_state_distinguishes_not_evaluated_and_evaluated_zero_req_06() {
    let mut not_evaluated = Accumulator::new();
    not_evaluated.offer(shard("a", 2, vec![1, 0, 1])).unwrap();
    assert_eq!(
        not_evaluated.pooled_state().cells[0].determinant,
        DeterminantCounts::NotEvaluated
    );

    let mut evaluated = Accumulator::new();
    evaluated
        .offer(evaluated_shard("a", 2, vec![1, 0, 1]))
        .unwrap();
    assert_eq!(
        evaluated.pooled_state().cells[0].determinant,
        DeterminantCounts::Evaluated {
            sample_count: 2,
            zero_count: 0,
        }
    );
}

#[test]
fn failed_shard_is_quarantined_until_explicit_readmission_req_07() {
    let mut accumulator = Accumulator::new();
    let failed = Shard::failed(ShardId::new("failed"), cell(5), "evaluation timed out");
    assert_eq!(
        accumulator.offer(failed).unwrap(),
        OfferOutcome::Quarantined {
            identity: ShardId::new("failed")
        }
    );
    assert_eq!(accumulator.pooled_state().cells.len(), 0);
    assert_eq!(accumulator.pooled_state().quarantined.len(), 1);
    assert_eq!(
        accumulator.pooled_state().quarantined[0].reason,
        "evaluation timed out"
    );

    accumulator
        .readmit(
            &ShardId::new("failed"),
            counts(1, vec![1, 0, 0, 0, 0], DeterminantCounts::NotEvaluated),
        )
        .unwrap();
    assert!(accumulator.pooled_state().quarantined.is_empty());
    assert_eq!(accumulator.pooled_state().cells[0].matrix_count, 1);
}

#[test]
fn snapshot_schema_version_mismatch_is_refused_req_08() {
    let snapshot = AccumulatorSnapshot {
        schema_version: AccumulatorSnapshot::SCHEMA_VERSION + 1,
        ..Accumulator::new().snapshot()
    };
    let bytes = serde_json::to_vec(&snapshot).unwrap();

    assert!(matches!(
        Accumulator::from_snapshot_bytes(&bytes),
        Err(AccumulatorError::UnsupportedSchemaVersion { found, expected })
            if found == AccumulatorSnapshot::SCHEMA_VERSION + 1
                && expected == AccumulatorSnapshot::SCHEMA_VERSION
    ));
}

#[test]
fn pooled_state_exposes_all_committed_shard_identities_req_09() {
    let mut accumulator = Accumulator::new();
    accumulator.offer(shard("b", 2, vec![1, 0, 1])).unwrap();
    accumulator.offer(shard("a", 3, vec![1, 1, 1])).unwrap();

    assert_eq!(
        accumulator.pooled_state().committed_shards,
        vec![ShardId::new("a"), ShardId::new("b")]
    );
    assert_eq!(
        accumulator.pooled_state().cells[0].shard_ids,
        vec![ShardId::new("a"), ShardId::new("b")]
    );
}
