//! Index-ordered parallel dispatch: a batch split into contiguous partitions,
//! one per caller-owned worker workspace, each writing its own sub-slice of a
//! caller-provided output.

use std::ops::Range;

/// Returns the first item index of partition `index` when `items` items are
/// split over `partitions` contiguous partitions.
///
/// Each partition holds $\lfloor m/w \rfloor$ items and the first
/// $m \bmod w$ hold one more, so no two partitions differ by more than one
/// item and none is empty for $w \le m$. The offset depends on the counts
/// alone, so a sub-range of $w'$ partitions holding the items this rule gives
/// them re-derives the same boundaries inside itself.
///
/// # Panics
///
/// Panics when `partitions` is zero.
#[inline]
#[must_use]
pub fn partition_offset(items: usize, partitions: usize, index: usize) -> usize {
    index * (items / partitions) + index.min(items % partitions)
}

/// Where [`for_each_partition`] runs its partitions.
///
/// A caller crate with its own `parallel` feature selects by that feature, so
/// its threading does not change when feature unification enables gf2-core's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionDispatch {
    /// The halves of each split go to [`rayon::join`] under gf2-core's
    /// `parallel` feature and run left before right on the calling thread
    /// without it.
    Pool,
    /// Every partition runs on the calling thread, in partition order.
    CallingThread,
}

/// Splits `outputs` into one contiguous partition per workspace and calls
/// `f(workspace, items, partition_outputs)` once for each.
///
/// `items` is the partition's index range within `outputs`; callers with an
/// input slice of the same length take `&inputs[items]`. Boundaries follow
/// [`partition_offset`] over `min(workspaces.len(), outputs.len())`
/// partitions, so they depend only on the two lengths: partition `p` runs
/// with `workspaces[p]`, workspaces beyond the item count are not touched, and
/// an empty `outputs` calls `f` never.
///
/// The result does not depend on the workspace count when `f` writes each
/// output from its item index and the inputs alone: any workspace state it
/// reads is reset per partition or equivalent across workspaces, and no output
/// depends on the items processed before it in the same partition.
///
/// The partitions are reached by halving the workspace slice, on the threads
/// `dispatch` selects. One partition runs directly on the calling thread
/// without reaching the pool. The dispatch allocates nothing.
///
/// # Panics
///
/// Panics when `workspaces` is empty and `outputs` is not, and propagates a
/// panic of `f`.
///
/// # Complexity
///
/// One `f` call per partition and $O(\log w)$ dispatch depth for $w$
/// partitions.
///
/// # Examples
///
/// ```
/// use gf2_core::compute::{for_each_partition, PartitionDispatch};
///
/// let messages: Vec<u64> = (0..100).collect();
/// let mut codewords = vec![0_u64; messages.len()];
/// // One scratch buffer per worker, allocated once and reused across batches.
/// let mut workspaces = vec![Vec::<u64>::new(); 4];
/// for_each_partition(
///     PartitionDispatch::Pool,
///     &mut codewords,
///     &mut workspaces,
///     |scratch, items, out| {
///         scratch.clear();
///         scratch.extend(messages[items].iter().map(|m| m.wrapping_mul(3)));
///         out.copy_from_slice(scratch);
///     },
/// );
/// assert_eq!(codewords[99], 297);
/// ```
pub fn for_each_partition<O, W, F>(
    dispatch: PartitionDispatch,
    outputs: &mut [O],
    workspaces: &mut [W],
    f: F,
) where
    O: Send,
    W: Send,
    F: Fn(&mut W, Range<usize>, &mut [O]) + Sync,
{
    if outputs.is_empty() {
        return;
    }
    assert!(
        !workspaces.is_empty(),
        "a non-empty batch needs at least one workspace"
    );
    let partitions = workspaces.len().min(outputs.len());
    partitions_over(dispatch, 0, outputs, &mut workspaces[..partitions], &f);
}

