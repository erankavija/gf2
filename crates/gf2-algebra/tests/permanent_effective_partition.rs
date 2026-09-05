//! Effective-partition witness for the production parallel permanent walk.

#![cfg(feature = "parallel")]

use gf2_algebra::packed::Bipedal3Matrix;
use gf2_algebra::permanent::parallel_bipedal3::{
    last_effective_chunk, last_effective_partition, permanent_bipedal3_parallel_with_chunk,
    permanent_chunk_len, reset_last_effective_partition, PermanentPartitionObservation,
};
use gf2_algebra::permanent::{permanent_bipedal3, permanent_bipedal3_parallel};
use gf2_core::gfp::Fp;

fn matrix(n: usize) -> Bipedal3Matrix {
    let entries = (0..n * n)
        .map(|index| Fp::<3>::new((index as u64 * 7 + 1) % 3))
        .collect::<Vec<_>>();
    Bipedal3Matrix::from_row_major(&entries, n, n)
}

fn assert_partition(n: usize, chunk_subsets: usize, expected: PermanentPartitionObservation) {
    reset_last_effective_partition();
    assert_eq!(last_effective_partition(), None);

    let _ = permanent_bipedal3_parallel_with_chunk(&matrix(n), chunk_subsets);

    assert_eq!(last_effective_chunk(), chunk_subsets);
    assert_eq!(last_effective_partition(), Some(expected));
}

#[test]
fn completed_parallel_walk_reports_its_effective_partition() {
    reset_last_effective_partition();
    assert_eq!(last_effective_partition(), None);

    let empty = Bipedal3Matrix::from_row_major(&[], 0, 0);
    assert_eq!(
        permanent_bipedal3_parallel_with_chunk(&empty, 11),
        Fp::<3>::new(1)
    );
    assert_eq!(last_effective_chunk(), 11);
    assert_eq!(last_effective_partition(), None);

    reset_last_effective_partition();
    let invalid_chunk = std::panic::catch_unwind(|| {
        let _ = permanent_bipedal3_parallel_with_chunk(&matrix(2), 0);
    });
    assert!(invalid_chunk.is_err());
    assert_eq!(last_effective_partition(), None);

    assert_partition(
        3,
        7,
        PermanentPartitionObservation {
            maximum_chunk_len: 7,
            chunk_count: 1,
            last_chunk_len: 7,
        },
    );
    assert_partition(
        4,
        5,
        PermanentPartitionObservation {
            maximum_chunk_len: 5,
            chunk_count: 3,
            last_chunk_len: 5,
        },
    );
    assert_partition(
        4,
        6,
        PermanentPartitionObservation {
            maximum_chunk_len: 6,
            chunk_count: 3,
            last_chunk_len: 3,
        },
    );
    assert_partition(
        4,
        20,
        PermanentPartitionObservation {
            maximum_chunk_len: 15,
            chunk_count: 1,
            last_chunk_len: 15,
        },
    );
    assert_partition(
        20,
        1_048_576,
        PermanentPartitionObservation {
            maximum_chunk_len: 1_048_575,
            chunk_count: 1,
            last_chunk_len: 1_048_575,
        },
    );

    let maximum_chunk_matrix = matrix(4);
    let maximum_chunk_expected = permanent_bipedal3(&maximum_chunk_matrix);
    reset_last_effective_partition();
    let maximum_chunk_actual =
        permanent_bipedal3_parallel_with_chunk(&maximum_chunk_matrix, usize::MAX);
    assert_eq!(maximum_chunk_actual, maximum_chunk_expected);
    assert_eq!(last_effective_chunk(), usize::MAX);
    assert_eq!(
        last_effective_partition(),
        Some(PermanentPartitionObservation {
            maximum_chunk_len: 15,
            chunk_count: 1,
            last_chunk_len: 15,
        })
    );

    let public_matrix = matrix(5);
    let expected = permanent_bipedal3(&public_matrix);
    reset_last_effective_partition();
    let actual = permanent_bipedal3_parallel(&public_matrix);
    assert_eq!(actual, expected);
    assert_eq!(last_effective_chunk(), permanent_chunk_len());
    assert_eq!(
        last_effective_partition(),
        Some(PermanentPartitionObservation {
            maximum_chunk_len: 31,
            chunk_count: 1,
            last_chunk_len: 31,
        })
    );
}
