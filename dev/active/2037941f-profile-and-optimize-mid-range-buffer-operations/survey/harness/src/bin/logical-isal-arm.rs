//! Protocol-v4 external comparator arm: ISA-L `xor_gen_base/scalar`.
//!
//! `raid.h` documents `xor_gen_base(vects, len, array)` as writing
//! `array[vects - 1]` with the XOR of `array[0..vects - 2]`, with 32-byte
//! aligned source and destination pointers. This arm links only that scalar
//! reference, compiled from the pinned checkout by `build.rs`. The NASM-built
//! multibinary `xor_gen` is unavailable on the qualifying host; admitting it
//! requires a newly qualified comparator and a versioned amendment.
//!
//! `--backend` reports the linked symbol and that availability. `--oracle`
//! runs the comparator's semantic cases against the gf2 peer. With no
//! argument the executable is a child-v2 campaign arm.

use logical_buffer_harness::cells::{Cache, Layout, Workload, ALL_WORDS, CAMPAIGN_SEED};
use logical_buffer_harness::fixture::{AlignedSlab, XorBanks};
use logical_buffer_harness::oracle::OracleCase;
use logical_buffer_harness::routes::{fresh_destination, observe_output, run_windows, WindowPlan};
use logical_buffer_harness::wire::{
    read_request, require_window_unless_child, ArmResult, ConversionCosts,
};
use std::hint::black_box;
use std::time::Instant;

/// Alignment the ISA-L XOR interface requires of every pointer it receives.
const ISAL_ALIGN: usize = 32;
/// Sources every cell of this addendum supplies.
const SOURCES: usize = 2;

unsafe extern "C" {
    /// ISA-L's portable scalar XOR reference, from `raid/raid_base.c`.
    fn xor_gen_base(vects: i32, len: i32, array: *mut *mut core::ffi::c_void) -> i32;
}

/// One ISA-L call: the pointer array is formed inside the call, the fresh
/// destination is written directly, and the output is observed.
#[inline]
fn isal_operation(source0: &[u64], source1: &[u64]) -> Result<u64, String> {
    let words = source0.len();
    let mut destination = fresh_destination(words);
    let status = {
        let target = destination.as_mut_slice();
        let mut array: [*mut core::ffi::c_void; SOURCES + 1] = [
            source0.as_ptr() as *mut core::ffi::c_void,
            source1.as_ptr() as *mut core::ffi::c_void,
            target.as_mut_ptr().cast(),
        ];
        // SAFETY: `array` holds exactly `SOURCES + 1` valid pointers in ISA-L's
        // source-then-destination order, each to `words * 8` readable bytes
        // (writable for the destination) that outlive the call, all 64-byte
        // aligned and therefore 32-byte aligned as `raid.h` requires. The
        // sources and the fresh destination do not overlap.
        unsafe { xor_gen_base((SOURCES + 1) as i32, (words * 8) as i32, array.as_mut_ptr()) }
    };
    if status != 0 {
        return Err(format!("xor_gen_base returned {status}"));
    }
    Ok(observe_output(destination.as_mut_slice()))
}

/// One separately measured arrangement: fresh destination plus pointer array.
///
/// A timed execution charges this cost; the non-timed arrangement pass of a
/// zero-window request runs no probe and reports zero.
fn arrangement_probe_ns(words: usize, source0: &[u64], source1: &[u64]) -> u64 {
    const REPS: u32 = 100_000;
    let started = Instant::now();
    for _ in 0..REPS {
        let mut destination = fresh_destination(words);
        let target = destination.as_mut_slice();
        let mut array: [*mut core::ffi::c_void; SOURCES + 1] = [
            source0.as_ptr() as *mut core::ffi::c_void,
            source1.as_ptr() as *mut core::ffi::c_void,
            target.as_mut_ptr().cast(),
        ];
        black_box(array.as_mut_ptr());
    }
    let elapsed = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
    (elapsed + u64::from(REPS) / 2) / u64::from(REPS)
}

fn backend_record() -> String {
    format!(
        "{{\"library\":\"isa-l\",\"entrypoint\":\"xor_gen_base\",\"qualified_label\":\
         \"xor_gen_base/scalar\",\"runtime_dispatch\":false,\"public_xor_gen_available\":false,\
         \"unavailable_row\":\"isal-dispatched-xor-gen\",\"unavailable_reason\":\"NASM is absent \
         on the qualifying host, so raid_multibinary.asm and the SSE/AVX/AVX-512 xor_gen objects \
         have no reproduced executable or observed dispatch route\",\"comparator\":\"{}\"}}",
        logical_buffer_harness::cells::COMPARATOR_PATH
    )
}

