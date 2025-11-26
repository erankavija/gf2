#include <m4ri/m4ri.h>
#include <chrono>
#include <iostream>
#include <iomanip>

using namespace std;
using namespace std::chrono;

// Benchmark matrix-vector multiplication: y = A * x
void benchmark_matvec(int rows, int cols) {
    // Create random matrix A (rows x cols)
    mzd_t *A = mzd_init(rows, cols);
    mzd_randomize(A);
    
    // Create random vector x (represented as cols x 1 matrix)
    mzd_t *x = mzd_init(cols, 1);
    mzd_randomize(x);
    
    // Create output vector y (rows x 1)
    mzd_t *y = mzd_init(rows, 1);
    
    // Warm up
    mzd_mul(y, A, x, 0);
    
    // Benchmark (100 iterations for better measurement)
    const int iterations = 100;
    auto start = high_resolution_clock::now();
    for (int i = 0; i < iterations; i++) {
        mzd_mul(y, A, x, 0);
    }
    auto end = high_resolution_clock::now();
    
    auto total_us = duration_cast<microseconds>(end - start).count();
    double avg_us = total_us / double(iterations);
    
    cout << "M4RI matvec (" << rows << "x" << cols << "): ";
    if (avg_us < 1000.0) {
        cout << fixed << setprecision(2) << avg_us << " µs" << endl;
    } else {
        cout << fixed << setprecision(2) << avg_us / 1000.0 << " ms" << endl;
    }
    
    mzd_free(A);
    mzd_free(x);
    mzd_free(y);
}

// Benchmark transpose matrix-vector multiplication: y = A^T * x
void benchmark_matvec_transpose(int rows, int cols) {
    // Create random matrix A (rows x cols)
    mzd_t *A = mzd_init(rows, cols);
    mzd_randomize(A);
    
    // Transpose A to get A^T (cols x rows)
    mzd_t *AT = mzd_transpose(NULL, A);
    
    // Create random vector x (represented as rows x 1 matrix)
    mzd_t *x = mzd_init(rows, 1);
    mzd_randomize(x);
    
    // Create output vector y (cols x 1)
    mzd_t *y = mzd_init(cols, 1);
    
    // Warm up
    mzd_mul(y, AT, x, 0);
    
    // Benchmark (100 iterations for better measurement)
    const int iterations = 100;
    auto start = high_resolution_clock::now();
    for (int i = 0; i < iterations; i++) {
        mzd_mul(y, AT, x, 0);
    }
    auto end = high_resolution_clock::now();
    
    auto total_us = duration_cast<microseconds>(end - start).count();
    double avg_us = total_us / double(iterations);
    
    cout << "M4RI matvec_transpose (" << rows << "x" << cols << "): ";
    if (avg_us < 1000.0) {
        cout << fixed << setprecision(2) << avg_us << " µs" << endl;
    } else {
        cout << fixed << setprecision(2) << avg_us / 1000.0 << " ms" << endl;
    }
    
    mzd_free(A);
    mzd_free(AT);
    mzd_free(x);
    mzd_free(y);
}

// Benchmark at different densities (approximated by random matrices)
void benchmark_by_density(int size) {
    cout << "\n=== Matrix-Vector at Different Densities (size=" << size << ") ===" << endl;
    
    // For M4RI, we can't directly control density, but we can note that
    // random matrices are approximately 50% dense
    mzd_t *A = mzd_init(size, size);
    mzd_randomize(A);
    
    mzd_t *x = mzd_init(size, 1);
    mzd_randomize(x);
    
    mzd_t *y = mzd_init(size, 1);
    
    // Warm up
    mzd_mul(y, A, x, 0);
    
    // Benchmark
    const int iterations = 100;
    auto start = high_resolution_clock::now();
    for (int i = 0; i < iterations; i++) {
        mzd_mul(y, A, x, 0);
    }
    auto end = high_resolution_clock::now();
    
    auto total_us = duration_cast<microseconds>(end - start).count();
    double avg_us = total_us / double(iterations);
    
    cout << "M4RI matvec (random ~50% dense): ";
    if (avg_us < 1000.0) {
        cout << fixed << setprecision(2) << avg_us << " µs" << endl;
    } else {
        cout << fixed << setprecision(2) << avg_us / 1000.0 << " ms" << endl;
    }
    
    mzd_free(A);
    mzd_free(x);
    mzd_free(y);
}

int main() {
    cout << "=== M4RI Matrix-Vector Multiplication Benchmarks ===" << endl;
    cout << endl;
    
    cout << "=== Square Matrices (matvec) ===" << endl;
    benchmark_matvec(64, 64);
    benchmark_matvec(128, 128);
    benchmark_matvec(256, 256);
    benchmark_matvec(512, 512);
    benchmark_matvec(1024, 1024);
    
    cout << "\n=== Square Matrices (matvec_transpose) ===" << endl;
    benchmark_matvec_transpose(64, 64);
    benchmark_matvec_transpose(128, 128);
    benchmark_matvec_transpose(256, 256);
    benchmark_matvec_transpose(512, 512);
    benchmark_matvec_transpose(1024, 1024);
    
    cout << "\n=== Rectangular Matrices ===" << endl;
    benchmark_matvec(100, 1000);
    benchmark_matvec(500, 100);
    benchmark_matvec(1000, 1000);
    
    cout << "\n=== Rectangular Transpose ===" << endl;
    benchmark_matvec_transpose(100, 1000);
    benchmark_matvec_transpose(500, 100);
    benchmark_matvec_transpose(1000, 1000);
    
    benchmark_by_density(512);
    
    return 0;
}
