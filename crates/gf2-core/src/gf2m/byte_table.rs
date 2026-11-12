//! Cached GF(2^8) split product tables and the byte-region routes that read
//! them. One table set holds, for every constant `c`, the products of `c` with
//! the sixteen low and the sixteen high nibbles under one degree-8 reduction
//! polynomial, keyed by that polynomial's low eight bits: 8192 bytes per key,
//! at most 2 MiB over all keys, never evicted. The kernels of
//! `gf2_kernels_simd::gf256_region`, selected by [`crate::gf256_region`],
//! multiply through them.

use gf2_kernels_simd::gf256_region::{GemmBlock, SplitTable};
use std::num::NonZeroUsize;
use std::sync::OnceLock;

/// The split table of every constant under one reduction polynomial.
pub(crate) type Gf256SplitTables = [SplitTable; 256];

/// Registry slots: one per degree-8 reduction polynomial.
const REGISTRY_SLOTS: usize = 256;

/// Cache blocks of the GF(2^8) byte matrix product: each block reads
/// `depth` rows of the right operand over `width` of its columns, for every
/// row of the left operand, and a right operand given transposed is restored
/// to row order in square tiles of `tile` bytes a side.
///
/// [`Gf256GemmBlocking::structural`] sizes them from the host's caches and
/// the dispatched kernel's strip; [`crate::gf2m::Gf2mField::gf256_gemm_with`]
/// and [`crate::gf2m::Gf2mField::gf256_gemm_transposed_with`] take a
/// caller's blocking instead. Every blocking gives the same product.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gf256GemmBlocking {
    depth: NonZeroUsize,
    width: NonZeroUsize,
    tile: NonZeroUsize,
}

impl Gf256GemmBlocking {
    /// Blocks of `depth` inner indices by `width` columns, and transpose
    /// tiles of `tile` by `tile` bytes.
    #[must_use]
    pub const fn new(depth: NonZeroUsize, width: NonZeroUsize, tile: NonZeroUsize) -> Self {
        Self { depth, width, tile }
    }

    /// The blocking for this host's per-core caches and the dispatched block
    /// kernel.
    ///
    /// One kernel step reads a strip of the block's columns down all `depth`
    /// rows, one cache line per row, and the next strip of the same lines
    /// follows; so `depth` is half the first-level data cache in lines,
    /// leaving the other half to the 8 KiB table set and the left operand's
    /// rows. Every row group of the left operand reads the whole block, so
    /// `width` holds the block to half the second-level cache, rounded down to
    /// whole strips. A transpose tile reads a cache line from each of `tile`
    /// source rows and writes one to each of `tile` destination rows, so
    /// `tile` is one line, shrunk until both sides' lines fit in half the
    /// first-level data cache. On Linux the cache and line sizes are those of CPU 0's
    /// sysfs cache description; elsewhere, or when it is unreadable, 32 KiB,
    /// 256 KiB and 64-byte lines, the per-core sizes of Haswell, the first AVX2
    /// x86-64 core.
    /// With 32 KiB and 512 KiB caches and 64-byte lines, as on a Ryzen 9
    /// 5900X, the blocks are 256 by 1024 and the tiles 64 bytes a side.
    #[must_use]
    pub fn structural() -> Self {
        static STRUCTURAL: OnceLock<Gf256GemmBlocking> = OnceLock::new();
        *STRUCTURAL.get_or_init(|| {
            Self::for_caches(HostCaches::detect(), crate::gf256_region().gemm_strip)
        })
    }

    /// The derivation [`Self::structural`] states, for `caches` and a kernel
    /// strip of `strip` columns.
    fn for_caches(caches: HostCaches, strip: usize) -> Self {
        let strip = strip.max(1);
        let depth = (caches.l1d / (2 * caches.line.max(1))).max(1);
        let width = (caches.l2 / (2 * depth) / strip * strip).max(strip);
        let line = caches.line.max(1);
        let tile = line.min(caches.l1d / (4 * line)).max(1);
        Self::new(
            NonZeroUsize::new(depth).expect("a positive depth"),
            NonZeroUsize::new(width).expect("a positive width"),
            NonZeroUsize::new(tile).expect("a positive tile"),
        )
    }

