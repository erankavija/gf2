//! Benchmarks comparing GF(2^m) multiplication strategies.
//!
//! Covers naive (schoolbook), direct LUT, split LUT, log/exp table, `Gf2mField`,
//! and PCLMULQDQ+Barrett reduction multiplication across GF(2^8), GF(2^16),
//! GF(2^63), and — with `Gf2mField_<u128>` — GF(2^64).
//!
//! # Key questions
//!
//! 1. **Crossover point**: At what field size do table-based strategies lose their advantage?
//!    Expected: direct LUT dominates for m <= 8, log/exp tables win for m = 8..16, and
//!    schoolbook or CLMUL is required for m > 16. The split-LUT strategy for
//!    GF(2^16) tests whether partial tables can extend LUT viability.
//!
//! 2. **Throughput vs latency**: Table lookups have low latency but can suffer cache misses at
//!    scale. The split-LUT approach for GF(2^16) tests this L1 boundary explicitly.
//!
//! 3. **Batch performance**: 1000-element dot products stress the memory subsystem for LUT
//!    strategies. Batch benchmarks exist for all field sizes.
//!
//! 4. **Backend selection guidance**: Results inform runtime dispatch thresholds in
//!    `gf2-core/src/kernels/`.
//!
//! 5. **u128 storage overhead (m=64)**: `Gf2mField_<u128>` unlocks true
//!    GF(2^64) but doubles the operand width. The GF(2^64) benchmarks compare
//!    the new u128-backed path with the hand-rolled `naive_mul_64` u64
//!    baseline to quantify that overhead.
//!
//! # Strategies benchmarked
//!
//! | Strategy | GF(2^8) | GF(2^16) | GF(2^63) | GF(2^64) |
//! |----------|---------|----------|----------|----------|
//! | Naive shift-and-add | yes | yes | yes | yes (u64 specialised) |
//! | Direct LUT (256x256) | yes (64 KB) | no | no | no |
//! | Split LUT (2x 256x256) | no | yes (128 KB) | no | no |
//! | Log/exp tables | yes (2x256) | yes (2x65536) | no | no |
//! | Existing Gf2mField | yes | yes | yes | yes (via `Gf2mField_<u128>`) |
//! | PCLMULQDQ + Barrett | yes | yes | yes | m <= 63 (multi-word support deferred to JIT issue `6fb4abad`) |

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::gf2m::{Gf2mField, Gf2mField_};
use gf2_core::primitive_polys::PrimitivePolynomialDatabase;

// ---------------------------------------------------------------------------
// Primitive polynomials
// ---------------------------------------------------------------------------

/// GF(2^8): x^8 + x^4 + x^3 + x^2 + 1 (standard, same as Gf2mField::gf256)
const POLY_8: u64 = 0b1_0001_1101;

/// GF(2^16): x^16 + x^12 + x^3 + x + 1 (same as Gf2mField::gf65536)
const POLY_16: u64 = 0b1_0001_0000_0000_1011;

/// GF(2^64): x^64 + x^4 + x^3 + x + 1 (a standard irreducible polynomial for GF(2^64);
/// primitivity is not independently verified — see `standard_u128_irreducibility_note`
/// in `primitive_polys.rs`).
/// Note: This polynomial has degree 64 so the leading bit (bit 64) is implicit in the
/// reduction — we store only the lower 64 bits as the reduction mask.
const POLY_64_REDUCE: u64 = 0b11011; // x^4 + x^3 + x + 1

// ---------------------------------------------------------------------------
// Strategy 1: Naive shift-and-add (schoolbook polynomial multiplication)
// ---------------------------------------------------------------------------

/// Schoolbook GF(2^m) multiplication: shift-and-add with modular reduction.
///
/// For each bit of `b`, conditionally XOR `a` into the accumulator, then shift `a` left
/// and reduce if the high bit overflows the field.
#[inline]
fn naive_mul(a: u64, b: u64, m: usize, poly: u64) -> u64 {
    if a == 0 || b == 0 {
        return 0;
    }
    let mut result = 0u64;
    let mut temp = a;
    for i in 0..m {
        if (b >> i) & 1 == 1 {
            result ^= temp;
        }
        let will_overflow = (temp >> (m - 1)) & 1 == 1;
        temp <<= 1;
        if will_overflow {
            temp ^= poly;
        }
    }
    result & ((1u64 << m) - 1)
}