/// The comparator's semantic cases against the gf2 peer.
fn oracle() -> Result<Vec<OracleCase>, String> {
    let mut report = Vec::new();
    for words in ALL_WORDS {
        let name = format!("isal-{words}w");
        let seed = CAMPAIGN_SEED ^ ((words as u64) << 24);
        let banks = XorBanks::build(words, Layout::A64, Cache::Warm, seed);
        let (source0, source1) = banks.sources(0, 0);
        let mut checks = 0;
        let (first, second) = banks.addresses_mod_64(0, 0);
        if !first.is_multiple_of(ISAL_ALIGN) || !second.is_multiple_of(ISAL_ALIGN) {
            return Err(format!("FAIL {name}: a source is not 32-byte aligned"));
        }
        checks += 2;

        // A poisoned fresh destination: every written word must come from the
        // call, not from the allocation.
        let mut destination = AlignedSlab::zeroed(words);
        for word in destination.as_mut_slice().iter_mut() {
            *word = 0xA5A5_A5A5_A5A5_A5A5;
        }
        if !destination.base_addr().is_multiple_of(ISAL_ALIGN) {
            return Err(format!(
                "FAIL {name}: the destination is not 32-byte aligned"
            ));
        }
        checks += 1;
        let status = {
            let target = destination.as_mut_slice();
            let mut array: [*mut core::ffi::c_void; SOURCES + 1] = [
                source0.as_ptr() as *mut core::ffi::c_void,
                source1.as_ptr() as *mut core::ffi::c_void,
                target.as_mut_ptr().cast(),
            ];
            // SAFETY: as in `isal_operation`; the three pointers are valid,
            // non-overlapping, 32-byte aligned and live for the call.
            unsafe { xor_gen_base((SOURCES + 1) as i32, (words * 8) as i32, array.as_mut_ptr()) }
        };
        if status != 0 {
            return Err(format!("FAIL {name}: xor_gen_base returned {status}"));
        }
        checks += 1;

        let observed = destination.as_slice();
        for index in 0..words {
            if observed[index] != source0[index] ^ source1[index] {
                return Err(format!(
                    "FAIL {name}: word {index} differs from the gf2 peer"
                ));
            }
            checks += 1;
        }
        for index in 0..words * 64 {
            let got = (observed[index >> 6] >> (index & 63)) & 1;
            let want = ((source0[index >> 6] >> (index & 63)) & 1)
                ^ ((source1[index >> 6] >> (index & 63)) & 1);
            if got != want {
                return Err(format!("FAIL {name}: bit {index} is not LSB-first XOR"));
            }
            checks += 1;
        }

        // Pointer-array order: swapping a source for the destination slot would
        // change the result, so the qualified order is checked by rebuilding
        // the same result through the gf2 peer.
        let peer = logical_buffer_harness::routes::gap_gf2_operation(source0, source1);
        if peer != observe_output(observed) {
            return Err(format!(
                "FAIL {name}: the observed output differs from the gf2 peer"
            ));
        }
        checks += 1;

        let fresh = XorBanks::build(words, Layout::A64, Cache::Warm, seed);
        let (again0, again1) = fresh.sources(0, 0);
        if again0 != source0 || again1 != source1 {
            return Err(format!("FAIL {name}: a source changed"));
        }
        checks += 2;
        report.push(OracleCase { name, checks });
    }
    Ok(report)
}

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match arguments.as_slice() {
        [] => run(),
        [flag] if flag == "--backend" => {
            println!("{}", backend_record());
            Ok(())
        }
        [flag] if flag == "--oracle" => oracle().map(|report| {
            for case in report {
                println!("{case}");
            }
        }),
        _ => Err("usage: logical-isal-arm [--backend | --oracle]".to_owned()),
    };
    if let Err(error) = outcome {
        eprintln!("logical-isal-arm: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    require_window_unless_child()?;
    let (request, case, cache) = read_request()?;
    let Workload::IsalGap { words } = case.workload()? else {
        return Err("the ISA-L comparator arm serves only isal-base-gap cells".into());
    };
    let setup_start = Instant::now();
    let banks = XorBanks::build(words, Layout::A64, cache, case.seed());
    let mut sink = 0_u64;
    if cache == Cache::Streaming {
        sink ^= banks.touch();
    }
    let (first, second) = banks.addresses_mod_64(0, 0);
    if !first.is_multiple_of(ISAL_ALIGN) || !second.is_multiple_of(ISAL_ALIGN) {
        return Err("ISA-L cells use only the aligned layout".into());
    }
    let selected_path = format!(
        "isa-l/xor_gen_base/scalar/vects={}/len={}B/src0%64={first}/src1%64={second}/working-set={}B",
        SOURCES + 1,
        words * 8,
        banks.working_set_bytes()
    );
    let (probe0, probe1) = banks.sources(0, 0);
    let dispatch_ns = if request.windows > 0 {
        arrangement_probe_ns(words, probe0, probe1)
    } else {
        0
    };
    let items = banks.items();
    let failure = std::cell::RefCell::new(None);
    let observed = std::cell::Cell::new(0_u64);
    let mut body = |bank: usize, item: usize| {
        let (source0, source1) = banks.sources(bank, item);
        match isal_operation(source0, source1) {
            Ok(value) => observed.set(observed.get() ^ value),
            Err(error) => *failure.borrow_mut() = Some(error),
        }
    };
    let setup_ns = u64::try_from(setup_start.elapsed().as_nanos()).unwrap_or(u64::MAX);
    let samples = run_windows(
        WindowPlan {
            cache,
            cold_calls: request.cold_calls,
            windows: request.windows,
            window_target_ms: request.window_target_ms,
            banks: cache.banks(),
            items,
        },
        &mut body,
        |_| Ok(()),
    )
    .map_err(|error| format!("timing failed: {error}"))?;
    if let Some(error) = failure.borrow().clone() {
        return Err(error);
    }
    sink ^= observed.get();
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