    /// Returns the inner indices of one block.
    #[must_use]
    pub const fn depth(self) -> usize {
        self.depth.get()
    }

    /// Returns the columns of one block.
    #[must_use]
    pub const fn width(self) -> usize {
        self.width.get()
    }

    /// Returns the side of one transpose tile in bytes.
    #[must_use]
    pub const fn tile(self) -> usize {
        self.tile.get()
    }
}

/// Per-core cache sizes in bytes.
#[derive(Clone, Copy, Debug)]
struct HostCaches {
    l1d: usize,
    l2: usize,
    line: usize,
}

impl HostCaches {
    const FALLBACK: Self = Self {
        l1d: 32 << 10,
        l2: 256 << 10,
        line: 64,
    };

    /// CPU 0's first-level data and second-level caches, each field that
    /// cannot be read taken from [`Self::FALLBACK`].
    fn detect() -> Self {
        let mut caches = Self::FALLBACK;
        let Ok(entries) = std::fs::read_dir("/sys/devices/system/cpu/cpu0/cache") else {
            return caches;
        };
        for entry in entries.flatten() {
            let read = |name: &str| {
                std::fs::read_to_string(entry.path().join(name))
                    .map(|text| text.trim().to_owned())
                    .ok()
            };
            let Some(size) = read("size").as_deref().and_then(parse_cache_size) else {
                continue;
            };
            match (read("level").as_deref(), read("type").as_deref()) {
                (Some("1"), Some("Data")) => {
                    caches.l1d = size;
                    if let Some(line) = read("coherency_line_size").and_then(|l| l.parse().ok()) {
                        caches.line = line;
                    }
                }
                (Some("2"), Some("Unified" | "Data")) => caches.l2 = size,
                _ => {}
            }
        }
        caches
    }
}

/// Bytes of a sysfs cache size such as `32K` or `1M`.
fn parse_cache_size(text: &str) -> Option<usize> {
    let (digits, scale) = match text.as_bytes().last()? {
        b'K' => (&text[..text.len() - 1], 1 << 10),
        b'M' => (&text[..text.len() - 1], 1 << 20),
        _ => (text, 1),
    };
    digits.parse::<usize>().ok()?.checked_mul(scale)
}

/// Builds the split tables for the reduction polynomial whose low eight bits
/// are `reduction_low`.
///
/// Each constant's 256 products follow the doubling recurrence over that
/// polynomial: `c * 0` is zero, `c * 1` is `c`, an even multiple is [`xtime`]
/// of half of it, and an odd multiple is the even one below it XOR `c`.
fn build(reduction_low: u8) -> Gf256SplitTables {
    let mut tables = [SplitTable {
        low: [0; 16],
        high: [0; 16],
    }; 256];
    for (coefficient, table) in tables.iter_mut().enumerate() {
        let mut row = [0u8; 256];
        row[1] = coefficient as u8;
        for value in 2..256usize {
            row[value] = if value % 2 == 0 {
                xtime(row[value / 2], reduction_low)
            } else {
                row[value - 1] ^ row[1]
            };
        }
        for nibble in 0..16 {
            table.low[nibble] = row[nibble];
            table.high[nibble] = row[nibble << 4];
        }
    }
    tables
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

/// One lazily initialised table set per degree-8 reduction polynomial.
static TABLES: [OnceLock<Gf256SplitTables>; REGISTRY_SLOTS] =
    [const { OnceLock::new() }; REGISTRY_SLOTS];

/// Returns the process-wide table set for the reduction polynomial whose low
/// eight bits are `reduction_low`, building it on the first touch of that key.
///
/// The first caller for a key builds the tables, concurrent callers for the
/// same key block until they are published, and every later caller takes the
/// published reference with no lock.
pub(crate) fn split_tables(reduction_low: u8) -> &'static Gf256SplitTables {
    TABLES[usize::from(reduction_low)].get_or_init(|| {
        record_table_build();
        build(reduction_low)
    })
}