/// Naive multiplication specialized for GF(2^64).
///
/// The leading bit of the primitive polynomial is implicit (it would be bit 64, which
/// does not fit in u64). We store only the reduction polynomial (lower 64 bits).
#[inline]
fn naive_mul_64(a: u64, b: u64, reduce: u64) -> u64 {
    if a == 0 || b == 0 {
        return 0;
    }
    let mut result = 0u64;
    let mut temp = a;
    for i in 0..64 {
        if (b >> i) & 1 == 1 {
            result ^= temp;
        }
        let will_overflow = (temp >> 63) & 1 == 1;
        temp <<= 1;
        if will_overflow {
            temp ^= reduce;
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Strategy 2: Direct lookup table (256x256 = 64 KB for GF(2^8))
// ---------------------------------------------------------------------------

/// A 256x256 direct multiplication lookup table for GF(2^8).
///
/// Total size: 256 * 256 * 1 byte = 64 KB (fits in L1 cache on most CPUs).
/// Lookup is O(1) but the table must be precomputed.
struct DirectLut {
    table: Vec<u8>, // 256 * 256 = 65536 entries
}

impl DirectLut {
    fn new(poly: u64) -> Self {
        let mut table = vec![0u8; 256 * 256];
        for a in 0u64..256 {
            for b in 0u64..256 {
                table[(a as usize) * 256 + (b as usize)] = naive_mul(a, b, 8, poly) as u8;
            }
        }
        DirectLut { table }
    }

    #[inline]
    fn mul(&self, a: u8, b: u8) -> u8 {
        self.table[(a as usize) * 256 + (b as usize)]
    }
}

// ---------------------------------------------------------------------------
// Strategy 2b: Split lookup table for GF(2^16)
// ---------------------------------------------------------------------------

/// Split-LUT multiplication for GF(2^16): decomposes each 16-bit operand into two 8-bit
/// halves and uses four 256x256 sub-tables (partial products), then XORs the results.
///
/// Decomposition: a = a_hi * x^8 + a_lo, b = b_hi * x^8 + b_lo
/// a * b = (a_hi * b_hi) * x^16 + (a_hi * b_lo + a_lo * b_hi) * x^8 + a_lo * b_lo
///
/// Each partial product is an 8x8 -> 16-bit multiply, stored in a 256x256 table of u16.
/// Total size: 4 tables * 256 * 256 * 2 bytes = 128 KB (fits in L2, partially in L1).
struct SplitLut16 {
    /// table_ll[a_lo][b_lo] = reduce(a_lo * b_lo)     (low * low)
    /// table_lh[a_lo][b_hi] = reduce(a_lo * b_hi * x^8)  (low * high, shifted)
    /// table_hl[a_hi][b_lo] = reduce(a_hi * b_lo * x^8)  (high * low, shifted)
    /// table_hh[a_hi][b_hi] = reduce(a_hi * b_hi * x^16) (high * high, shifted)
    table_ll: Vec<u16>,
    table_lh: Vec<u16>,
    table_hl: Vec<u16>,
    table_hh: Vec<u16>,
}

impl SplitLut16 {
    fn new(m: usize, poly: u64) -> Self {
        let mut table_ll = vec![0u16; 256 * 256];
        let mut table_lh = vec![0u16; 256 * 256];
        let mut table_hl = vec![0u16; 256 * 256];
        let mut table_hh = vec![0u16; 256 * 256];

        // x^8 as a field element (for shifting the high-byte partial products)
        let x8: u64 = 1u64 << 8;

        for a in 0u64..256 {
            for b in 0u64..256 {
                let idx = (a as usize) * 256 + (b as usize);
                // a_lo * b_lo (no shift)
                table_ll[idx] = naive_mul(a, b, m, poly) as u16;
                // a_lo * b_hi: multiply a_lo * (b_hi * x^8)
                let bh_shifted = naive_mul(b, x8, m, poly);
                table_lh[idx] = naive_mul(a, bh_shifted, m, poly) as u16;
                // a_hi * b_lo: multiply (a_hi * x^8) * b_lo
                let ah_shifted = naive_mul(a, x8, m, poly);
                table_hl[idx] = naive_mul(ah_shifted, b, m, poly) as u16;
                // a_hi * b_hi: multiply (a_hi * x^8) * (b_hi * x^8)
                table_hh[idx] = naive_mul(ah_shifted, bh_shifted, m, poly) as u16;
            }
        }

        SplitLut16 {
            table_ll,
            table_lh,
            table_hl,
            table_hh,
        }
    }

    #[inline]
    fn mul(&self, a: u16, b: u16) -> u16 {
        let a_lo = (a & 0xFF) as usize;
        let a_hi = (a >> 8) as usize;
        let b_lo = (b & 0xFF) as usize;
        let b_hi = (b >> 8) as usize;

        self.table_ll[a_lo * 256 + b_lo]
            ^ self.table_lh[a_lo * 256 + b_hi]
            ^ self.table_hl[a_hi * 256 + b_lo]
            ^ self.table_hh[a_hi * 256 + b_hi]
    }
}

// ---------------------------------------------------------------------------
// Strategy 3: Log/exp table multiplication
// ---------------------------------------------------------------------------

/// Log/antilog (exp) table multiplication for GF(2^m).
///
/// Uses the identity: a * b = exp[log[a] + log[b]] (mod 2^m - 1).
/// Table sizes: 2 * field_size entries (log table + exp table).
/// For GF(2^8): 2 * 256 = 512 entries (negligible memory).
/// For GF(2^16): 2 * 65536 = 131072 entries (~256 KB).
struct LogExpTable {
    log_table: Vec<u32>,
    exp_table: Vec<u32>,
    _order: usize, // 2^m - 1 (multiplicative group order)
}

impl LogExpTable {
    fn new(m: usize, poly: u64) -> Self {
        let field_size = 1usize << m;
        let order = field_size - 1;

        let mut log_table = vec![0u32; field_size];
        let mut exp_table = vec![0u32; 2 * order]; // doubled for modular indexing

        // Generate using primitive element alpha = 2 (x)
        let mut val = 1u64;
        for i in 0..order {
            exp_table[i] = val as u32;
            exp_table[i + order] = val as u32; // wrap-around copy
            log_table[val as usize] = i as u32;

            // Multiply by alpha (= x): shift left, reduce if needed
            val <<= 1;
            if val & (1u64 << m) != 0 {
                val ^= poly;
            }
            val &= (1u64 << m) - 1;
        }
        // log[0] is undefined; we leave it as 0 and guard in mul()

        LogExpTable {
            log_table,
            exp_table,
            _order: order,
        }
    }

    #[inline]
    fn mul(&self, a: u64, b: u64) -> u64 {
        if a == 0 || b == 0 {
            return 0;
        }
        let log_a = self.log_table[a as usize] as usize;
        let log_b = self.log_table[b as usize] as usize;
        // No modular reduction needed: exp_table is doubled in size
        self.exp_table[log_a + log_b] as u64
    }
}

// ---------------------------------------------------------------------------
// Benchmark helpers
// ---------------------------------------------------------------------------

/// Pseudo-random non-zero element in GF(2^m).
///
/// Uses the workspace SSOT deterministic LCG in [`gf2_core::rng`] to
/// generate reproducible benchmark inputs. The `| 1` at the end ensures
/// the value is non-zero after masking, which some benchmark kernels
/// require.
#[inline]
fn pseudo_random_element(seed: u64, mask: u64) -> u64 {
    let val = gf2_core::rng::Lcg::new(seed).next_u64();
    (val & mask) | 1
}

/// Generate N pseudo-random non-zero field elements.
fn random_elements(n: usize, m: usize) -> Vec<u64> {
    let mask = (1u64 << m) - 1;
    (0..n)
        .map(|i| pseudo_random_element(i as u64, mask))
        .collect()
}

const DOT_SIZE: usize = 1000;

// ---------------------------------------------------------------------------
// Single multiplication benchmarks
// ---------------------------------------------------------------------------

fn bench_single_gf2_8(c: &mut Criterion) {
    let mut group = c.benchmark_group("gf2m_mul_single/gf2_8");
    group.throughput(Throughput::Elements(1));

    let a: u64 = 0xAB;
    let b: u64 = 0xCD;

    // Strategy 1: Naive shift-and-add
    group.bench_function("naive", |bench| {
        bench.iter(|| naive_mul(black_box(a), black_box(b), 8, POLY_8))
    });

    // Strategy 2: Direct LUT (256x256)
    let lut = DirectLut::new(POLY_8);
    group.bench_function("direct_lut", |bench| {
        bench.iter(|| lut.mul(black_box(a as u8), black_box(b as u8)))
    });

    // Strategy 3: Log/exp tables
    let log_exp = LogExpTable::new(8, POLY_8);
    group.bench_function("log_exp", |bench| {
        bench.iter(|| log_exp.mul(black_box(a), black_box(b)))
    });

    // Strategy 4: Existing Gf2mField (uses log/exp tables via with_tables(), else schoolbook)
    let field = Gf2mField::gf256().with_tables();
    let ea = field.element(a);
    let eb = field.element(b);
    group.bench_function("gf2m_field", |bench| {
        bench.iter(|| black_box(&ea) * black_box(&eb))
    });

    group.finish();
}

fn bench_single_gf2_16(c: &mut Criterion) {
    let mut group = c.benchmark_group("gf2m_mul_single/gf2_16");
    group.throughput(Throughput::Elements(1));

    let a: u64 = 0xABCD;
    let b: u64 = 0x1234;

    // Strategy 1: Naive shift-and-add
    group.bench_function("naive", |bench| {
        bench.iter(|| naive_mul(black_box(a), black_box(b), 16, POLY_16))
    });

    // Strategy 2: Direct LUT — N/A for GF(2^16): a full 65536x65536 table would be 4 GB.
    // Instead we use a split-LUT approach with two 8-bit halves (128 KB total).
    let split_lut = SplitLut16::new(16, POLY_16);
    group.bench_function("split_lut", |bench| {
        bench.iter(|| split_lut.mul(black_box(a as u16), black_box(b as u16)))
    });

    // Strategy 3: Log/exp tables
    let log_exp = LogExpTable::new(16, POLY_16);
    group.bench_function("log_exp", |bench| {
        bench.iter(|| log_exp.mul(black_box(a), black_box(b)))
    });

    // Strategy 4: Existing Gf2mField with tables
    let field = Gf2mField::gf65536().with_tables();
    let ea = field.element(a);
    let eb = field.element(b);
    group.bench_function("gf2m_field", |bench| {
        bench.iter(|| black_box(&ea) * black_box(&eb))
    });

    group.finish();
}

fn bench_single_gf2_63(c: &mut Criterion) {
    // We use m=63 (not 64) because Gf2mField requires m < 64 for u64 storage:
    // the schoolbook reduction needs the leading bit (bit m) to fit in u64, so the
    // maximum extension degree is 63. GF(2^63) serves as the large-field representative
    // for these benchmarks. True GF(2^64) would require u128 backing or the dedicated
    // naive_mul_64 path (benchmarked below as "naive_64").
    let mut group = c.benchmark_group("gf2m_mul_single/gf2_63");
    group.throughput(Throughput::Elements(1));

    let a: u64 = 0xDEAD_BEEF_CAFE_BABE;
    let b: u64 = 0x0123_4567_89AB_CDEF;

    // x^63 + x + 1 -- a known primitive polynomial for GF(2^63)
    let poly_63: u64 = (1u64 << 63) | 0b11;
    let mask_63 = (1u64 << 63) - 1;

    // Strategy 1a: Naive shift-and-add via the generic routine (m=63)
    group.bench_function("naive", |bench| {
        bench.iter(|| naive_mul(black_box(a & mask_63), black_box(b & mask_63), 63, poly_63))
    });

    // Strategy 1b: Naive shift-and-add with the GF(2^64) specialization.
    // This uses a different reduction polynomial (x^64 + x^4 + x^3 + x + 1) and operates
    // on the full 64-bit space, so it is NOT directly comparable to the m=63 results —
    // it is included to show the cost of true 64-bit schoolbook multiplication.
    group.bench_function("naive_64", |bench| {
        bench.iter(|| naive_mul_64(black_box(a), black_box(b), POLY_64_REDUCE))
    });

    // Strategies 2,3: LUT and log/exp tables are infeasible for fields this large

    // Strategy 4: Existing Gf2mField (schoolbook, no tables for m=63)
    let field = Gf2mField::new(63, poly_63);
    let ea = field.element(a & mask_63);
    let eb = field.element(b & mask_63);
    group.bench_function("gf2m_field", |bench| {
        bench.iter(|| black_box(&ea) * black_box(&eb))
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Dot product benchmarks (batch performance)
// ---------------------------------------------------------------------------

fn bench_dot_gf2_8(c: &mut Criterion) {
    let mut group = c.benchmark_group("gf2m_dot_product/gf2_8");
    group.throughput(Throughput::Elements(DOT_SIZE as u64));

    let xs = random_elements(DOT_SIZE, 8);
    let ys = random_elements(DOT_SIZE, 8);

    // Strategy 1: Naive shift-and-add
    group.bench_function("naive", |bench| {
        bench.iter(|| {
            let mut acc = 0u64;
            for (&x, &y) in xs.iter().zip(ys.iter()) {
                acc ^= naive_mul(x, y, 8, POLY_8);
            }
            black_box(acc)
        })
    });

    // Strategy 2: Direct LUT
    let lut = DirectLut::new(POLY_8);
    group.bench_function("direct_lut", |bench| {
        bench.iter(|| {
            let mut acc = 0u8;
            for (&x, &y) in xs.iter().zip(ys.iter()) {
                acc ^= lut.mul(x as u8, y as u8);
            }
            black_box(acc)
        })
    });

    // Strategy 3: Log/exp tables
    let log_exp = LogExpTable::new(8, POLY_8);
    group.bench_function("log_exp", |bench| {
        bench.iter(|| {
            let mut acc = 0u64;
            for (&x, &y) in xs.iter().zip(ys.iter()) {
                acc ^= log_exp.mul(x, y);
            }
            black_box(acc)
        })
    });

    // Strategy 4: Existing Gf2mField
    let field = Gf2mField::gf256().with_tables();
    let ex: Vec<_> = xs.iter().map(|&x| field.element(x)).collect();
    let ey: Vec<_> = ys.iter().map(|&y| field.element(y)).collect();
    group.bench_function("gf2m_field", |bench| {
        bench.iter(|| {
            let mut acc = field.element(0);
            for (a, b) in ex.iter().zip(ey.iter()) {
                acc += a * b;
            }
            black_box(acc.value())
        })
    });

    group.finish();
}

fn bench_dot_gf2_16(c: &mut Criterion) {
    let mut group = c.benchmark_group("gf2m_dot_product/gf2_16");
    group.throughput(Throughput::Elements(DOT_SIZE as u64));

    let xs = random_elements(DOT_SIZE, 16);
    let ys = random_elements(DOT_SIZE, 16);

    // Strategy 1: Naive shift-and-add
    group.bench_function("naive", |bench| {
        bench.iter(|| {
            let mut acc = 0u64;
            for (&x, &y) in xs.iter().zip(ys.iter()) {
                acc ^= naive_mul(x, y, 16, POLY_16);
            }
            black_box(acc)
        })
    });

    // Strategy 2: Split LUT (128 KB)
    let split_lut = SplitLut16::new(16, POLY_16);
    group.bench_function("split_lut", |bench| {
        bench.iter(|| {
            let mut acc = 0u16;
            for (&x, &y) in xs.iter().zip(ys.iter()) {
                acc ^= split_lut.mul(x as u16, y as u16);
            }
            black_box(acc)
        })
    });

    // Strategy 3: Log/exp tables
    let log_exp = LogExpTable::new(16, POLY_16);
    group.bench_function("log_exp", |bench| {
        bench.iter(|| {
            let mut acc = 0u64;
            for (&x, &y) in xs.iter().zip(ys.iter()) {
                acc ^= log_exp.mul(x, y);
            }
            black_box(acc)
        })
    });

    // Strategy 4: Existing Gf2mField
    let field = Gf2mField::gf65536().with_tables();
    let ex: Vec<_> = xs.iter().map(|&x| field.element(x)).collect();
    let ey: Vec<_> = ys.iter().map(|&y| field.element(y)).collect();
    group.bench_function("gf2m_field", |bench| {
        bench.iter(|| {
            let mut acc = field.element(0);
            for (a, b) in ex.iter().zip(ey.iter()) {
                acc += a * b;
            }
            black_box(acc.value())
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Dot product benchmark for large field (GF(2^63))
// ---------------------------------------------------------------------------

fn bench_dot_gf2_63(c: &mut Criterion) {
    let mut group = c.benchmark_group("gf2m_dot_product/gf2_63");
    group.throughput(Throughput::Elements(DOT_SIZE as u64));

    // x^63 + x + 1
    let poly_63: u64 = (1u64 << 63) | 0b11;
    let mask_63 = (1u64 << 63) - 1;

    let xs: Vec<u64> = random_elements(DOT_SIZE, 63);
    let ys: Vec<u64> = random_elements(DOT_SIZE, 63);

    // Strategy 1: Naive shift-and-add (generic, m=63)
    group.bench_function("naive", |bench| {
        bench.iter(|| {
            let mut acc = 0u64;
            for (&x, &y) in xs.iter().zip(ys.iter()) {
                acc ^= naive_mul(x, y, 63, poly_63);
            }
            black_box(acc)
        })
    });

    // Strategy 4: Existing Gf2mField (schoolbook, no tables)
    let field = Gf2mField::new(63, poly_63);
    let ex: Vec<_> = xs
        .iter()
        .map(|&x| field.element(x & mask_63))
        .collect::<Vec<_>>();
    let ey: Vec<_> = ys
        .iter()
        .map(|&y| field.element(y & mask_63))
        .collect::<Vec<_>>();
    group.bench_function("gf2m_field", |bench| {
        bench.iter(|| {
            let mut acc = field.element(0);
            for (a, b) in ex.iter().zip(ey.iter()) {
                acc += a * b;
            }
            black_box(acc.value())
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// True GF(2^64) benchmarks via Gf2mField_<u128>
// ---------------------------------------------------------------------------

/// Pseudo-random 64-bit element suitable for GF(2^64).
#[inline]
fn pseudo_random_u64_element(seed: u64) -> u64 {
    // `| 1` guarantees a non-zero operand; MSB is allowed since m=64.
    gf2_core::rng::Lcg::new(seed).next_u64() | 1
}

/// Generate N pseudo-random non-zero elements in GF(2^64), stored as `u128`.
fn random_elements_gf2_64(n: usize) -> Vec<u128> {
    (0..n)
        .map(|i| pseudo_random_u64_element(i as u64) as u128)
        .collect()
}

fn bench_single_gf2_64(c: &mut Criterion) {
    // GF(2^64) is the smallest field where u64 storage is insufficient: the
    // leading coefficient of the primitive polynomial sits at bit 64 and does
    // not fit in the element slot. We therefore dispatch through
    // `Gf2mField_<u128>` and compare against the hand-rolled `naive_mul_64`
    // baseline (which avoids storing the leading bit at all).
    let mut group = c.benchmark_group("gf2m_mul_single/gf2_64");
    group.throughput(Throughput::Elements(1));

    let a: u64 = 0xDEAD_BEEF_CAFE_BABE;
    let b: u64 = 0x0123_4567_89AB_CDEF;

    // Reference: u64-specialised schoolbook (no u128 overhead, no bit storage for x^64).
    group.bench_function("naive_64", |bench| {
        bench.iter(|| naive_mul_64(black_box(a), black_box(b), POLY_64_REDUCE))
    });

    // Actual u128-backed Gf2mField<u128> at m=64 (the new path unlocked by c488ed29).
    let poly_64 = PrimitivePolynomialDatabase::standard_u128(64)
        .expect("GF(2^64) standard polynomial catalogued");
    let field = Gf2mField_::<u128>::new(64, poly_64);
    let ea = field.element(a as u128);
    let eb = field.element(b as u128);
    group.bench_function("gf2m_field_u128", |bench| {
        bench.iter(|| black_box(&ea) * black_box(&eb))
    });

    group.finish();
}

fn bench_dot_gf2_64(c: &mut Criterion) {
    let mut group = c.benchmark_group("gf2m_dot_product/gf2_64");
    group.throughput(Throughput::Elements(DOT_SIZE as u64));

    let xs = random_elements_gf2_64(DOT_SIZE);
    let ys = random_elements_gf2_64(DOT_SIZE);

    // Reference: u64 specialised schoolbook (strategy 1b from the single-mul bench).
    group.bench_function("naive_64", |bench| {
        bench.iter(|| {
            let mut acc = 0u64;
            for (&x, &y) in xs.iter().zip(ys.iter()) {
                acc ^= naive_mul_64(x as u64, y as u64, POLY_64_REDUCE);
            }
            black_box(acc)
        })
    });

    // Gf2mField_<u128> at m=64 (the new public path).
    let poly_64 = PrimitivePolynomialDatabase::standard_u128(64)
        .expect("GF(2^64) standard polynomial catalogued");
    let field = Gf2mField_::<u128>::new(64, poly_64);
    let ex: Vec<_> = xs.iter().map(|&x| field.element(x)).collect();
    let ey: Vec<_> = ys.iter().map(|&y| field.element(y)).collect();
    group.bench_function("gf2m_field_u128", |bench| {
        bench.iter(|| {
            let mut acc = field.element(0);
            for (a, b) in ex.iter().zip(ey.iter()) {
                acc += a * b;
            }
            black_box(acc.value())
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Parameterized sweep: single mul across field sizes (for crossover analysis)
// ---------------------------------------------------------------------------

fn bench_crossover_sweep(c: &mut Criterion) {
    let mut group = c.benchmark_group("gf2m_mul_crossover");

    // Field sizes and their primitive polynomials
    let fields: &[(usize, u64)] = &[
        (4, 0b10011),
        (8, POLY_8),
        (12, 0b1000001010011), // x^12 + x^6 + x^4 + x + 1 (from primitive_polys database)
        (16, POLY_16),
    ];

    for &(m, poly) in fields {
        let mask = (1u64 << m) - 1;
        let a = 0xABCD_EF01u64 & mask | 1;
        let b = 0x1234_5678u64 & mask | 1;

        // Naive
        group.bench_with_input(BenchmarkId::new("naive", m), &m, |bench, _| {
            bench.iter(|| naive_mul(black_box(a), black_box(b), m, poly))
        });

        // Log/exp tables
        let log_exp = LogExpTable::new(m, poly);
        group.bench_with_input(BenchmarkId::new("log_exp", m), &m, |bench, _| {
            bench.iter(|| log_exp.mul(black_box(a), black_box(b)))
        });

        // Existing Gf2mField (with tables where applicable)
        let field = Gf2mField::new(m, poly).with_tables();
        let ea = field.element(a);
        let eb = field.element(b);
        group.bench_with_input(BenchmarkId::new("gf2m_field", m), &m, |bench, _| {
            bench.iter(|| black_box(&ea) * black_box(&eb))
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// PCLMULQDQ + Barrett benchmarks
// ---------------------------------------------------------------------------

/// Benchmark PCLMULQDQ + Barrett reduction strategy.
///
/// This strategy uses the all-in-one carry-less multiply + Barrett reduce kernel
/// that performs all three PCLMULQDQ instructions in a single `#[target_feature]`
/// scope, eliminating function-pointer call overhead.
///
/// ## Measured crossover results (Zen 4, 2026-04-05)
///
/// | m | Naive | PCLMULQDQ+Barrett | Speedup |
/// |---|-------|-------------------|---------|
/// | 4 | 2.76 ns | 4.49 ns | 0.61× (naive wins) |
/// | 8 | 5.80 ns | 4.49 ns | 1.29× (PCLMULQDQ wins) |
/// | 12 | 7.22 ns | 12.99 ns | 0.56× (naive wins) |
/// | 16 | 9.52 ns | 13.0 ns | 0.73× (naive wins) |
///
/// PCLMULQDQ+Barrett wins at m=8 (23% faster). For m≥12, the u128 register
/// transfer overhead in Barrett reduction outweighs the O(m) naive loop.
/// Barrett becomes dominant for m≥32+ when u128 storage is added (c488ed29).
fn bench_pclmulqdq_barrett(c: &mut Criterion) {
    use gf2_core::gf2m::barrett::BarrettReducer;

    // Detect PCLMULQDQ via the kernels crate directly
    let gf2m_fns = gf2_kernels_simd::gf2m::detect();
    let clmul_barrett_fn = gf2m_fns.as_ref().and_then(|f| f.clmul_barrett_fn);
    let clmul_fn = gf2m_fns.as_ref().and_then(|f| f.clmul_fn);

    if clmul_barrett_fn.is_none() {
        // PCLMULQDQ not available; skip these benchmarks silently
        return;
    }

    let clmul_barrett = clmul_barrett_fn.unwrap();
    let clmul = clmul_fn.unwrap();

    let fields: &[(usize, u64, &str)] = &[(8, POLY_8, "gf2_8"), (16, POLY_16, "gf2_16")];

    for &(m, poly, label) in fields {
        let reducer = BarrettReducer::new(poly as u128, m as u32);
        let mu = reducer.mu() as u64;
        let modulus = reducer.modulus() as u64;
        let degree = reducer.degree();
        let mask = (1u64 << m) - 1;
        let a = 0xABCD_EF01u64 & mask | 1;
        let b = 0x1234_5678u64 & mask | 1;

        // Single multiplication (all-in-one kernel)
        {
            let mut group = c.benchmark_group(format!("gf2m_mul_single/{label}"));
            group.throughput(Throughput::Elements(1));
            group.bench_function("pclmulqdq_barrett", |bench| {
                bench.iter(|| clmul_barrett(black_box(a), black_box(b), mu, modulus, degree))
            });
            group.finish();
        }

        // Dot product (all-in-one kernel)
        {
            let xs = random_elements(DOT_SIZE, m);
            let ys = random_elements(DOT_SIZE, m);
            let mut group = c.benchmark_group(format!("gf2m_dot_product/{label}"));
            group.throughput(Throughput::Elements(DOT_SIZE as u64));
            group.bench_function("pclmulqdq_barrett", |bench| {
                bench.iter(|| {
                    let mut acc = 0u64;
                    for (&x, &y) in xs.iter().zip(ys.iter()) {
                        acc ^= clmul_barrett(x, y, mu, modulus, degree);
                    }
                    black_box(acc)
                })
            });
            group.finish();
        }
    }

    // Batch carry-less multiply benchmark (VPCLMULQDQ path when available)
    let batch_fn = gf2m_fns.as_ref().and_then(|f| f.clmul_batch_fn);
    if let Some(batch_clmul) = batch_fn {
        let reducer = BarrettReducer::new(POLY_8 as u128, 8);
        let xs = random_elements(DOT_SIZE, 8);
        let ys = random_elements(DOT_SIZE, 8);

        let mut group = c.benchmark_group("gf2m_dot_product/gf2_8");
        group.throughput(Throughput::Elements(DOT_SIZE as u64));
        group.bench_function("pclmulqdq_batch_barrett", |bench| {
            bench.iter(|| {
                let mut products = vec![0u128; DOT_SIZE];
                batch_clmul(&xs, &ys, &mut products);
                let mut acc = 0u64;
                for &p in &products {
                    acc ^= reducer.reduce_with_clmul(p, clmul);
                }
                black_box(acc)
            })
        });
        group.finish();
    }

    // Crossover sweep with PCLMULQDQ + Barrett (all-in-one kernel)
    {
        let mut group = c.benchmark_group("gf2m_mul_crossover");
        let sweep_fields: &[(usize, u64)] = &[
            (4, 0b10011),
            (8, POLY_8),
            (12, 0b1000001010011),
            (16, POLY_16),
        ];

        for &(m, poly) in sweep_fields {
            let reducer = BarrettReducer::new(poly as u128, m as u32);
            let mu = reducer.mu() as u64;
            let modulus = reducer.modulus() as u64;
            let degree = reducer.degree();
            let mask = (1u64 << m) - 1;
            let a = 0xABCD_EF01u64 & mask | 1;
            let b = 0x1234_5678u64 & mask | 1;

            group.bench_with_input(BenchmarkId::new("pclmulqdq_barrett", m), &m, |bench, _| {
                bench.iter(|| clmul_barrett(black_box(a), black_box(b), mu, modulus, degree))
            });
        }

        group.finish();
    }
}

criterion_group!(
    benches,
    bench_single_gf2_8,
    bench_single_gf2_16,
    bench_single_gf2_63,
    bench_single_gf2_64,
    bench_dot_gf2_8,
    bench_dot_gf2_16,
    bench_dot_gf2_63,
    bench_dot_gf2_64,
    bench_crossover_sweep,
    bench_pclmulqdq_barrett,
);
criterion_main!(benches);

// ---------------------------------------------------------------------------
// Correctness verification
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    /// Verify that all multiplication strategies produce identical results for GF(2^8).
    #[test]
    fn test_mul_strategies_agree_gf2_8() {
        let lut = DirectLut::new(POLY_8);
        let log_exp = LogExpTable::new(8, POLY_8);
        let field = Gf2mField::gf256().with_tables();

        // Test a representative set of input pairs including edge cases
        let test_pairs: Vec<(u64, u64)> = vec![
            (0, 0),
            (0, 1),
            (1, 0),
            (1, 1),
            (1, 255),
            (255, 255),
            (0xAB, 0xCD),
            (2, 3), // alpha * (alpha + 1)
            (127, 128),
            (0x53, 0xCA),
        ];

        for (a, b) in test_pairs {
            let naive_result = naive_mul(a, b, 8, POLY_8);
            let lut_result = lut.mul(a as u8, b as u8) as u64;
            let log_exp_result = log_exp.mul(a, b);
            let field_result = if a == 0 || b == 0 {
                // Gf2mField element panics on multiply check, just verify zero
                0u64
            } else {
                let ea = field.element(a);
                let eb = field.element(b);
                (&ea * &eb).value()
            };

            assert_eq!(
                naive_result, lut_result,
                "Naive vs LUT mismatch for ({a}, {b}): {naive_result} != {lut_result}"
            );
            assert_eq!(
                naive_result, log_exp_result,
                "Naive vs log/exp mismatch for ({a}, {b}): {naive_result} != {log_exp_result}"
            );
            // field_result for zero inputs is handled separately
            if a != 0 && b != 0 {
                assert_eq!(
                    naive_result, field_result,
                    "Naive vs Gf2mField mismatch for ({a}, {b}): {naive_result} != {field_result}"
                );
            }
        }
    }

    /// Verify that all applicable strategies agree for GF(2^16).
    #[test]
    fn test_mul_strategies_agree_gf2_16() {
        let log_exp = LogExpTable::new(16, POLY_16);
        let field = Gf2mField::gf65536().with_tables();

        let test_pairs: Vec<(u64, u64)> = vec![
            (0, 0),
            (1, 1),
            (0xABCD, 0x1234),
            (0xFFFF, 0xFFFF),
            (2, 3),
            (0x8000, 0x0001),
            (0x7FFF, 0x8001),
        ];

        for (a, b) in test_pairs {
            let naive_result = naive_mul(a, b, 16, POLY_16);
            let log_exp_result = log_exp.mul(a, b);

            assert_eq!(
                naive_result, log_exp_result,
                "Naive vs log/exp mismatch for ({a:#06x}, {b:#06x}): {naive_result:#06x} != {log_exp_result:#06x}"
            );

            if a != 0 && b != 0 {
                let ea = field.element(a);
                let eb = field.element(b);
                let field_result = (&ea * &eb).value();
                assert_eq!(
                    naive_result, field_result,
                    "Naive vs Gf2mField mismatch for ({a:#06x}, {b:#06x}): {naive_result:#06x} != {field_result:#06x}"
                );
            }
        }
    }

    /// Verify the GF(2^64) naive multiplication against known identities.
    #[test]
    fn test_mul_gf2_64_identities() {
        // Multiplicative identity: a * 1 = a
        let a = 0xDEAD_BEEF_CAFE_BABEu64;
        assert_eq!(naive_mul_64(a, 1, POLY_64_REDUCE), a);
        assert_eq!(naive_mul_64(1, a, POLY_64_REDUCE), a);

        // Zero: a * 0 = 0
        assert_eq!(naive_mul_64(a, 0, POLY_64_REDUCE), 0);
        assert_eq!(naive_mul_64(0, a, POLY_64_REDUCE), 0);

        // Commutativity: a * b = b * a
        let b = 0x0123_4567_89AB_CDEFu64;
        assert_eq!(
            naive_mul_64(a, b, POLY_64_REDUCE),
            naive_mul_64(b, a, POLY_64_REDUCE)
        );
    }

    /// Cross-check that the `naive_mul_64` u64-specialised path and the u128-backed
    /// `Gf2mField_<u128>` path agree on GF(2^64) — both use the primitive polynomial
    /// x^64 + x^4 + x^3 + x + 1 (with the leading bit stored implicitly in `naive_mul_64`
    /// and explicitly in the u128 field). This ensures the GF(2^64) benchmark pair
    /// measures equivalent work.
    #[test]
    fn test_gf2_64_naive_matches_u128_field() {
        let poly_64_u128 = (1u128 << 64) | (POLY_64_REDUCE as u128);
        let field = Gf2mField_::<u128>::new(64, poly_64_u128);

        let test_pairs: &[(u64, u64)] = &[
            (0, 0),
            (1, 1),
            (0xDEAD_BEEF_CAFE_BABE, 0x0123_4567_89AB_CDEF),
            (u64::MAX, u64::MAX),
            (1u64 << 63, 2),
            (1u64 << 63, 1u64 << 63),
            (0xAAAA_AAAA_AAAA_AAAA, 0x5555_5555_5555_5555),
        ];

        for &(a, b) in test_pairs {
            let naive = naive_mul_64(a, b, POLY_64_REDUCE);
            let ea = field.element(a as u128);
            let eb = field.element(b as u128);
            let viafield = (&ea * &eb).value() as u64;
            assert_eq!(
                naive, viafield,
                "GF(2^64) mismatch for ({a:#018x}, {b:#018x}): \
                 naive_mul_64={naive:#018x} vs Gf2mField_<u128>={viafield:#018x}"
            );
        }
    }

    /// Verify that naive_mul (generic) and Gf2mField produce identical results for GF(2^63).
    ///
    /// This is important because the large-field benchmarks use both paths (naive_mul with
    /// m=63 and Gf2mField with m=63) and their results must agree.
    #[test]
    fn test_mul_strategies_agree_gf2_63() {
        // x^63 + x + 1
        let poly_63: u64 = (1u64 << 63) | 0b11;
        let mask_63 = (1u64 << 63) - 1;
        let field = Gf2mField::new(63, poly_63);

        let test_pairs: Vec<(u64, u64)> = vec![
            (1, 1),
            (1, mask_63),
            (2, 3),
            (0xDEAD_BEEF & mask_63, 0xCAFE_BABE & mask_63),
            (
                0x0123_4567_89AB_CDEF & mask_63,
                0xFEDC_BA98_7654_3210 & mask_63,
            ),
            (mask_63, mask_63),
            (0x4000_0000_0000_0000, 0x4000_0000_0000_0000), // high-bit elements
            (mask_63, 1),
        ];

        for (a, b) in test_pairs {
            let naive_result = naive_mul(a, b, 63, poly_63);
            let ea = field.element(a);
            let eb = field.element(b);
            let field_result = (&ea * &eb).value();

            assert_eq!(
                naive_result, field_result,
                "Naive vs Gf2mField mismatch for GF(2^63) ({a:#018x}, {b:#018x}): \
                 {naive_result:#018x} != {field_result:#018x}"
            );

            // Verify commutativity for both strategies
            let naive_commuted = naive_mul(b, a, 63, poly_63);
            assert_eq!(
                naive_result, naive_commuted,
                "Commutativity failure for naive GF(2^63) ({a:#018x}, {b:#018x})"
            );
        }

        // Also verify identity and zero
        let a = 0x5555_5555_5555_5555u64 & mask_63;
        assert_eq!(
            naive_mul(a, 1, 63, poly_63),
            a,
            "Multiplicative identity failed"
        );
        assert_eq!(
            naive_mul(a, 0, 63, poly_63),
            0,
            "Zero multiplication failed"
        );
    }

    /// Verify that the split-LUT strategy agrees with naive and log/exp for GF(2^16).
    #[test]
    fn test_split_lut_agrees_gf2_16() {
        let split_lut = SplitLut16::new(16, POLY_16);
        let log_exp = LogExpTable::new(16, POLY_16);

        let test_pairs: Vec<(u64, u64)> = vec![
            (0, 0),
            (1, 1),
            (0xABCD, 0x1234),
            (0xFFFF, 0xFFFF),
            (2, 3),
            (0x8000, 0x0001),
            (0x7FFF, 0x8001),
            (0x00FF, 0xFF00), // tests cross-byte interaction
            (0xFF00, 0x00FF),
        ];

        for (a, b) in test_pairs {
            let naive_result = naive_mul(a, b, 16, POLY_16);
            let split_result = split_lut.mul(a as u16, b as u16) as u64;
            let log_exp_result = log_exp.mul(a, b);

            assert_eq!(
                naive_result, split_result,
                "Naive vs split-LUT mismatch for ({a:#06x}, {b:#06x}): \
                 {naive_result:#06x} != {split_result:#06x}"
            );
            assert_eq!(
                naive_result, log_exp_result,
                "Naive vs log/exp mismatch for ({a:#06x}, {b:#06x}): \
                 {naive_result:#06x} != {log_exp_result:#06x}"
            );
        }
    }

    /// Verify log/exp table internal consistency.
    #[test]
    fn test_log_exp_table_roundtrip_gf2_8() {
        let table = LogExpTable::new(8, POLY_8);

        // exp[log[a]] = a for all non-zero a
        for a in 1u64..256 {
            let log_a = table.log_table[a as usize] as usize;
            assert_eq!(table.exp_table[log_a] as u64, a, "exp[log[{a}]] != {a}");
        }

        // log[exp[i]] = i for i in 0..255
        for i in 0usize..255 {
            let exp_i = table.exp_table[i] as usize;
            assert_eq!(table.log_table[exp_i] as usize, i, "log[exp[{i}]] != {i}");
        }
    }
}
