//! Cached GF(2^8) byte product tables and the region kernel that reads them.
//!
//! One table holds the product of every byte pair under one degree-8
//! reduction polynomial, keyed by that polynomial's low eight bits. A table
//! occupies 65536 bytes; the registry holds one per key used, at most 16 MiB,
//! and never evicts.

use std::sync::OnceLock;

/// Products of one fixed coefficient with every byte, indexed by that byte.
pub(crate) type ProductRow = [u8; 256];

/// Registry slots: one per degree-8 reduction polynomial.
const REGISTRY_SLOTS: usize = 256;

/// Entries in one product table: every ordered byte pair.
const TABLE_ENTRIES: usize = 256 * 256;

/// Elements [`axpy_region`] handles per unrolled step.
pub(crate) const AXPY_UNROLL: usize = 8;

/// The products of every ordered byte pair under one degree-8 reduction
/// polynomial.
///
/// `entries[256 * c + v]` is `c * v` reduced by the polynomial the table was
/// built for. Immutable after construction and shareable across every worker.
pub(crate) struct Gf256ProductTable {
    entries: [u8; TABLE_ENTRIES],
}

impl Gf256ProductTable {
    /// Builds the whole table for the reduction polynomial whose low eight
    /// bits are `reduction_low`.
    ///
    /// Each row is the doubling recurrence over that polynomial: `c * 0` is
    /// zero, `c * 1` is `c`, an even multiple is [`xtime`] of half of it, and
    /// an odd multiple is the even one below it XOR `c`.
    fn build(reduction_low: u8) -> Self {
        let mut entries = [0u8; TABLE_ENTRIES];
        for coefficient in 0..256usize {
            let base = coefficient << 8;
            entries[base + 1] = coefficient as u8;
            for value in 2..256usize {
                entries[base + value] = if value % 2 == 0 {
                    xtime(entries[base + value / 2], reduction_low)
                } else {
                    entries[base + value - 1] ^ entries[base + 1]
                };
            }
        }
        Self { entries }
    }

    /// Returns the 256 products of `coefficient` with every byte.
    pub(crate) fn row(&self, coefficient: u8) -> &ProductRow {
        let start = usize::from(coefficient) << 8;
        self.entries[start..start + 256]
            .try_into()
            .expect("a 256-byte window of a 65536-byte table")
    }
}

/// Multiplies by `x` in GF(2^8) under the modulus whose low eight bits are
/// `reduction_low`.
#[inline]
fn xtime(value: u8, reduction_low: u8) -> u8 {
    let doubled = value << 1;
    if value & 0x80 != 0 {
        doubled ^ reduction_low
    } else {
        doubled
    }
}

/// One lazily initialised table per degree-8 reduction polynomial.
static TABLES: [OnceLock<Gf256ProductTable>; REGISTRY_SLOTS] =
    [const { OnceLock::new() }; REGISTRY_SLOTS];

/// Returns the process-wide table for the reduction polynomial whose low eight
/// bits are `reduction_low`, building it on the first touch of that key.
///
/// The first caller for a key builds the table, concurrent callers for the
/// same key block until it is published, and every later caller takes the
/// published reference with no lock.
pub(crate) fn product_table(reduction_low: u8) -> &'static Gf256ProductTable {
    TABLES[usize::from(reduction_low)].get_or_init(|| {
        record_table_build();
        Gf256ProductTable::build(reduction_low)
    })
}

/// XORs the products of one coefficient row into `y`, reading `x` of the same length.
///
/// `source_byte` reads the GF(2^8) value out of a source element and
/// `accumulate` XORs one product into a destination element, so the same
/// kernel serves the runtime-context element and the compile-time-configured
/// wide value without either representation packing its operands into bytes.
/// The kernel allocates nothing and reads `x` while writing `y`, which the
/// caller's borrows keep disjoint.
pub(crate) fn axpy_region<Y, X>(
    y: &mut [Y],
    x: &[X],
    row: &ProductRow,
    source_byte: impl Fn(&X) -> u8,
    accumulate: impl Fn(&mut Y, u8),
) {
    let mut destinations = y.chunks_exact_mut(AXPY_UNROLL);
    let mut sources = x.chunks_exact(AXPY_UNROLL);
    for (destination, source) in destinations.by_ref().zip(sources.by_ref()) {
        for lane in 0..AXPY_UNROLL {
            accumulate(
                &mut destination[lane],
                row[usize::from(source_byte(&source[lane]))],
            );
        }
    }
    for (destination, source) in destinations
        .into_remainder()
        .iter_mut()
        .zip(sources.remainder())
    {
        accumulate(destination, row[usize::from(source_byte(source))]);
    }
}

