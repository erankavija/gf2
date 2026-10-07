//! Fixed-duration profile driver for the measured logical-buffer routes.
//!
//! `perf` wraps one invocation per profile case, so its counters belong to one
//! route at one frozen cell. The driver observes its own call count rather than
//! carrying a per-case call constant, which would be a prior figure about this
//! host. It is the campaign arm's sibling: same crate, same fixtures and the
//! same public entry points.

use logical_buffer_harness::cells::{cells, Cache, Cell, Workload};
use logical_buffer_harness::fixture::{RowBanks, XorBanks};
use logical_buffer_harness::routes::{
    backend_name, nr_construct, public_row_xor, public_xor, resolved_xor, resolved_xor_apply,
    verify_nr, Route,
};
use logical_buffer_harness::wire::require_window_unless_child;
use std::hint::black_box;
use std::time::{Duration, Instant};

/// The frozen profile matrix: one cell of the addendum and the route observed
/// on it. The isolated resolved rows are the addendum's attribution rows, so
/// they appear at the two smallest anchors only.
const PROFILE_CASES: [(&str, &str); 14] = [
    ("xor-8w-a64-warm", "public-xor-a"),
    ("xor-9w-a64-warm", "public-xor-a"),
    ("xor-64w-a64-warm", "public-xor-a"),
    ("xor-8w-o8-warm", "public-xor-a"),
    ("xor-64w-a64-streaming", "public-xor-a"),
    ("xor-8w-a64-warm", "resolved-xor"),
    ("xor-9w-a64-warm", "resolved-xor"),
    ("row-xor-8w-full-warm", "row-xor-a"),
    ("row-xor-9w-full-warm", "row-xor-a"),
    ("row-xor-64w-full-warm", "row-xor-a"),
    ("row-xor-8w-tail63-warm", "row-xor-a"),
    ("row-xor-64w-full-streaming", "row-xor-a"),
    ("nr-construct-bg2-256-49-z9-8w-warm", "nr-construct-a"),
    ("nr-construct-bg2-1024-441-z56-46w-warm", "nr-construct-a"),
];

/// Longest chunk the driver runs before it consults the clock again.
const MAX_CHUNK: u64 = 1 << 22;

fn main() {
    if let Err(error) = run() {
        eprintln!("logical-profile: {error}");
        std::process::exit(2);
    }
}

fn case_id(cell_id: &str, route: &str) -> String {
    format!("{cell_id}@{route}")
}

fn argument<'a>(arguments: &'a [String], name: &str) -> Option<&'a str> {
    arguments
        .iter()
        .position(|value| value == name)
        .and_then(|index| arguments.get(index + 1))
        .map(String::as_str)
}

fn run() -> Result<(), String> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match arguments.first().map(String::as_str) {
        Some("cases") => {
            for (cell_id, route) in PROFILE_CASES {
                println!("{}", case_id(cell_id, route));
            }
            Ok(())
        }
        Some("run") => {
            require_window_unless_child()?;
            let wanted = argument(&arguments, "--case").ok_or("run needs --case")?;
            let seconds: f64 = argument(&arguments, "--seconds")
                .ok_or("run needs --seconds")?
                .parse()
                .map_err(|error| format!("--seconds is not a number: {error}"))?;
            let (cell_id, route) = PROFILE_CASES
                .iter()
                .find(|(cell_id, route)| case_id(cell_id, route) == wanted)
                .ok_or_else(|| format!("{wanted} is not a frozen profile case"))?;
            measure(
                cell_id,
                Route::parse(route)?,
                Duration::from_secs_f64(seconds),
            )
        }
        _ => Err(
            "usage: logical-profile cases | logical-profile run --case <id> --seconds <n>".into(),
        ),
    }
}