/// XORs `table`'s products of the sources of `x` into the destinations of
/// `y`, of the same length, in one pass.
///
/// `source_byte` reads the GF(2^8) value out of a source element and
/// `accumulate` XORs one product into a destination element, so the same
/// route serves the runtime-context element and the compile-time-configured
/// wide value. Elements of 8 or 16 bytes leave nothing for the byte kernel to
/// gain over one pass: copying the values into bytes and back costs more than
/// the products, so the route expands the constant's split table into its 256
/// products once and looks each one up. Allocates nothing.
pub(crate) fn axpy_elements<Y, X>(
    y: &mut [Y],
    x: &[X],
    table: &SplitTable,
    source_byte: impl Fn(&X) -> u8,
    accumulate: impl Fn(&mut Y, u8),
) {
    debug_assert_eq!(y.len(), x.len());
    let row: [u8; 256] = core::array::from_fn(|value| table.apply(value as u8));
    for (destination, source) in y.iter_mut().zip(x) {
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

/// `c = a · b` over bytes, `a` `m × k`, `b` `k × n` and `c` `m × n`, all
/// row-major, overwriting `c`.
///
/// The product runs in the blocks of `blocking`, each through the dispatched
/// block kernel, so a block of the right operand is read from cache by every
/// row of the left one.
pub(crate) fn gemm_bytes(
    tables: &Gf256SplitTables,
    shape: &GemmShape,
    blocking: Gf256GemmBlocking,
    a: &[u8],
    b: &[u8],
    c: &mut [u8],
) {
    let GemmShape { m, k, n } = *shape;
    debug_assert_eq!((a.len(), b.len(), c.len()), (m * k, k * n, m * n));
    c.fill(0);
    // An empty operand has no block to address past its first.
    if m == 0 || k == 0 || n == 0 {
        return;
    }
    let fns = crate::gf256_region();
    let strip = fns.gemm_strip;
    // Whole-strip rows and blocks preserve an aligned base. Copy once so
    // every row group reuses loads that do not straddle cache lines.
    let mut packed = Vec::new();
    let b = if n.is_multiple_of(strip)
        && blocking.width().is_multiple_of(strip)
        && b.as_ptr().align_offset(strip) != 0
    {
        packed.resize(b.len() + strip - 1, 0);
        let offset = packed.as_ptr().align_offset(strip);
        packed[offset..offset + b.len()].copy_from_slice(b);
        &packed[offset..offset + b.len()]
    } else {
        b
    };
    let kernel = fns.gemm_block;
    for depth_start in (0..k).step_by(blocking.depth()) {
        let depth = blocking.depth().min(k - depth_start);
        for column in (0..n).step_by(blocking.width()) {
            let width = blocking.width().min(n - column);
            let block = GemmBlock {
                tables,
                a: &a[depth_start..],
                a_stride: k,
                b: &b[depth_start * n + column..],
                b_stride: n,
                rows: m,
                depth,
                width,
            };
            kernel(&block, &mut c[column..], n);
        }
    }
}

/// [`gemm_bytes`] with the right operand given transposed, `n × k`
/// row-major, which [`untranspose`] restores in `blocking`'s tiles.
pub(crate) fn gemm_transposed_bytes(
    tables: &Gf256SplitTables,
    shape: &GemmShape,
    blocking: Gf256GemmBlocking,
    a: &[u8],
    b_t: &[u8],
    c: &mut [u8],
) {
    let b = untranspose(b_t, shape.k, shape.n, blocking.tile(), |byte| *byte);
    gemm_bytes(tables, shape, blocking, a, &b, c);
}

/// Writes the product of `a` and the operand `b_t` holds transposed into
/// `out`, through [`gemm_bytes`].
///
/// `b_t` is the right operand in `n × k` row-major order, the form the
/// whole-product hook receives; restoring it to `k × n` lets the block kernel
/// read rows of the right operand, and costs `O(kn)` byte moves inside an
/// `O(mkn)` product. `byte_of` reads the GF(2^8) value out of an operand
/// element and `store` writes one result byte into a destination element, so
/// the same traversal serves the runtime-context element and the
/// compile-time-configured wide value.
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
/// `O(mkn)` table lookups over three byte buffers totalling `mk + kn + mn`
/// bytes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn gemm_region<T>(
    a: &[T],
    b_t: &[T],
    shape: &GemmShape,
    out: &mut [T],
    tables: &Gf256SplitTables,
    blocking: Gf256GemmBlocking,
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
    let right = untranspose(b_t, k, n, blocking.tile(), byte_of);
    let mut products = vec![0u8; m * n];
    gemm_bytes(tables, shape, blocking, &left, &right, &mut products);

    for (slot, &product) in out.iter_mut().zip(products.iter()) {
        store(slot, product);
    }
}

/// The `k × n` row-major bytes of the operand `b_t` holds in `n × k`
/// row-major order, moved in square tiles of `tile` a side so both sides of
/// a tile stay in cache.
pub(crate) fn untranspose<T>(
    b_t: &[T],
    k: usize,
    n: usize,
    tile: usize,
    byte_of: impl Fn(&T) -> u8,
) -> Vec<u8> {
    debug_assert_eq!(b_t.len(), n * k);
    let tile = tile.max(1);
    let mut right = vec![0u8; k * n];
    for j0 in (0..n).step_by(tile) {
        for p0 in (0..k).step_by(tile) {
            for j in j0..(j0 + tile).min(n) {
                for p in p0..(p0 + tile).min(k) {
                    right[p * n + j] = byte_of(&b_t[j * k + p]);
                }
            }
        }
    }
    right
}

/// Name the GF(2^8) table dispatch reports when it runs the byte-region
/// kernels over the cached split tables.
pub const GF256_TABLE_LANE: &str = "gf256-split-table";

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
) -> Option<&'static Gf256SplitTables> {
    if degree == 8 && single_u64_word && gf256_table_lane_enabled() && shared_field_context() {
        record_gf256_table_lane(GF256_TABLE_LANE);
        Some(split_tables(reduction_low))
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

/// How many split table sets this process has built.
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
    /// Shares no code with [`build`]'s doubling recurrence
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
            let tables = split_tables(key);
            for c in 0..=255u8 {
                for v in 0..=255u8 {
                    assert_eq!(
                        tables[usize::from(c)].apply(v),
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
            let tables = split_tables(key);
            for v in 0..=255u8 {
                assert_eq!(tables[0].apply(v), 0);
                assert_eq!(tables[1].apply(v), v);
            }
        }
    }

    #[test]
    fn a_second_lookup_of_a_key_returns_the_same_table_and_builds_nothing() {
        let key = 0x1d;
        let first = split_tables(key);
        let builds_after_first = gf256_table_builds();
        let second = split_tables(key);
        assert!(std::ptr::eq(first, second));
        assert_eq!(gf256_table_builds(), builds_after_first);

        let other = split_tables(0x1b);
        assert!(!std::ptr::eq(first, other));
    }

    #[test]
    fn concurrent_first_touches_of_one_key_observe_one_table() {
        // A key no other test in this module touches, so the threads below
        // race on its construction.
        let key = 0x2b;
        let observed: Vec<&'static Gf256SplitTables> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..16)
                .map(|_| scope.spawn(move || split_tables(key)))
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
                assert_eq!(
                    first[usize::from(c)].apply(v),
                    schoolbook_product(c, v, key)
                );
            }
        }
    }

    #[test]
    fn the_blocked_product_matches_the_schoolbook_sum_across_block_edges() {
        let key = 0x1d;
        let mut state = 0x9e37_79b9u32;
        let mut next = || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 24) as u8
        };
        let blocking = |depth, width| {
            Gf256GemmBlocking::new(
                NonZeroUsize::new(depth).unwrap(),
                NonZeroUsize::new(width).unwrap(),
                NonZeroUsize::new(1).unwrap(),
            )
        };
        let structural = Gf256GemmBlocking::structural();
        let (depth, width) = (structural.depth(), structural.width());
        for (blocking, (m, k, n)) in [
            (structural, (0, 3, 4)),
            (structural, (3, 0, 4)),
            (structural, (1, 1, 1)),
            (structural, (5, depth + 3, 7)),
            (structural, (6, 9, width + 33)),
            (structural, (9, depth + 1, width + 1)),
            (blocking(1, 1), (5, 7, 9)),
            (blocking(3, 33), (6, 10, 100)),
            (blocking(7, 32), (4, 15, 65)),
            (blocking(64, 31), (9, 130, 95)),
        ] {
            let a: Vec<u8> = (0..m * k).map(|_| next()).collect();
            let b: Vec<u8> = (0..k * n).map(|_| next()).collect();
            let mut c = vec![0xa5u8; m * n];
            gemm_bytes(
                split_tables(key),
                &GemmShape { m, k, n },
                blocking,
                &a,
                &b,
                &mut c,
            );
            for i in 0..m {
                for j in 0..n {
                    let expected = (0..k).fold(0, |sum, p| {
                        sum ^ schoolbook_product(a[i * k + p], b[p * n + j], key)
                    });
                    assert_eq!(
                        c[i * n + j],
                        expected,
                        "{m}x{k}x{n} at {i},{j} under {blocking:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_structural_blocking_follows_the_caches_and_the_strip() {
        let caches = |l1d: usize, l2: usize, line: usize| HostCaches { l1d, l2, line };
        for (caches, strip, (depth, width, tile)) in [
            (caches(32 << 10, 512 << 10, 64), 32, (256, 1024, 64)),
            (caches(32 << 10, 256 << 10, 64), 32, (256, 512, 64)),
            (caches(48 << 10, 2 << 20, 64), 32, (384, 2720, 64)),
            (caches(48 << 10, 2 << 20, 64), 1, (384, 2730, 64)),
            (caches(16 << 10, 256 << 10, 128), 32, (64, 2048, 32)),
            (caches(64, 64, 64), 32, (1, 32, 1)),
            (caches(0, 0, 0), 0, (1, 1, 1)),
        ] {
            let blocking = Gf256GemmBlocking::for_caches(caches, strip);
            assert_eq!(
                (blocking.depth(), blocking.width(), blocking.tile()),
                (depth, width, tile),
                "{caches:?}, strip {strip}"
            );
        }
    }

    #[test]
    fn the_untranspose_is_the_same_at_every_tile() {
        for (k, n) in [
            (0usize, 5usize),
            (5, 0),
            (1, 1),
            (7, 13),
            (64, 65),
            (130, 67),
        ] {
            let b_t: Vec<u8> = (0..n * k).map(|i| (i * 31 + 7) as u8).collect();
            let expected: Vec<u8> = (0..k * n).map(|i| b_t[(i % n) * k + i / n]).collect();
            for tile in [1usize, 2, 3, 7, 63, 64, 65, 200] {
                assert_eq!(
                    untranspose(&b_t, k, n, tile, |byte| *byte),
                    expected,
                    "{k}x{n} at tile {tile}"
                );
            }
        }
    }

    #[test]
    fn sysfs_cache_sizes_parse_with_their_suffix() {
        assert_eq!(parse_cache_size("32K"), Some(32 << 10));
        assert_eq!(parse_cache_size("1M"), Some(1 << 20));
        assert_eq!(parse_cache_size("512"), Some(512));
        assert_eq!(parse_cache_size("K"), None);
        assert_eq!(parse_cache_size(""), None);
    }

    #[test]
    fn the_element_route_accumulates_products_at_every_byte_offset() {
        let key = 0x1d;
        let table = &split_tables(key)[0x57];
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

                    axpy_elements(
                        &mut destination[destination_offset..],
                        &source[source_offset..],
                        table,
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