/// Shape of one dense product: an `m × k` left operand by a `k × n` right
/// operand.
pub(crate) struct GemmShape {
    /// Rows of the left operand and of the output.
    pub(crate) m: usize,
    /// Inner dimension summed over.
    pub(crate) k: usize,
    /// Columns of the right operand and of the output.
    pub(crate) n: usize,
}

/// Writes the product of `a` and the operand `b_t` holds transposed into
/// `out`.
///
/// `b_t` is the right operand in `n × k` row-major order, the form the
/// whole-product hook receives; restoring it to `k × n` is what lets one
/// left-hand coefficient drive a whole output row through [`axpy_region`], and
/// costs `O(kn)` byte moves inside an `O(mkn)` kernel. `byte_of` reads the
/// GF(2^8) value out of an operand element and `store` writes one result byte
/// into a destination element, so the same traversal serves the
/// runtime-context element and the compile-time-configured wide value.
///
/// # Arguments
///
/// * `a` — left operand, `m × k` row-major, `m * k` elements.
/// * `b_t` — transposed right operand, `n × k` row-major, `n * k` elements.
/// * `out` — destination, `m × n` row-major, overwritten rather than
///   accumulated into.
///
/// # Complexity
///
/// `O(mkn)` indexed loads and XORs over three byte buffers totalling
/// `mk + kn + mn` bytes.
pub(crate) fn gemm_region<T>(
    a: &[T],
    b_t: &[T],
    shape: &GemmShape,
    out: &mut [T],
    table: &Gf256ProductTable,
    byte_of: impl Fn(&T) -> u8,
    store: impl Fn(&mut T, u8),
) {
    let GemmShape { m, k, n } = *shape;
    debug_assert_eq!(a.len(), m * k);
    debug_assert_eq!(b_t.len(), n * k);
    debug_assert_eq!(out.len(), m * n);
    if m == 0 || n == 0 {
        return;
    }

    let left: Vec<u8> = a.iter().map(&byte_of).collect();
    let mut right = vec![0u8; k * n];
    for (p, right_row) in right.chunks_exact_mut(n).enumerate() {
        for (j, slot) in right_row.iter_mut().enumerate() {
            *slot = byte_of(&b_t[j * k + p]);
        }
    }

    let mut products = vec![0u8; m * n];
    for (i, product_row) in products.chunks_exact_mut(n).enumerate() {
        for (p, right_row) in right.chunks_exact(n).enumerate() {
            axpy_region(
                product_row,
                right_row,
                table.row(left[i * k + p]),
                |byte| *byte,
                |slot, product| *slot ^= product,
            );
        }
    }

    for (slot, &product) in out.iter_mut().zip(products.iter()) {
        store(slot, product);
    }
}

/// Name the GF(2^8) table dispatch reports when it runs the cached product
/// table.
pub const GF256_TABLE_LANE: &str = "gf256-product-table";

/// Name the GF(2^8) table dispatch reports when it declines, leaving the
/// caller on the route it takes without the table.
pub const GF256_SCALAR_LANE: &str = "gf256-table-declined";

/// Selects between the cached-table lane and the consumer's own scalar path
/// for one GF(2^8) operation.
///
/// # Dispatch predicate
///
/// This is the authoritative statement of when the table lane runs; every
/// other mention of this dispatch cites it by name rather than restate it.
///
/// The table lane runs when *all* of the following hold:
///
/// 1. `degree` is 8, so every field value is one byte.
/// 2. `single_u64_word` — the representation holds that value in a single
///    `u64`-backed word, which is `V::IS_U64` for the runtime-context element
///    and `N == 1` for the wide value.
/// 3. `shared_field_context` returns `true`. The runtime-context element
///    passes a pointer comparison of every operand's field handle against the
///    coefficient's, which preserves the scalar path's field-context assertion
///    semantics; the compile-time-configured wide value carries its field in
///    its type and passes a constant `true`. It is evaluated only after the
///    two cheap guards above pass, so a declined call pays nothing for it.
/// 4. The test-only force switch is clear. [`force_scalar_gf256_table`] sets
///    it, and outside `test` and `test-support` builds it does not exist.
///
/// No processor feature and no cargo feature takes part: the table lane is
/// safe scalar Rust in this crate and calls no kernel crate, so `simd` neither
/// enables nor disables it. Every other degree, every other backing width,
/// every multi-word configuration, a mixed field context and a forced scalar
/// lane all decline, and the consumer runs the element loop it runs without
/// this dispatch.
///
/// Either lane records itself through the witness
/// [`last_gf256_table_lane`] reads.
pub(crate) fn gf256_table_dispatch(
    degree: usize,
    single_u64_word: bool,
    reduction_low: u8,
    shared_field_context: impl FnOnce() -> bool,
) -> Option<&'static Gf256ProductTable> {
    if degree == 8 && single_u64_word && gf256_table_lane_enabled() && shared_field_context() {
        record_gf256_table_lane(GF256_TABLE_LANE);
        Some(product_table(reduction_low))
    } else {
        record_gf256_table_lane(GF256_SCALAR_LANE);
        None
    }
}

