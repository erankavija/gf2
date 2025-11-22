#include <m4ri/m4ri.h>
#include <chrono>
#include <iostream>
#include <iomanip>

using namespace std;
using namespace std::chrono;

void benchmark_multiplication(int n) {
    // Create random matrices
    mzd_t *A = mzd_init(n, n);
    mzd_t *B = mzd_init(n, n);
    mzd_t *C = mzd_init(n, n);
    
    mzd_randomize(A);
    mzd_randomize(B);
    
    // Warm up
    mzd_mul(C, A, B, 0);
    
    // Benchmark
    auto start = high_resolution_clock::now();
    mzd_mul(C, A, B, 0);
    auto end = high_resolution_clock::now();
    
    auto duration = duration_cast<microseconds>(end - start).count();
    
    cout << "M4RI matrix multiplication (" << n << "x" << n << "): " 
         << fixed << setprecision(2) << duration / 1000.0 << " ms" << endl;
    
    mzd_free(A);
    mzd_free(B);
    mzd_free(C);
}

void benchmark_gaussian_elimination(int n) {
    mzd_t *A = mzd_init(n, n);
    mzd_randomize(A);
    
    // Warm up
    mzd_t *A_copy = mzd_copy(NULL, A);
    mzd_echelonize(A_copy, 0);
    mzd_free(A_copy);
    
    // Benchmark
    A_copy = mzd_copy(NULL, A);
    auto start = high_resolution_clock::now();
    mzd_echelonize(A_copy, 0);
    auto end = high_resolution_clock::now();
    
    auto duration = duration_cast<microseconds>(end - start).count();
    
    cout << "M4RI Gaussian elimination (" << n << "x" << n << "): " 
         << fixed << setprecision(2) << duration / 1000.0 << " ms" << endl;
    
    mzd_free(A);
    mzd_free(A_copy);
}

void benchmark_inversion(int n) {
    // Try a few times to get an invertible matrix
    mzd_t *A = NULL;
    for (int attempt = 0; attempt < 10; attempt++) {
        A = mzd_init(n, n);
        mzd_randomize(A);
        
        // Check rank
        mzd_t *rank_test = mzd_copy(NULL, A);
        int rank = mzd_echelonize(rank_test, 0);
        mzd_free(rank_test);
        
        if (rank == n) {
            break; // Found full rank matrix
        }
        mzd_free(A);
        A = NULL;
    }
    
    if (A == NULL) {
        cout << "M4RI matrix inversion (" << n << "x" << n << "): " 
             << "Failed to generate invertible matrix" << endl;
        return;
    }
    
    // Create identity matrix
    mzd_t *I = mzd_init(n, n);
    for (int i = 0; i < n; i++) {
        mzd_write_bit(I, i, i, 1);
    }
    
    // Create output matrix
    mzd_t *INV = mzd_init(n, n);
    mzd_t *A_copy = mzd_copy(NULL, A);
    
    // Warm up
    mzd_invert_naive(INV, A_copy, I);
    mzd_free(A_copy);
    
    // Benchmark
    A_copy = mzd_copy(NULL, A);
    auto start = high_resolution_clock::now();
    mzd_t *result = mzd_invert_naive(INV, A_copy, I);
    auto end = high_resolution_clock::now();
    
    auto duration = duration_cast<microseconds>(end - start).count();
    
    if (result != NULL) {
        cout << "M4RI matrix inversion (" << n << "x" << n << "): " 
             << fixed << setprecision(2) << duration / 1000.0 << " ms" << endl;
    } else {
        cout << "M4RI matrix inversion (" << n << "x" << n << "): Failed" << endl;
    }
    
    mzd_free(A);
    mzd_free(A_copy);
    mzd_free(I);
    mzd_free(INV);
}

int main(int argc, char* argv[]) {
    cout << "=== M4RI Matrix Operations Benchmark ===" << endl;
    cout << "M4RI Version: (installed)" << endl << endl;
    
    // Test different matrix sizes
    int sizes[] = {256, 512, 1024, 2048};
    
    for (int n : sizes) {
        cout << "--- Size: " << n << "x" << n << " ---" << endl;
        benchmark_multiplication(n);
        benchmark_gaussian_elimination(n);
        if (n <= 1024) { // Inversion is expensive
            benchmark_inversion(n);
        }
        cout << endl;
    }
    
    return 0;
}
