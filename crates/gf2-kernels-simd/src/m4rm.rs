//! M4RM Gray-code table builders: the portable reference behind
//! [`crate::M4rmGrayBuildFn`] and the argument contract every builder of that
//! type answers.

use crate::M4rmGrayBuildFn;

/// Portable [`M4rmGrayBuildFn`] for any `stride_words`.
///
/// Walks the Gray code, so each entry costs one row copy and at most one row
/// XOR.
///
/// # Panics
///
/// As [`M4rmGrayBuildFn`], for every stride.
pub fn m4rm_gray_build_scalar(
    buffer: &mut [u64],
    panel: &[u64],
    stride_words: usize,
    table_size: usize,
    valid_rows: usize,
) {
    assert_gray_build_lengths(
        "m4rm_gray_build_scalar",
        buffer.len(),
        panel.len(),
        stride_words,
        table_size,
        valid_rows,
    );
    buffer[..stride_words].fill(0);
    let mut prev_gray = 0usize;
    for i in 1..table_size {
        let curr_gray = i ^ (i >> 1);
        let bit_pos = (prev_gray ^ curr_gray).trailing_zeros() as usize;
        buffer.copy_within(
            prev_gray * stride_words..(prev_gray + 1) * stride_words,
            curr_gray * stride_words,
        );
        if bit_pos < valid_rows {
            let entry = &mut buffer[curr_gray * stride_words..(curr_gray + 1) * stride_words];
            let row = &panel[bit_pos * stride_words..(bit_pos + 1) * stride_words];
            for (word, &row_word) in entry.iter_mut().zip(row) {
                *word ^= row_word;
            }
        }
        prev_gray = curr_gray;
    }
}

const _: M4rmGrayBuildFn = m4rm_gray_build_scalar;

/// The argument check every Gray-table builder runs before its first store.
///
/// The Gray index of `i < table_size` stays below `table_size` only for a
/// power of two, and a builder stores entry 0 unconditionally.
///
/// # Panics
///
/// Panics, with a message opening with `builder`, unless `table_size` is a
/// nonzero power of two, neither `table_size * stride_words` nor
/// `valid_rows * stride_words` overflows `usize`, `buffer_len` covers the
/// first product and `panel_len` the second.
pub fn assert_gray_build_lengths(
    builder: &str,
    buffer_len: usize,
    panel_len: usize,
    stride_words: usize,
    table_size: usize,
    valid_rows: usize,
) {
    assert!(
        table_size.is_power_of_two(),
        "{builder}: table_size must be a nonzero power of two"
    );
    let Some(table_words) = table_size.checked_mul(stride_words) else {
        panic!("{builder}: table_size * stride_words overflows usize");
    };
    assert!(buffer_len >= table_words, "{builder}: buffer too small");
    let Some(panel_words) = valid_rows.checked_mul(stride_words) else {
        panic!("{builder}: valid_rows * stride_words overflows usize");
    };
    assert!(panel_len >= panel_words, "{builder}: panel too small");
}

/// The argument contract of a Gray-table builder, as cases any implementation
/// can be handed.
///
/// This module's own tests run [`contract::assert_contract`] over
/// [`contract::kernel_builders`], and
/// `crates/gf2-core/tests/m4rm_gray_build_contract.rs` runs it over the same
/// list and the matrix-level builder of `gf2-core`.
#[cfg(any(test, feature = "test-support"))]
pub mod contract {
    use super::m4rm_gray_build_scalar;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    /// The call shape of [`crate::M4rmGrayBuildFn`], as a closure so an
    /// adapter can stand in for a builder of another signature.
    pub type Build<'a> = &'a dyn Fn(&mut [u64], &[u64], usize, usize, usize);

