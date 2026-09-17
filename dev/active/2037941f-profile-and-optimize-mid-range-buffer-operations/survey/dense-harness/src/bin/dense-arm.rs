//! Protocol-v4 gf2 campaign arm for the dense-parity questions (jit:e1f9a78f).
//!
//! One executable serves every gf2 route of the frozen addendum; the campaign
//! plan selects a route through `GF2_DENSE_ROUTE`. The executable is built
//! `conservative-portable`: Rust 1.95 release with the workspace's ordinary
//! feature configuration and no `target-cpu` override. The addendum's reference
//! arm is the same executable built without the `simd` feature, and a route
//! refuses the build that cannot serve it.

use dense_parity_harness::cells::{Cache, Workload};
use dense_parity_harness::fixture::{KernelBanks, MatvecBanks};
use dense_parity_harness::routes::{
    fused_and_popcnt, fused_bundle, observe_output, public_matvec, run_windows, verify_lane,
    verify_shape, OutputSink, Route, WindowPlan,
};
use dense_parity_harness::wire::{
    read_request, require_window_unless_child, ArmResult, ConversionCosts, Request,
};
use std::cell::{Cell as MutCell, RefCell};
use std::hint::black_box;
use std::time::Instant;
use tuning_campaign_support::timing::{TimingProgress, TimingSample};

fn main() {
    if let Err(error) = run() {
        eprintln!("dense-arm: {error}");
        std::process::exit(2);
    }
}

fn elapsed_ns(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

fn run() -> Result<(), String> {
    require_window_unless_child()?;
    let (request, case, cache) = read_request()?;
    let route = Route::from_environment()?;
    route.check_build()?;
    let workload = case.workload()?;
    if route.question() != workload.question() {
        return Err(format!("{} does not serve a {:?} cell", route.id(), workload));
    }
    let seed = case.seed();
    let mut sink = 0_u64;
    let setup_start = Instant::now();

    let (selected_path, samples, setup_ns) = match workload {
        Workload::AndPopcnt { words } => {
            let bundle = fused_bundle()
                .ok_or("the host detected no kernel bundle, so the fused entry is unavailable")?;
            let banks = KernelBanks::build(words, cache, seed);
            if cache == Cache::Streaming {
                sink ^= banks.touch();
            }
            let (row_addr, vector_addr) = banks.addresses_mod_64(0, 0);
            let selected = format!(
                "gf2-kernels-simd/LogicalFns::and_popcnt_fn/indirect/w={words}\
                 /row%64={row_addr}/x%64={vector_addr}/working-set={}B",
                banks.working_set_bytes()
            );
            let counted = MutCell::new(0_u64);
            let items = banks.items();
            let mut body = |bank: usize, item: usize| {
                let (row, vector) = banks.operands(bank, item);
                counted.set(counted.get() ^ fused_and_popcnt(&bundle, row, vector));
            };
            let setup_ns = elapsed_ns(setup_start);
            let samples = windows(&request, cache, cache.banks(), items, &mut body, |_| Ok(()))?;
            sink ^= counted.get();
            (selected, samples, setup_ns)
        }
        Workload::Matvec { words, shape } => {
            let banks = MatvecBanks::allocated(words, shape, cache, seed);
            if cache == Cache::Streaming {
                sink ^= banks.touch();
            }
            let facts = verify_shape(
                &banks.item(0, 0).matrix,
                &banks.item(0, 0).vector,
                banks.rows(),
                banks.columns(),
            )?;
            verify_lane(facts, route)?;
            // The allocated boundary charges the output allocation and excludes
            // its release, so each call's output is retained and the batch is
            // released after the window closes.
            let retained = RefCell::new(OutputSink::default());
            retained.borrow_mut().reserve(request.cold_calls.unwrap_or(0));
            let selected = format!(
                "gf2-core/BitMatrix::matvec/{}/stride={words}w/shape={}/rows={}/cols={}\
                 /base%64={}/retain={}/working-set={}B",
                facts.lane(),
                shape.id(),
                banks.rows(),
                banks.columns(),
                banks.base_mod_64(0, 0),
                retained.borrow().capacity(),
                banks.working_set_bytes()
            );
            let items = banks.items();
            let mut body = |bank: usize, item: usize| {
                let item = banks.item(bank, item);
                let output = public_matvec(&item.matrix, &item.vector);
                retained.borrow_mut().keep(output);
            };
            let setup_ns = elapsed_ns(setup_start);
            let samples = windows(&request, cache, cache.banks(), items, &mut body, |progress| {
                let mut retained = retained.borrow_mut();
                match progress {
                    TimingProgress::CalibrationComplete { calls } => retained.reserve(calls),
                    TimingProgress::WindowComplete(_) => retained.release(),
                }
                Ok(())
            })?;
            retained.borrow_mut().release();
            (selected, samples, setup_ns)
        }
        Workload::M4riGap { shape, .. } => {
            let banks = MatvecBanks::build(shape.rows, shape.cols, cache, seed);
            let facts = verify_shape(
                &banks.item(0, 0).matrix,
                &banks.item(0, 0).vector,
                shape.rows,
                shape.cols,
            )?;
            verify_lane(facts, route)?;
            let selected = format!(
                "gf2-core/BitMatrix::matvec+release/{}/stride={}w/rows={}/cols={}\
                 /base%64={}/working-set={}B",
                facts.lane(),
                facts.stride_words,
                shape.rows,
                shape.cols,
                banks.base_mod_64(0, 0),
                banks.working_set_bytes()
            );
            // The comparator boundary charges the release, so the output is
            // dropped at the end of every measured call.
            let observed = MutCell::new(0_u64);
            let items = banks.items();
            let mut body = |bank: usize, item: usize| {
                let item = banks.item(bank, item);
                let output = public_matvec(&item.matrix, &item.vector);
                observed.set(observed.get() ^ observe_output(&output));
            };
            let setup_ns = elapsed_ns(setup_start);
            let samples = windows(&request, cache, cache.banks(), items, &mut body, |_| Ok(()))?;
            sink ^= observed.get();
            (selected, samples, setup_ns)
        }
    };
    black_box(sink);
    ArmResult::new(
        &samples,
        cache,
        selected_path,
        ConversionCosts {
            setup_ns,
            pack_ns: 0,
            unpack_ns: 0,
            batch_fill_ns: 0,
            dispatch_ns: 0,
        },
    )
    .emit()
    .map_err(|error| error.to_string())
}

fn windows(
    request: &Request,
    cache: Cache,
    banks: usize,
    items: usize,
    body: &mut dyn FnMut(usize, usize),
    progress: impl FnMut(TimingProgress) -> std::io::Result<()>,
) -> Result<Vec<TimingSample>, String> {
    run_windows(
        WindowPlan {
            cache,
            cold_calls: request.cold_calls,
            windows: request.windows,
            window_target_ms: request.window_target_ms,
            banks,
            items,
        },
        body,
        progress,
    )
    .map_err(|error| format!("timing failed: {error}"))
}