/// Records the lane the most recent GF(2^8) table dispatch selected.
///
/// Compiled away outside `test` and `test-support` builds, where the recorder
/// is an empty inlined function.
#[cfg(any(test, feature = "test-support"))]
#[inline]
fn record_gf256_table_lane(lane: &'static str) {
    LAST_GF256_TABLE_LANE.with(|cell| cell.set(lane));
}

#[cfg(not(any(test, feature = "test-support")))]
#[inline(always)]
fn record_gf256_table_lane(_lane: &'static str) {}

#[cfg(any(test, feature = "test-support"))]
std::thread_local! {
    /// Lane of the most recent GF(2^8) table dispatch on this thread.
    static LAST_GF256_TABLE_LANE: std::cell::Cell<&'static str> =
        const { std::cell::Cell::new(GF256_SCALAR_LANE) };
}

/// The lane the most recent GF(2^8) table dispatch took on this thread:
/// [`GF256_TABLE_LANE`] or [`GF256_SCALAR_LANE`].
///
/// The witness the conformance suite reads to prove which path a public call
/// took. A thread that has dispatched nothing yet reports
/// [`GF256_SCALAR_LANE`].
#[cfg(any(test, feature = "test-support"))]
pub fn last_gf256_table_lane() -> &'static str {
    LAST_GF256_TABLE_LANE.with(std::cell::Cell::get)
}

#[cfg(any(test, feature = "test-support"))]
static TABLE_BUILDS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[cfg(any(test, feature = "test-support"))]
#[inline]
fn record_table_build() {
    TABLE_BUILDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(not(any(test, feature = "test-support")))]
#[inline(always)]
fn record_table_build() {}

/// How many product tables this process has built.
///
/// The witness that a key is built at most once: the counter rises by one the
/// first time a key is touched and stands still on every later lookup of it.
#[cfg(any(test, feature = "test-support"))]
pub fn gf256_table_builds() -> usize {
    TABLE_BUILDS.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(any(test, feature = "test-support"))]
static FORCE_SCALAR_GF256_TABLE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Holds every GF(2^8) caller on its own scalar path, or releases it back to
/// `gf256_table_dispatch`, and reports the previous setting.
///
/// One switch covers every caller of that dispatch, so the scalar fallback
/// stays reachable under test on a host where the table lane would run. Both
/// lanes compute the same bytes, so a concurrent call that observes the switch
/// writes the same values either way.
#[cfg(any(test, feature = "test-support"))]
pub fn force_scalar_gf256_table(forced: bool) -> bool {
    FORCE_SCALAR_GF256_TABLE.swap(forced, std::sync::atomic::Ordering::Relaxed)
}

/// Whether [`gf256_table_dispatch`] is free to select the table lane.
#[cfg(any(test, feature = "test-support"))]
#[inline]
fn gf256_table_lane_enabled() -> bool {
    !FORCE_SCALAR_GF256_TABLE.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(not(any(test, feature = "test-support")))]