/// Runs `body` until `target` elapses, growing the chunk while a chunk is
/// short enough that the clock reading would dominate it.
fn drive(
    target: Duration,
    body: &mut dyn FnMut(usize, usize),
    banks: usize,
    items: usize,
) -> (u64, u64) {
    let mut calls = 0_u64;
    let mut chunk = 1_u64;
    let started = Instant::now();
    while started.elapsed() < target {
        let chunk_started = Instant::now();
        for _ in 0..chunk {
            let bank = (calls as usize) % banks;
            let item = (calls / banks as u64) as usize % items;
            body(bank, item);
            calls += 1;
        }
        if chunk_started.elapsed() < Duration::from_millis(10) && chunk < MAX_CHUNK {
            chunk *= 2;
        }
    }
    let elapsed = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
    (calls, elapsed)
}

fn frozen_cell(cell_id: &str) -> Result<Cell, String> {
    cells()
        .into_iter()
        .find(|cell| cell.cell_id == cell_id)
        .ok_or_else(|| format!("{cell_id} is not a frozen cell"))
}

fn measure(cell_id: &str, route: Route, target: Duration) -> Result<(), String> {
    let cell = frozen_cell(cell_id)?;
    let cache = cell.cache;
    let seed = cell.seed;
    let mut sink = 0_u64;
    let (selected, calls, elapsed) = match cell.workload {
        Workload::Xor { words, layout } => {
            let mut banks = XorBanks::build(words, layout, cache, seed);
            if cache == Cache::Streaming {
                sink ^= banks.touch();
            }
            let (source, destination) = banks.addresses_mod_64(0, 0);
            let hoisted = (route == Route::ResolvedXor).then(|| resolved_xor(words));
            let selected = format!(
                "gf2-core/kernels::ops::{}/{}/w={words}/layout={}/src%64={source}/dst%64={destination}/working-set={}B",
                if hoisted.is_some() {
                    "resolve_xor_inplace"
                } else {
                    "xor_inplace"
                },
                backend_name(words),
                layout.id(),
                banks.working_set_bytes()
            );
            let items = banks.items();
            if cache == Cache::Warm {
                let (destination, source) = banks.pair(0, 0);
                public_xor(destination, source);
            }
            let mut body = |bank: usize, item: usize| {
                let (destination, source) = banks.pair(bank, item);
                match hoisted {
                    Some(resolved) => resolved_xor_apply(resolved, destination, source),
                    None => public_xor(destination, source),
                }
            };
            let (calls, elapsed) = drive(target, &mut body, cache.banks(), items);
            (selected, calls, elapsed)
        }
        Workload::RowXor { words, shape } => {
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
            if cache == Cache::Warm {
                public_row_xor(banks.matrix_mut(0, 0), 0);
            }
            let mut index = 0_usize;
            let mut body = |bank: usize, item: usize| {
                public_row_xor(banks.matrix_mut(bank, item), index);
                index = index.wrapping_add(1);
            };
            let (calls, elapsed) = drive(target, &mut body, cache.banks(), items);
            (selected, calls, elapsed)
        }
        Workload::Nr(target_code) => {
            let facts = verify_nr(&nr_construct(&target_code), &target_code)?;
            let selected = format!(
                "gf2-coding/QuasiCyclicLdpc::nr_5g_rate_matched({},{},{})/{}/Z={}/dense={}x{}/stride={}w/nnz={}/h-sha256={}",
                target_code.base_graph,
                target_code.target_n,
                target_code.target_k,
                backend_name(facts.stride_words),
                facts.lifting_factor,
                facts.dense_rows,
                facts.dense_cols,
                facts.stride_words,
                facts.nnz,
                facts.structure_digest
            );
            let mut body = |_: usize, _: usize| {
                black_box(nr_construct(&target_code));
            };
            let (calls, elapsed) = drive(target, &mut body, 1, 1);
            (selected, calls, elapsed)
        }
        Workload::IsalGap { .. } => {
            return Err(format!("{cell_id} is not a gf2 profile workload"));
        }
    };
    black_box(sink);
    println!(
        "{}",
        serde_json::json!({
            "schema": "logical-buffer-profile-case-v1",
            "case": case_id(cell_id, route.id()),
            "cell": cell_id,
            "route": route.id(),
            "cache_state": cache.id(),
            "selected_path": selected,
            "calls": calls,
            "elapsed_ns": elapsed,
        })
    );
    Ok(())
}
