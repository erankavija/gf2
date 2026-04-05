// Benchmark FFLAS::fdot for different prime sizes and vector lengths.
//
// Compares against gf2-core's FieldVec::dot_product (see fieldvec_dot_product.rs).
//
// Build:
//   ./benches/build_fflas_bench.sh
//
// Run:
//   ./benches/fflas_fdot_bench

#include <iostream>
#include <iomanip>
#include <chrono>
#include <vector>
#include <random>
#include <cstdint>

#include <givaro/modular.h>
#include <fflas-ffpack/fflas/fflas.h>

// Number of warm-up iterations before timing.
static constexpr int WARMUP = 5;

// Number of timed iterations.
static constexpr int ITERS = 100;

// Benchmark fdot for Modular<int64_t>.
static void bench_fdot_int64(const char* label, int64_t prime, size_t n) {
    using Field = Givaro::Modular<int64_t>;
    Field F(prime);

    typename Field::Element_ptr a = FFLAS::fflas_new(F, n);
    typename Field::Element_ptr b = FFLAS::fflas_new(F, n);

    std::mt19937_64 rng(42);
    for (size_t i = 0; i < n; ++i) {
        int64_t va = static_cast<int64_t>(rng() % static_cast<uint64_t>(prime));
        int64_t vb = static_cast<int64_t>(rng() % static_cast<uint64_t>(prime));
        F.init(a[i], va);
        F.init(b[i], vb);
    }

    typename Field::Element result;
    F.init(result, 0);

    // Warm up.
    for (int i = 0; i < WARMUP; ++i) {
        result = FFLAS::fdot(F, n, a, 1, b, 1);
    }

    // Timed iterations.
    auto start = std::chrono::high_resolution_clock::now();
    for (int i = 0; i < ITERS; ++i) {
        result = FFLAS::fdot(F, n, a, 1, b, 1);
    }
    auto end = std::chrono::high_resolution_clock::now();

    volatile auto sink = result;
    (void)sink;

    double total_ns = std::chrono::duration<double, std::nano>(end - start).count();
    double ns_per_elem = total_ns / (static_cast<double>(ITERS) * static_cast<double>(n));

    std::cout << std::left << std::setw(32) << label
              << "  n=" << std::setw(6) << n
              << "  " << std::fixed << std::setprecision(2) << ns_per_elem << " ns/elem"
              << std::endl;

    FFLAS::fflas_delete(a);
    FFLAS::fflas_delete(b);
}

// Benchmark fdot for Modular<double> (BLAS-accelerated path for small primes).
static void bench_fdot_double(const char* label, double prime, size_t n) {
    using Field = Givaro::Modular<double>;
    Field F(prime);

    typename Field::Element_ptr a = FFLAS::fflas_new(F, n);
    typename Field::Element_ptr b = FFLAS::fflas_new(F, n);

    std::mt19937_64 rng(42);
    for (size_t i = 0; i < n; ++i) {
        double va = static_cast<double>(rng() % static_cast<uint64_t>(prime));
        double vb = static_cast<double>(rng() % static_cast<uint64_t>(prime));
        F.init(a[i], va);
        F.init(b[i], vb);
    }

    typename Field::Element result;
    F.init(result, 0.0);

    for (int i = 0; i < WARMUP; ++i) {
        result = FFLAS::fdot(F, n, a, 1, b, 1);
    }

    auto start = std::chrono::high_resolution_clock::now();
    for (int i = 0; i < ITERS; ++i) {
        result = FFLAS::fdot(F, n, a, 1, b, 1);
    }
    auto end = std::chrono::high_resolution_clock::now();

    volatile auto sink = result;
    (void)sink;

    double total_ns = std::chrono::duration<double, std::nano>(end - start).count();
    double ns_per_elem = total_ns / (static_cast<double>(ITERS) * static_cast<double>(n));

    std::cout << std::left << std::setw(32) << label
              << "  n=" << std::setw(6) << n
              << "  " << std::fixed << std::setprecision(2) << ns_per_elem << " ns/elem"
              << std::endl;

    FFLAS::fflas_delete(a);
    FFLAS::fflas_delete(b);
}

int main() {
    std::cout << "fflas-ffpack fdot benchmark" << std::endl;
    std::cout << "Iterations per measurement: " << ITERS << std::endl;
    std::cout << std::string(64, '-') << std::endl;

    // Primes matching the Rust benchmarks in fieldvec_dot_product.rs.
    static constexpr int64_t SMALL_PRIME = 65521;             // 16-bit
    static constexpr int64_t MERSENNE_31 = (1LL << 31) - 1;  // 2^31-1

    // Note: Givaro::Modular<int64_t> max cardinality is ~2^31 due to delayed
    // reduction needing room for accumulation.  The large prime (~2^62) used in
    // the Rust benchmarks cannot be represented — this is a fundamental
    // limitation of fflas-ffpack's integer field types.  gf2-core handles it
    // via Montgomery multiplication with explicit chunked reduction.

    static constexpr size_t LENGTHS[] = {100, 1000, 10000};

    // --- Modular<double> for p=65521 (BLAS ddot path) ---
    std::cout << "\n--- Modular<double>, p=65521 (BLAS ddot path) ---" << std::endl;
    for (size_t n : LENGTHS) {
        bench_fdot_double("fdot<double>/p=65521", 65521.0, n);
    }

    // --- Modular<int64_t> for p=65521 ---
    std::cout << "\n--- Modular<int64_t>, p=65521 ---" << std::endl;
    for (size_t n : LENGTHS) {
        bench_fdot_int64("fdot<int64>/p=65521", SMALL_PRIME, n);
    }

    // --- Modular<int64_t> for p=2^31-1 ---
    std::cout << "\n--- Modular<int64_t>, p=2^31-1 ---" << std::endl;
    for (size_t n : LENGTHS) {
        bench_fdot_int64("fdot<int64>/p=2^31-1", MERSENNE_31, n);
    }

    std::cout << "\n--- p=~2^62: NOT SUPPORTED ---" << std::endl;
    std::cout << "Modular<int64_t> max cardinality is ~2^31; primes near 2^62" << std::endl;
    std::cout << "require Modular<Integer> (GMP), which has ~100x overhead." << std::endl;
    std::cout << "gf2-core handles this range natively via Montgomery arithmetic." << std::endl;

    return 0;
}