fn partitions_over<O, W, F>(
    dispatch: PartitionDispatch,
    first: usize,
    outputs: &mut [O],
    workspaces: &mut [W],
    f: &F,
) where
    O: Send,
    W: Send,
    F: Fn(&mut W, Range<usize>, &mut [O]) + Sync,
{
    if let [workspace] = workspaces {
        f(workspace, first..first + outputs.len(), outputs);
        return;
    }

    let left_workers = workspaces.len() / 2;
    let at = partition_offset(outputs.len(), workspaces.len(), left_workers);
    let (left_outputs, right_outputs) = outputs.split_at_mut(at);
    let (left_workspaces, right_workspaces) = workspaces.split_at_mut(left_workers);
    let left = |outputs: &mut [O], workspaces: &mut [W]| {
        partitions_over(dispatch, first, outputs, workspaces, f);
    };
    let right = |outputs: &mut [O], workspaces: &mut [W]| {
        partitions_over(dispatch, first + at, outputs, workspaces, f);
    };

    match dispatch {
        #[cfg(feature = "parallel")]
        PartitionDispatch::Pool => {
            rayon::join(
                || left(left_outputs, left_workspaces),
                || right(right_outputs, right_workspaces),
            );
        }
        _ => {
            left(left_outputs, left_workspaces);
            right(right_outputs, right_workspaces);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LENGTHS: [usize; 5] = [0, 1, 63, 64, 65];
    const WORKERS: [usize; 6] = [1, 2, 3, 7, 64, 100];

    fn value(index: usize) -> u64 {
        (index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x5555
    }

    /// Workspace that records every partition it ran and carries running
    /// state across the items of its own partition.
    #[derive(Default)]
    struct Recorder {
        ranges: Vec<Range<usize>>,
        fold: u64,
    }

    const DISPATCHES: [PartitionDispatch; 2] =
        [PartitionDispatch::Pool, PartitionDispatch::CallingThread];

    fn cases() -> impl Iterator<Item = (PartitionDispatch, usize)> {
        DISPATCHES
            .into_iter()
            .flat_map(|dispatch| WORKERS.map(|workers| (dispatch, workers)))
    }

    fn run(
        dispatch: PartitionDispatch,
        len: usize,
        workers: usize,
    ) -> (Vec<u64>, Vec<u32>, Vec<Recorder>) {
        let inputs: Vec<u64> = (0..len).map(value).collect();
        let mut outputs = vec![(0_u64, 0_u32); len];
        let mut workspaces: Vec<Recorder> = (0..workers).map(|_| Recorder::default()).collect();
        for_each_partition(
            dispatch,
            &mut outputs,
            &mut workspaces,
            |workspace, items, out| {
                assert_eq!(out.len(), items.len());
                workspace.ranges.push(items.clone());
                for (input, slot) in inputs[items].iter().zip(out) {
                    workspace.fold = workspace.fold.wrapping_add(*input);
                    slot.0 = input.rotate_left(7);
                    slot.1 += 1;
                }
            },
        );
        let (values, visits) = outputs.into_iter().unzip();
        (values, visits, workspaces)
    }

    #[test]
    fn test_output_is_identical_across_worker_counts() {
        for len in LENGTHS {
            let expected: Vec<u64> = (0..len).map(|i| value(i).rotate_left(7)).collect();
            for (dispatch, workers) in cases() {
                let (values, _, _) = run(dispatch, len, workers);
                assert_eq!(
                    values, expected,
                    "{dispatch:?}, len {len}, {workers} workers"
                );
            }
        }
    }

    #[test]
    fn test_every_item_is_covered_exactly_once() {
        for len in LENGTHS {
            for (dispatch, workers) in cases() {
                let (_, visits, workspaces) = run(dispatch, len, workers);
                assert!(
                    visits.iter().all(|&v| v == 1),
                    "len {len}, {workers} workers"
                );
                let mut covered: Vec<usize> = workspaces
                    .iter()
                    .flat_map(|w| w.ranges.iter().cloned().flatten())
                    .collect();
                covered.sort_unstable();
                assert_eq!(covered, (0..len).collect::<Vec<_>>());
            }
        }
    }

    #[test]
    fn test_each_workspace_runs_its_own_balanced_partition() {
        for len in LENGTHS {
            for (dispatch, workers) in cases() {
                let (_, _, workspaces) = run(dispatch, len, workers);
                let partitions = workers.min(len);
                let mut next = 0;
                for (p, workspace) in workspaces.iter().enumerate() {
                    if p >= partitions {
                        assert!(workspace.ranges.is_empty(), "surplus workspace {p} ran");
                        assert_eq!(workspace.fold, 0);
                        continue;
                    }
                    assert_eq!(workspace.ranges.len(), 1, "workspace {p} runs once");
                    let range = workspace.ranges[0].clone();
                    assert_eq!(range.start, next, "partitions are contiguous in order");
                    assert!(!range.is_empty());
                    assert!(range.len().abs_diff(len / partitions) <= 1);
                    let fold = range.clone().map(value).fold(0, u64::wrapping_add);
                    assert_eq!(workspace.fold, fold, "state stays within its partition");
                    next = range.end;
                }
                assert_eq!(next, len);
            }
        }
    }

    #[test]
    fn test_empty_batch_touches_no_workspace() {
        let mut outputs: [u8; 0] = [];
        let mut workspaces: [u8; 0] = [];
        for_each_partition(
            PartitionDispatch::Pool,
            &mut outputs,
            &mut workspaces,
            |_, _, _| unreachable!(),
        );
        let mut workspaces = [0_u8; 3];
        for_each_partition(
            PartitionDispatch::Pool,
            &mut outputs,
            &mut workspaces,
            |_, _, _| unreachable!(),
        );
    }

    #[test]
    fn test_calling_thread_dispatch_stays_on_the_calling_thread() {
        let caller = std::thread::current().id();
        let mut outputs = vec![None; 65];
        let mut workspaces = [(); 7];
        for_each_partition(
            PartitionDispatch::CallingThread,
            &mut outputs,
            &mut workspaces,
            |(), _, out| out.fill(Some(std::thread::current().id())),
        );
        assert!(outputs.iter().all(|&thread| thread == Some(caller)));
    }

    #[test]
    #[should_panic(expected = "at least one workspace")]
    fn test_non_empty_batch_without_workspaces_panics() {
        let mut outputs = [0_u8; 1];
        let mut workspaces: [u8; 0] = [];
        for_each_partition(
            PartitionDispatch::Pool,
            &mut outputs,
            &mut workspaces,
            |_, _, _| {},
        );
    }

    #[test]
    fn test_partition_offset_balances_to_within_one() {
        for items in [0_usize, 1, 63, 64, 65] {
            for partitions in 1..=9 {
                let bounds: Vec<usize> = (0..=partitions)
                    .map(|p| partition_offset(items, partitions, p))
                    .collect();
                assert_eq!(bounds[0], 0);
                assert_eq!(bounds[partitions], items);
                for pair in bounds.windows(2) {
                    let size = pair[1] - pair[0];
                    assert!(size.abs_diff(items / partitions) <= 1);
                    assert!(size >= items / partitions);
                }
            }
        }
        assert_eq!(partition_offset(10, 3, 1), 4);
        assert_eq!(partition_offset(10, 3, 2), 7);
    }
}