    /// One Gray-table builder under test.
    pub struct Builder<'a> {
        /// Names the builder and its stride in a report.
        pub label: String,
        /// The name the builder's own panic messages open with.
        pub panic_prefix: &'a str,
        /// The stride the cases run at.
        pub stride_words: usize,
        /// Whether the builder takes `table_size` and `valid_rows` as free
        /// arguments. A builder that derives them from a matrix has a
        /// power-of-two table and a panel of exactly its rows by
        /// construction, and skips the cases that need a free value.
        pub takes_sizes: bool,
        pub build: Build<'a>,
    }

    /// Every builder this crate publishes: the portable reference at strides
    /// on both sides of the specialized widths, and each specialized builder
    /// of the detected bundle. The reference is always present.
    pub fn kernel_builders() -> Vec<Builder<'static>> {
        let mut builders: Vec<Builder<'static>> = [1usize, 4, 5, 8, 16]
            .into_iter()
            .map(|stride_words| Builder {
                label: format!("m4rm_gray_build_scalar, stride {stride_words}"),
                panic_prefix: "m4rm_gray_build_scalar",
                stride_words,
                takes_sizes: true,
                build: &m4rm_gray_build_scalar,
            })
            .collect();
        if let Some(fns) = crate::detect() {
            let fns: &'static crate::LogicalFns = Box::leak(Box::new(fns));
            builders.push(Builder {
                label: "m4rm_gray_build4, stride 4".to_owned(),
                panic_prefix: "m4rm_gray_build4",
                stride_words: 4,
                takes_sizes: true,
                build: &fns.m4rm_gray_build4_fn,
            });
            builders.push(Builder {
                label: "m4rm_gray_build8, stride 8".to_owned(),
                panic_prefix: "m4rm_gray_build8",
                stride_words: 8,
                takes_sizes: true,
                build: &fns.m4rm_gray_build8_fn,
            });
        }
        builders
    }

    const SENTINEL: u64 = 0xA5A5_5A5A_A5A5_5A5A;

    /// Deterministic panel words.
    fn panel_words(rows: usize, stride_words: usize) -> Vec<u64> {
        (0..rows * stride_words)
            .map(|i| (i as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15))
            .collect()
    }

    /// Requires `builder` to panic with its own message containing
    /// `expected`, leaving a sentinel-filled buffer of `buffer_words` words
    /// as it found it.
    fn rejects(
        builder: &Builder<'_>,
        buffer_words: usize,
        panel: &[u64],
        table_size: usize,
        valid_rows: usize,
        expected: &str,
    ) -> Result<(), String> {
        let mut buffer = vec![SENTINEL; buffer_words];
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            (builder.build)(
                &mut buffer,
                panel,
                builder.stride_words,
                table_size,
                valid_rows,
            );
        }));
        let Err(payload) = outcome else {
            return Err(format!(
                "accepted table_size {table_size}, valid_rows {valid_rows}"
            ));
        };
        let message = payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| {
                payload
                    .downcast_ref::<&str>()
                    .map(|text| (*text).to_owned())
            })
            .unwrap_or_default();
        if !(message.starts_with(builder.panic_prefix) && message.contains(expected)) {
            return Err(format!(
                "panicked with {message:?}, expected its own {expected:?}"
            ));
        }
        if buffer.iter().any(|&word| word != SENTINEL) {
            return Err(format!("stored before panicking with {message:?}"));
        }
        Ok(())
    }

    /// Requires entry `g` of a `table_size`-entry table to be the XOR of the
    /// panel rows the bits of `g` select among the first `valid_rows`.
    fn builds(builder: &Builder<'_>, table_size: usize, valid_rows: usize) -> Result<(), String> {
        let stride = builder.stride_words;
        let panel = panel_words(valid_rows, stride);
        let mut buffer = vec![SENTINEL; table_size * stride];
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            (builder.build)(&mut buffer, &panel, stride, table_size, valid_rows);
        }));
        if outcome.is_err() {
            return Err(format!(
                "panicked on table_size {table_size}, valid_rows {valid_rows}"
            ));
        }
        for entry in 0..table_size {
            for word in 0..stride {
                let expected = (0..valid_rows)
                    .filter(|row| entry >> row & 1 == 1)
                    .fold(0u64, |acc, row| acc ^ panel[row * stride + word]);
                if buffer[entry * stride + word] != expected {
                    return Err(format!(
                        "entry {entry} word {word} with {valid_rows} rows of {table_size}"
                    ));
                }
            }
        }
        Ok(())
    }

    /// Every case of the contract with its outcome for `builder`, in a fixed
    /// order.
    ///
    /// The rejection cases require a panic before any store, with the
    /// builder's own message: a zero table size; table sizes that are not a
    /// power of two, in a buffer that holds the next power of two entries so
    /// a builder that accepts them stays inside it; table and panel lengths
    /// whose product with the stride overflows `usize`; a buffer one word
    /// short; a panel one word short. The remaining cases require the
    /// documented table for a one-entry table and for every row count of a
    /// 32-entry table.
    pub fn cases(builder: &Builder<'_>) -> Vec<(&'static str, Result<(), String>)> {
        let stride = builder.stride_words;
        let power = "table_size must be a nonzero power of two";
        let mut outcomes = Vec::new();
        if builder.takes_sizes {
            outcomes.push((
                "zero table size",
                rejects(builder, stride, &panel_words(1, stride), 0, 1, power),
            ));
            let rejected = [3usize, 5, 6, 7, 12]
                .into_iter()
                .try_for_each(|table_size| {
                    let words = table_size.next_power_of_two() * stride;
                    rejects(
                        builder,
                        words,
                        &panel_words(4, stride),
                        table_size,
                        4,
                        power,
                    )
                });
            outcomes.push(("table size not a power of two", rejected));
        }
        if let Some(table_size) = (usize::MAX / stride)
            .checked_add(1)
            .and_then(usize::checked_next_power_of_two)
        {
            outcomes.push((
                "overflowing table length",
                rejects(
                    builder,
                    stride,
                    &panel_words(1, stride),
                    table_size,
                    1,
                    "table_size * stride_words overflows usize",
                ),
            ));
        }
        if builder.takes_sizes && stride > 1 {
            outcomes.push((
                "overflowing panel length",
                rejects(
                    builder,
                    2 * stride,
                    &panel_words(1, stride),
                    2,
                    usize::MAX / stride + 1,
                    "valid_rows * stride_words overflows usize",
                ),
            ));
        }
        outcomes.push((
            "buffer one word short",
            rejects(
                builder,
                4 * stride - 1,
                &panel_words(2, stride),
                4,
                2,
                "buffer too small",
            ),
        ));
        if builder.takes_sizes {
            let mut panel = panel_words(3, stride);
            panel.pop();
            outcomes.push((
                "panel one word short",
                rejects(builder, 8 * stride, &panel, 8, 3, "panel too small"),
            ));
        }
        outcomes.push(("one-entry table", builds(builder, 1, 0)));
        outcomes.push((
            "32-entry tables",
            (0..=5usize).try_for_each(|valid_rows| builds(builder, 32, valid_rows)),
        ));
        outcomes
    }

    /// Runs [`cases`] over `builders`, prints one line per builder and case,
    /// and panics after the report when a case failed.
    ///
    /// # Panics
    ///
    /// Panics when `builders` is empty or a case failed.
    pub fn assert_contract(builders: &[Builder<'_>]) {
        assert!(!builders.is_empty(), "no Gray-table builder was enumerated");
        let mut failures = 0usize;
        for builder in builders {
            for (case, outcome) in cases(builder) {
                match outcome {
                    Ok(()) => println!("ok      {}: {case}", builder.label),
                    Err(reason) => {
                        failures += 1;
                        println!("FAILED  {}: {case}: {reason}", builder.label);
                    }
                }
            }
        }
        assert_eq!(failures, 0, "Gray-table contract cases failed");
    }
}

#[cfg(test)]
mod tests {
    use super::contract::{assert_contract, kernel_builders};

    #[test]
    fn every_published_builder_answers_the_gray_build_contract() {
        assert_contract(&kernel_builders());
    }
}