#[inline(always)]
fn gf256_table_lane_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gf2m::Gf2mField;

    /// Independent schoolbook oracle: the carry-less product of two bytes,
    /// reduced by the degree-8 modulus bit by bit.
    ///
    /// Shares no code with [`Gf256ProductTable::build`]'s doubling recurrence
    /// and names no arithmetic from this crate.
    fn schoolbook_product(c: u8, v: u8, reduction_low: u8) -> u8 {
        let mut product: u16 = 0;
        for bit in 0..8 {
            if (v >> bit) & 1 == 1 {
                product ^= u16::from(c) << bit;
            }
        }
        let modulus: u16 = 0x100 | u16::from(reduction_low);
        for degree in (8..16).rev() {
            if product & (1u16 << degree) != 0 {
                product ^= modulus << (degree - 8);
            }
        }
        product as u8
    }

    /// The low eight bits of every irreducible degree-8 modulus, taken from
    /// the crate's own Rabin irreducibility test rather than a copied list.
    fn irreducible_keys() -> Vec<u8> {
        (0u16..256)
            .filter(|low| Gf2mField::new(8, 0x100 | u64::from(*low)).is_irreducible_rabin())
            .map(|low| low as u8)
            .collect()
    }

    #[test]
    fn the_crate_reports_thirty_irreducible_degree_8_moduli() {
        // Monic irreducible polynomials of degree 8 over GF(2): Mobius inversion
        // of 2^n = sum over d | n of d * N(d) gives (2^8 - 2^4) / 8.
        let expected = (256 - 16) / 8;
        assert_eq!(irreducible_keys().len(), expected);
    }

    #[test]
    fn every_entry_of_every_irreducible_modulus_matches_the_schoolbook_oracle() {
        for key in irreducible_keys() {
            let table = product_table(key);
            for c in 0..=255u8 {
                let row = table.row(c);
                for v in 0..=255u8 {
                    assert_eq!(
                        row[usize::from(v)],
                        schoolbook_product(c, v, key),
                        "product {c} * {v} under the modulus with low bits {key:#04x}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_zero_coefficient_gives_an_all_zero_row_and_one_gives_the_identity() {
        for key in irreducible_keys() {
            let table = product_table(key);
            assert!(table.row(0).iter().all(|&product| product == 0));
            for v in 0..=255u8 {
                assert_eq!(table.row(1)[usize::from(v)], v);
            }
        }
    }

    #[test]
    fn a_second_lookup_of_a_key_returns_the_same_table_and_builds_nothing() {
        let key = 0x1d;
        let first = product_table(key);
        let builds_after_first = gf256_table_builds();
        let second = product_table(key);
        assert!(std::ptr::eq(first, second));
        assert_eq!(gf256_table_builds(), builds_after_first);

        let other = product_table(0x1b);
        assert!(!std::ptr::eq(first, other));
    }

    #[test]
    fn concurrent_first_touches_of_one_key_observe_one_table() {
        // A key no other test in this module touches, so the threads below
        // race on its construction.
        let key = 0x2b;
        let observed: Vec<&'static Gf256ProductTable> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..16)
                .map(|_| scope.spawn(move || product_table(key)))
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().expect("a table lookup does not panic"))
                .collect()
        });

        let first = observed[0];
        assert!(observed.iter().all(|table| std::ptr::eq(*table, first)));
        for c in 0..=255u8 {
            for v in 0..=255u8 {
                assert_eq!(first.row(c)[usize::from(v)], schoolbook_product(c, v, key));
            }
        }
    }

    #[test]
    fn the_region_kernel_accumulates_products_at_every_byte_offset() {
        let key = 0x1d;
        let row = product_table(key).row(0x57);
        for length in [0usize, 1, 63, 64, 65, 137] {
            for destination_offset in 0..=7usize {
                for source_offset in 0..=7usize {
                    let source: Vec<u8> = (0..length + source_offset)
                        .map(|i| (i * 7 + 3) as u8)
                        .collect();
                    let initial: Vec<u8> = (0..length + destination_offset)
                        .map(|i| (i * 13 + 5) as u8)
                        .collect();
                    let mut destination = initial.clone();

                    axpy_region(
                        &mut destination[destination_offset..],
                        &source[source_offset..],
                        row,
                        |byte| *byte,
                        |slot, product| *slot ^= product,
                    );

                    assert_eq!(
                        &destination[..destination_offset],
                        &initial[..destination_offset],
                        "the kernel wrote before its destination window"
                    );
                    for i in 0..length {
                        assert_eq!(
                            destination[destination_offset + i],
                            initial[destination_offset + i]
                                ^ schoolbook_product(0x57, source[source_offset + i], key),
                            "length {length}, offsets {destination_offset}/{source_offset}, index {i}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_dispatch_declines_every_configuration_outside_single_word_gf256() {
        assert!(gf256_table_dispatch(8, true, 0x1d, || true).is_some());
        assert_eq!(last_gf256_table_lane(), GF256_TABLE_LANE);

        for (degree, single_word, shared) in [
            (4usize, true, true),
            (16, true, true),
            (8, false, true),
            (8, true, false),
        ] {
            assert!(gf256_table_dispatch(degree, single_word, 0x1d, || shared).is_none());
            assert_eq!(last_gf256_table_lane(), GF256_SCALAR_LANE);
        }
    }

    #[test]
    fn the_field_context_check_runs_only_after_the_cheap_guards_pass() {
        let mut evaluated = false;
        assert!(gf256_table_dispatch(16, true, 0x1d, || {
            evaluated = true;
            true
        })
        .is_none());
        assert!(!evaluated);
    }
}
