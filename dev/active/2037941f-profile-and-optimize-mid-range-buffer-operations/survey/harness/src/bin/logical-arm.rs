//! Protocol-v4 gf2 campaign arm for the logical-buffer questions (jit:bb769456).
//!
//! One executable serves every gf2 route of the frozen addendum; the campaign
//! plan selects a route through `GF2_LOGICAL_ROUTE`. The executable is built
//! `conservative-portable`: Rust 1.95 release with the workspace's ordinary
//! feature configuration and no `target-cpu` override.

use logical_buffer_harness::cells::{Cache, Layout, Workload};
use logical_buffer_harness::fixture::{RowBanks, XorBanks};
use logical_buffer_harness::routes::{
    backend_name, fresh_destination, gap_gf2_operation, nr_construct, public_row_xor, public_xor,
    resolved_xor, resolved_xor_apply, run_windows, verify_nr, Route, WindowPlan,
};
use logical_buffer_harness::wire::{
    read_request, require_window_unless_child, ArmResult, ConversionCosts,
};
use std::hint::black_box;
use std::time::Instant;

fn main() {
    if let Err(error) = run() {
        eprintln!("logical-arm: {error}");
        std::process::exit(2);
    }
}

fn elapsed_ns(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

/// One separately measured fresh-destination arrangement, averaged over many
/// repetitions so a few-nanosecond cost rounds to an observed value.
fn destination_probe_ns(words: usize) -> u64 {
    const REPS: u32 = 100_000;
    let started = Instant::now();
    for _ in 0..REPS {
        let mut destination = fresh_destination(words);
        black_box(destination.as_mut_slice().as_mut_ptr());
    }
    (elapsed_ns(started) + u64::from(REPS) / 2) / u64::from(REPS)
}

fn run() -> Result<(), String> {
    require_window_unless_child()?;
    let (request, case, cache) = read_request()?;
    let route = Route::from_environment()?;
    let workload = case.workload()?;
    let seed = case.seed();
    let mut sink = 0_u64;
    let mut dispatch_ns = 0_u64;
    let setup_start = Instant::now();

    let (selected_path, samples, setup_ns) = match workload {
        Workload::Xor { words, layout } => {
            if route.question() != logical_buffer_harness::cells::Question::IsolatedXor {
                return Err(format!(
                    "{} does not serve an isolated-XOR cell",
                    route.id()
                ));
            }
            let mut banks = XorBanks::build(words, layout, cache, seed);
            if cache == Cache::Streaming {
                sink ^= banks.touch();
            }
            let (source, destination) = banks.addresses_mod_64(0, 0);
            let selected = format!(
                "gf2-core/kernels::ops::{}/{}/w={words}/layout={}/src%64={source}/dst%64={destination}/working-set={}B",
                if route == Route::ResolvedXor {
                    "resolve_xor_inplace"
                } else {
                    "xor_inplace"
                },
                backend_name(words),
                layout.id(),
                banks.working_set_bytes()
            );
            let hoisted = (route == Route::ResolvedXor).then(|| resolved_xor(words));
            let items = banks.items();
            let mut body = |bank: usize, item: usize| {
                let (destination, source) = banks.pair(bank, item);
                match hoisted {
                    Some(resolved) => resolved_xor_apply(resolved, destination, source),
                    None => public_xor(destination, source),
                }
            };
            let setup_ns = elapsed_ns(setup_start);
            let samples = windows(&request, cache, cache.banks(), items, &mut body)?;
            (selected, samples, setup_ns)
        }
        Workload::RowXor { words, shape } => {
            if route.question() != logical_buffer_harness::cells::Question::PublicRowXor {
                return Err(format!("{} does not serve a row-XOR cell", route.id()));
            }
            let mut banks = RowBanks::build(words, shape, cache, seed);
            if cache == Cache::Streaming {
                sink ^= banks.touch();
            }
            let pairs = banks
                .pair_addresses_mod_64(0, 0)
                .iter()
                .map(|(dst, src)| format!("{dst}:{src}"))
                .collect::<Vec<_>>()
                .join(",");
            let selected = format!(
                "gf2-core/BitMatrix::row_xor/{}/stride={words}w/shape={}/base%64={}/dst:src%64={pairs}/working-set={}B",
                backend_name(words),
                shape.id(),
                banks.base_mod_64(0, 0),
                banks.working_set_bytes()
            );
            let items = banks.items();
            let mut index = 0_usize;
            let mut body = |bank: usize, item: usize| {
                public_row_xor(banks.matrix_mut(bank, item), index);
                index = index.wrapping_add(1);
            };
            let setup_ns = elapsed_ns(setup_start);
            let samples = windows(&request, cache, cache.banks(), items, &mut body)?;
            (selected, samples, setup_ns)
        }
        Workload::Nr(target) => {
            if route.question() != logical_buffer_harness::cells::Question::NrConstruction {
                return Err(format!("{} does not serve an NR cell", route.id()));
            }
            // Every stated lifting factor, dimension and stride is verified
            // from the returned public object before timing is enabled.
            let facts = verify_nr(&nr_construct(&target), &target)?;
            let selected = format!(
                "gf2-coding/QuasiCyclicLdpc::nr_5g_rate_matched({},{},{})/{}/Z={}/dense={}x{}/stride={}w/nnz={}/h-sha256={}",
                target.base_graph,
                target.target_n,
                target.target_k,
                backend_name(facts.stride_words),
                facts.lifting_factor,
                facts.dense_rows,
                facts.dense_cols,
                facts.stride_words,
                facts.nnz,
                facts.structure_digest
            );
            let mut body = |_: usize, _: usize| {
                black_box(nr_construct(&target));
            };
            let setup_ns = elapsed_ns(setup_start);
            let samples = windows(&request, cache, 1, 1, &mut body)?;
            (selected, samples, setup_ns)
        }
        Workload::IsalGap { words } => {
            if route != Route::IsalPeerGf2 {
                return Err(format!("{} does not serve an ISA-L gap cell", route.id()));
            }
            let banks = XorBanks::build(words, Layout::A64, cache, seed);
            if cache == Cache::Streaming {
                sink ^= banks.touch();
            }
            let (first, second) = banks.addresses_mod_64(0, 0);
            let selected = format!(
                "gf2-core/kernels::ops::xor_inplace/{}/fresh-destination+copy/w={words}/src0%64={first}/src1%64={second}/working-set={}B",
                backend_name(words),
                banks.working_set_bytes()
            );
            dispatch_ns = destination_probe_ns(words);
            let items = banks.items();
            let observed = std::cell::Cell::new(0_u64);
            let mut body = |bank: usize, item: usize| {
                let (source0, source1) = banks.sources(bank, item);
                observed.set(observed.get() ^ gap_gf2_operation(source0, source1));
            };
            let setup_ns = elapsed_ns(setup_start);
            let samples = windows(&request, cache, cache.banks(), items, &mut body)?;
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
            dispatch_ns,
        },
    )
    .emit()
    .map_err(|error| error.to_string())
}

fn windows(
    request: &logical_buffer_harness::wire::Request,
    cache: Cache,
    banks: usize,
    items: usize,
    body: &mut dyn FnMut(usize, usize),
) -> Result<Vec<tuning_campaign_support::timing::TimingSample>, String> {
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
        |_| Ok(()),
    )
    .map_err(|error| format!("timing failed: {error}"))
}
