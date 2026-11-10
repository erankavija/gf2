//! A fast-bench worker's timed call allocates nothing after its first call,
//! on every route and on batches up to `STACK_FRAMES`, so pinned workers
//! share no allocator traffic while they are timed.

use aff3ct_ldpc_gaps::batch::{
    config, layered_worker, qc_worker, ArmPolicy, BatchRoute, BatchWorker, LaneBackend,
    STACK_FRAMES,
};
use gf2_coding::ldpc::{DecodedWord, LdpcBatchDecoder, LdpcCode, QuasiCyclicLdpc};
use thread_allocations::{count, CountingAllocator};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

const CAP: usize = 6;

fn llrs(code: &LdpcCode, batch: usize) -> Vec<f32> {
    (0..batch * code.n())
        .map(|i| if i % 7 == 0 { -0.5 } else { 1.5 })
        .collect()
}

fn assert_second_call_allocates_nothing(name: &str, mut call: impl FnMut()) {
    call();
    let ((), counts) = count(&mut call);
    assert!(counts.is_zero(), "{name}: {counts:?}");
}

#[test]
fn timed_calls_allocate_nothing_after_the_first() {
    let decoder_config = config(0.75, true);
    let base = vec![vec![0, 3, 7, 1, 0, -1], vec![5, 0, 11, -1, 2, 9]];
    let code = LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::new(base, 16));
    for batch in [1, 16, STACK_FRAMES] {
        let input = llrs(&code, batch);
        for route in [
            BatchRoute::SingleFrame,
            BatchRoute::Lane(LaneBackend::Portable),
            BatchRoute::Lane(LaneBackend::detected()),
        ] {
            let decoder = route
                .build_decoder(&code, decoder_config, CAP, batch)
                .expect("route");
            let mut worker = BatchWorker::from_decoder(&code, batch, decoder).expect("worker");
            assert_second_call_allocates_nothing(&format!("{route:?} batch {batch}"), || {
                worker.decode_batch(&input);
            });
        }
        let default =
            LdpcBatchDecoder::new(code.clone(), decoder_config, CAP, DecodedWord::Codeword);
        let mut worker = BatchWorker::from_decoder(&code, batch, default).expect("worker");
        assert_second_call_allocates_nothing(&format!("default batch {batch}"), || {
            worker.decode_batch(&input);
        });
        let mut worker = qc_worker(&code, decoder_config, CAP, batch, false).expect("QC");
        assert_second_call_allocates_nothing(&format!("qc batch {batch}"), || {
            worker.decode_batch(&input);
        });
    }

    let nr = LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(1, 384));
    let input = llrs(&nr, 2);
    let mut worker = layered_worker(&nr, "nr-bg1-r12", 2, false).expect("layered");
    assert_second_call_allocates_nothing("layered", || worker.decode_batch(&input));
}
