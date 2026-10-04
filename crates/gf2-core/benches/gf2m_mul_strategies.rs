//! Benchmarks GF(2^m) multiplication strategies (schoolbook, direct LUT, split LUT,
//! log/exp tables, `Gf2mField`, PCLMULQDQ+Barrett and the AVX2 VPCLMULQDQ batch kernel)
//! across GF(2^8) to GF(2^64).

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_core::gf2m::{Gf2mField, Gf2mField_};
use gf2_core::primitive_polys::PrimitivePolynomialDatabase;

/// x^8 + x^4 + x^3 + x^2 + 1.
const POLY_8: u64 = 0b1_0001_1101;

/// x^16 + x^12 + x^3 + x + 1.
const POLY_16: u64 = 0b1_0001_0000_0000_1011;

/// Low 64 bits of x^64 + x^4 + x^3 + x + 1; the x^64 term is implicit.
const POLY_64_REDUCE: u64 = 0b11011; // x^4 + x^3 + x + 1

/// Schoolbook GF(2^m) multiplication: shift-and-add with modular reduction.
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

/// A 256x256 direct multiplication lookup table for GF(2^8).
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

/// Split-LUT multiplication for GF(2^16): decomposes each 16-bit operand into two 8-bit
/// halves and uses four 256x256 sub-tables (partial products), then XORs the results.
///
/// Decomposition: a = a_hi * x^8 + a_lo, b = b_hi * x^8 + b_lo
/// a * b = (a_hi * b_hi) * x^16 + (a_hi * b_lo + a_lo * b_hi) * x^8 + a_lo * b_lo
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

        let x8: u64 = 1u64 << 8;

        for a in 0u64..256 {
            for b in 0u64..256 {
                let idx = (a as usize) * 256 + (b as usize);
                table_ll[idx] = naive_mul(a, b, m, poly) as u16;
                let bh_shifted = naive_mul(b, x8, m, poly);
                table_lh[idx] = naive_mul(a, bh_shifted, m, poly) as u16;
                let ah_shifted = naive_mul(a, x8, m, poly);
                table_hl[idx] = naive_mul(ah_shifted, b, m, poly) as u16;
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

/// Log/antilog (exp) table multiplication for GF(2^m).
///
/// Uses the identity: a * b = exp[log[a] + log[b]] (mod 2^m - 1).
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

/// Pseudo-random element of GF(2^m); `| 1` keeps it non-zero after masking.
#[inline]
fn pseudo_random_element(seed: u64, mask: u64) -> u64 {
    let val = gf2_core::rng::Lcg::new(seed).next_u64();
    (val & mask) | 1
}

fn random_elements(n: usize, m: usize) -> Vec<u64> {
    let mask = (1u64 << m) - 1;
    (0..n)
        .map(|i| pseudo_random_element(i as u64, mask))
        .collect()
}

const DOT_SIZE: usize = 1000;

fn bench_single_gf2_8(c: &mut Criterion) {
    let mut group = c.benchmark_group("gf2m_mul_single/gf2_8");
    group.throughput(Throughput::Elements(1));

    let a: u64 = 0xAB;
    let b: u64 = 0xCD;

    group.bench_function("naive", |bench| {
        bench.iter(|| naive_mul(black_box(a), black_box(b), 8, POLY_8))
    });

    let lut = DirectLut::new(POLY_8);
    group.bench_function("direct_lut", |bench| {
        bench.iter(|| lut.mul(black_box(a as u8), black_box(b as u8)))
    });

    let log_exp = LogExpTable::new(8, POLY_8);
    group.bench_function("log_exp", |bench| {
        bench.iter(|| log_exp.mul(black_box(a), black_box(b)))
    });

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

    group.bench_function("naive", |bench| {
        bench.iter(|| naive_mul(black_box(a), black_box(b), 16, POLY_16))
    });

    let split_lut = SplitLut16::new(16, POLY_16);
    group.bench_function("split_lut", |bench| {
        bench.iter(|| split_lut.mul(black_box(a as u16), black_box(b as u16)))
    });

    let log_exp = LogExpTable::new(16, POLY_16);
    group.bench_function("log_exp", |bench| {
        bench.iter(|| log_exp.mul(black_box(a), black_box(b)))
    });

    let field = Gf2mField::gf65536().with_tables();
    let ea = field.element(a);
    let eb = field.element(b);
    group.bench_function("gf2m_field", |bench| {
        bench.iter(|| black_box(&ea) * black_box(&eb))
    });

    group.finish();
}

fn bench_single_gf2_63(c: &mut Criterion) {
    // m = 63 is the largest degree whose modulus fits in u64.
    let mut group = c.benchmark_group("gf2m_mul_single/gf2_63");
    group.throughput(Throughput::Elements(1));

    let a: u64 = 0xDEAD_BEEF_CAFE_BABE;
    let b: u64 = 0x0123_4567_89AB_CDEF;

    // x^63 + x + 1
    let poly_63: u64 = (1u64 << 63) | 0b11;
    let mask_63 = (1u64 << 63) - 1;

    group.bench_function("naive", |bench| {
        bench.iter(|| naive_mul(black_box(a & mask_63), black_box(b & mask_63), 63, poly_63))
    });

    // Modulus x^64 + x^4 + x^3 + x + 1 on the full 64-bit space: a different field from m = 63.
    group.bench_function("naive_64", |bench| {
        bench.iter(|| naive_mul_64(black_box(a), black_box(b), POLY_64_REDUCE))
    });

    let field = Gf2mField::new(63, poly_63);
    let ea = field.element(a & mask_63);
    let eb = field.element(b & mask_63);
    group.bench_function("gf2m_field", |bench| {
        bench.iter(|| black_box(&ea) * black_box(&eb))
    });

    group.finish();
}

fn bench_dot_gf2_8(c: &mut Criterion) {
    let mut group = c.benchmark_group("gf2m_dot_product/gf2_8");
    group.throughput(Throughput::Elements(DOT_SIZE as u64));

    let xs = random_elements(DOT_SIZE, 8);
    let ys = random_elements(DOT_SIZE, 8);

    group.bench_function("naive", |bench| {
        bench.iter(|| {
            let mut acc = 0u64;
            for (&x, &y) in xs.iter().zip(ys.iter()) {
                acc ^= naive_mul(x, y, 8, POLY_8);
            }
            black_box(acc)
        })
    });

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

    group.bench_function("naive", |bench| {
        bench.iter(|| {
            let mut acc = 0u64;
            for (&x, &y) in xs.iter().zip(ys.iter()) {
                acc ^= naive_mul(x, y, 16, POLY_16);
            }
            black_box(acc)
        })
    });

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

fn bench_dot_gf2_63(c: &mut Criterion) {
    let mut group = c.benchmark_group("gf2m_dot_product/gf2_63");
    group.throughput(Throughput::Elements(DOT_SIZE as u64));

    // x^63 + x + 1
    let poly_63: u64 = (1u64 << 63) | 0b11;
    let mask_63 = (1u64 << 63) - 1;

    let xs: Vec<u64> = random_elements(DOT_SIZE, 63);
    let ys: Vec<u64> = random_elements(DOT_SIZE, 63);

    group.bench_function("naive", |bench| {
        bench.iter(|| {
            let mut acc = 0u64;
            for (&x, &y) in xs.iter().zip(ys.iter()) {
                acc ^= naive_mul(x, y, 63, poly_63);
            }
            black_box(acc)
        })
    });

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

#[inline]
fn pseudo_random_u64_element(seed: u64) -> u64 {
    // `| 1` guarantees a non-zero operand; MSB is allowed since m=64.
    gf2_core::rng::Lcg::new(seed).next_u64() | 1
}

fn random_elements_gf2_64(n: usize) -> Vec<u128> {
    (0..n)
        .map(|i| pseudo_random_u64_element(i as u64) as u128)
        .collect()
}

fn bench_single_gf2_64(c: &mut Criterion) {
    // The modulus of GF(2^64) has bit 64 set, so elements use `Gf2mField_<u128>`;
    // `naive_mul_64` keeps that bit implicit.
    let mut group = c.benchmark_group("gf2m_mul_single/gf2_64");
    group.throughput(Throughput::Elements(1));

    let a: u64 = 0xDEAD_BEEF_CAFE_BABE;
    let b: u64 = 0x0123_4567_89AB_CDEF;

    group.bench_function("naive_64", |bench| {
        bench.iter(|| naive_mul_64(black_box(a), black_box(b), POLY_64_REDUCE))
    });

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

    group.bench_function("naive_64", |bench| {
        bench.iter(|| {
            let mut acc = 0u64;
            for (&x, &y) in xs.iter().zip(ys.iter()) {
                acc ^= naive_mul_64(x as u64, y as u64, POLY_64_REDUCE);
            }
            black_box(acc)
        })
    });

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

fn bench_crossover_sweep(c: &mut Criterion) {
    let mut group = c.benchmark_group("gf2m_mul_crossover");

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

        group.bench_with_input(BenchmarkId::new("naive", m), &m, |bench, _| {
            bench.iter(|| naive_mul(black_box(a), black_box(b), m, poly))
        });

        let log_exp = LogExpTable::new(m, poly);
        group.bench_with_input(BenchmarkId::new("log_exp", m), &m, |bench, _| {
            bench.iter(|| log_exp.mul(black_box(a), black_box(b)))
        });

        let field = Gf2mField::new(m, poly).with_tables();
        let ea = field.element(a);
        let eb = field.element(b);
        group.bench_with_input(BenchmarkId::new("gf2m_field", m), &m, |bench, _| {
            bench.iter(|| black_box(&ea) * black_box(&eb))
        });
    }

    group.finish();
}

fn bench_pclmulqdq_barrett(c: &mut Criterion) {
    use gf2_core::gf2m::barrett::BarrettReducer;

    let gf2m_fns = gf2_kernels_simd::gf2m::detect();
    let clmul_barrett_fn = gf2m_fns.as_ref().and_then(|f| f.clmul_barrett_fn);
    let clmul_fn = gf2m_fns.as_ref().and_then(|f| f.clmul_fn);

    if clmul_barrett_fn.is_none() {
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

        {
            let mut group = c.benchmark_group(format!("gf2m_mul_single/{label}"));
            group.throughput(Throughput::Elements(1));
            group.bench_function("pclmulqdq_barrett", |bench| {
                bench.iter(|| clmul_barrett(black_box(a), black_box(b), mu, modulus, degree))
            });
            group.finish();
        }

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

fn bench_gf2m_batch_pclmulqdq_unroll4(c: &mut Criterion) {
    use gf2_core::gf2m::barrett::BarrettReducer;

    let single_fns = gf2_kernels_simd::gf2m::detect();
    let batch_fns = gf2_kernels_simd::gf2m_batch::detect();

    let single_kernel = match single_fns.as_ref().and_then(|f| f.clmul_barrett_fn) {
        Some(f) => f,
        None => return,
    };
    let batch_kernel = match batch_fns.as_ref() {
        Some(f) => f.mul_fn,
        None => return,
    };

    // `dev/scripts/ppc-baselines.json` names the `gf2m_batch_unroll4` and
    // `pclmulqdq_barrett_loop_v0` IDs of this group.
    let sweep_fields: &[(usize, u64)] = &[(8, POLY_8), (16, POLY_16), (32, POLY_32)];
    const BATCH_SIZE: usize = 1024;

    for &(m, poly) in sweep_fields {
        let reducer = BarrettReducer::new(poly as u128, m as u32);
        let mu = reducer.mu() as u64;
        let modulus = reducer.modulus() as u64;
        let degree = reducer.degree();
        let mask = (1u64 << m) - 1;

        let xs = random_elements(BATCH_SIZE, m);
        let ys = random_elements(BATCH_SIZE, m);
        let mut out = vec![0u64; BATCH_SIZE];

        {
            let mut group = c.benchmark_group("gf2m_mul_crossover");
            group.throughput(Throughput::Elements(BATCH_SIZE as u64));
            group.bench_with_input(
                BenchmarkId::new("pclmulqdq_barrett_loop_v0", m),
                &m,
                |bench, _| {
                    bench.iter(|| {
                        out.iter_mut()
                            .zip(xs.iter().zip(ys.iter()))
                            .for_each(|(out, (&x, &y))| {
                                *out = single_kernel(x, y, mu, modulus, degree);
                            });
                        black_box(&out);
                    })
                },
            );
            group.finish();
        }

        {
            let mut group = c.benchmark_group("gf2m_mul_crossover");
            group.throughput(Throughput::Elements(BATCH_SIZE as u64));
            group.bench_with_input(BenchmarkId::new("gf2m_batch_unroll4", m), &m, |bench, _| {
                bench.iter(|| {
                    batch_kernel(&xs, &ys, &mut out, mu, modulus, degree);
                    black_box(&out);
                })
            });
            group.finish();
        }

        let mut single_out = vec![0u64; BATCH_SIZE];
        for i in 0..BATCH_SIZE {
            single_out[i] = single_kernel(xs[i], ys[i], mu, modulus, degree);
        }
        let mut batch_out = vec![0u64; BATCH_SIZE];
        batch_kernel(&xs, &ys, &mut batch_out, mu, modulus, degree);
        for i in 0..BATCH_SIZE {
            assert_eq!(
                single_out[i] & mask,
                batch_out[i] & mask,
                "C1 bench: batch kernel disagrees with single-shot at i={i}, m={m}",
            );
        }
    }
}

/// x^32 + x^22 + x^2 + x + 1.
const POLY_32: u64 = 0b1_0000_0000_0100_0000_0000_0000_0000_0111;

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
    bench_gf2m_batch_pclmulqdq_unroll4,
);
criterion_main!(benches);

#[cfg(test)]
mod tests {
    #[test]
    fn test_mul_strategies_agree_gf2_8() {
        let lut = DirectLut::new(POLY_8);
        let log_exp = LogExpTable::new(8, POLY_8);
        let field = Gf2mField::gf256().with_tables();

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
            if a != 0 && b != 0 {
                assert_eq!(
                    naive_result, field_result,
                    "Naive vs Gf2mField mismatch for ({a}, {b}): {naive_result} != {field_result}"
                );
            }
        }
    }

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

    #[test]
    fn test_mul_gf2_64_identities() {
        let a = 0xDEAD_BEEF_CAFE_BABEu64;
        assert_eq!(naive_mul_64(a, 1, POLY_64_REDUCE), a);
        assert_eq!(naive_mul_64(1, a, POLY_64_REDUCE), a);

        assert_eq!(naive_mul_64(a, 0, POLY_64_REDUCE), 0);
        assert_eq!(naive_mul_64(0, a, POLY_64_REDUCE), 0);

        let b = 0x0123_4567_89AB_CDEFu64;
        assert_eq!(
            naive_mul_64(a, b, POLY_64_REDUCE),
            naive_mul_64(b, a, POLY_64_REDUCE)
        );
    }

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

            let naive_commuted = naive_mul(b, a, 63, poly_63);
            assert_eq!(
                naive_result, naive_commuted,
                "Commutativity failure for naive GF(2^63) ({a:#018x}, {b:#018x})"
            );
        }

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

    #[test]
    fn test_log_exp_table_roundtrip_gf2_8() {
        let table = LogExpTable::new(8, POLY_8);

        for a in 1u64..256 {
            let log_a = table.log_table[a as usize] as usize;
            assert_eq!(table.exp_table[log_a] as u64, a, "exp[log[{a}]] != {a}");
        }

        for i in 0usize..255 {
            let exp_i = table.exp_table[i] as usize;
            assert_eq!(table.log_table[exp_i] as usize, i, "log[exp[{i}]] != {i}");
        }
    }
}
