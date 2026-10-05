//! Argument contract of every implementation behind
//! [`gf2_kernels_simd::M4rmGrayBuildFn`]: an invalid table size or an
//! overflowing length product panics before any store, and a valid call
//! builds the table its type documents.

use gf2_kernels_simd::M4rmGrayBuildFn;
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Each builder the detected bundle publishes, with the stride it specializes
/// for. Empty where the host has no bundle.
fn builders() -> Vec<(&'static str, M4rmGrayBuildFn, usize)> {
    gf2_kernels_simd::detect().map_or_else(Vec::new, |fns| {
        vec![
            ("m4rm_gray_build4", fns.m4rm_gray_build4_fn, 4),
            ("m4rm_gray_build8", fns.m4rm_gray_build8_fn, 8),
        ]
    })
}

/// The panic message of `build` on these arguments. `buffer_entries` and
/// `panel_rows` size the two buffers, so a call the builder wrongly accepts
/// stays inside them.
fn rejection(
    build: M4rmGrayBuildFn,
    stride: usize,
    buffer_entries: usize,
    panel_rows: usize,
    table_size: usize,
    valid_rows: usize,
) -> Option<String> {
    let mut buffer = vec![0u64; buffer_entries * stride];
    let panel = vec![0u64; panel_rows * stride];
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        build(&mut buffer, &panel, stride, table_size, valid_rows);
    }));
    outcome.err().map(|payload| {
        payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| {
                payload
                    .downcast_ref::<&str>()
                    .map(|text| (*text).to_owned())
            })
            .unwrap_or_default()
    })
}

#[track_caller]
fn assert_rejected(name: &str, message: Option<String>, expected: &str) {
    let message = message.unwrap_or_else(|| panic!("{name} accepted the arguments"));
    assert!(
        message.contains(name) && message.contains(expected),
        "{name} panicked with {message:?}, expected its own {expected:?}"
    );
}

#[test]
fn a_zero_table_size_is_rejected() {
    for (name, build, stride) in builders() {
        let message = rejection(build, stride, 1, 1, 0, 1);
        assert_rejected(name, message, "table_size must be a nonzero power of two");
    }
}

#[test]
fn a_table_size_that_is_not_a_power_of_two_is_rejected() {
    // The Gray index of `i < table_size` reaches the next power of two, so
    // the buffer holds that many entries.
    for (name, build, stride) in builders() {
        for table_size in [3usize, 5, 6, 7, 12] {
            let entries = table_size.next_power_of_two();
            let message = rejection(build, stride, entries, 4, table_size, 4);
            assert_rejected(name, message, "table_size must be a nonzero power of two");
        }
    }
}

#[test]
fn an_overflowing_table_length_is_rejected() {
    for (name, build, stride) in builders() {
        let table_size = usize::MAX / stride + 1;
        assert!(table_size.is_power_of_two());
        let message = rejection(build, stride, 1, 1, table_size, 1);
        assert_rejected(name, message, "table_size * stride_words overflows usize");
    }
}

#[test]
fn an_overflowing_panel_length_is_rejected() {
    for (name, build, stride) in builders() {
        let valid_rows = usize::MAX / stride + 1;
        let message = rejection(build, stride, 2, 1, 2, valid_rows);
        assert_rejected(name, message, "valid_rows * stride_words overflows usize");
    }
}

#[test]
fn a_one_entry_table_is_the_zero_entry() {
    for (name, build, stride) in builders() {
        let mut buffer = vec![u64::MAX; stride];
        build(&mut buffer, &[], stride, 1, 0);
        assert!(buffer.iter().all(|&word| word == 0), "{name}");
    }
}

#[test]
fn each_entry_is_the_xor_of_the_panel_rows_its_index_selects() {
    for (name, build, stride) in builders() {
        for valid_rows in 0..=5usize {
            let table_size = 1usize << 5;
            let panel: Vec<u64> = (0..valid_rows * stride)
                .map(|i| (i as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15))
                .collect();
            let mut buffer = vec![u64::MAX; table_size * stride];
            build(&mut buffer, &panel, stride, table_size, valid_rows);
            for entry in 0..table_size {
                for word in 0..stride {
                    let expected = (0..valid_rows)
                        .filter(|row| entry >> row & 1 == 1)
                        .fold(0u64, |acc, row| acc ^ panel[row * stride + word]);
                    assert_eq!(
                        buffer[entry * stride + word],
                        expected,
                        "{name}: entry {entry} word {word} with {valid_rows} rows"
                    );
                }
            }
        }
    }
}
