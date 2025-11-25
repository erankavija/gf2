// Focused M4RI Gaussian Elimination Benchmark for RREF Implementation
// Tests both standard sizes and DVB-T2 LDPC matrix sizes

#include <m4ri/m4ri.h>
#include <chrono>
#include <iostream>
#include <iomanip>
#include <vector>
#include <algorithm>
#include <cmath>

using namespace std;
using namespace std::chrono;

struct BenchmarkResult {
    int rows;
    int cols;
    double time_ms;
    int rank;
    bool success;
};

BenchmarkResult benchmark_gaussian_elimination(int m, int n, int iterations = 5) {
    BenchmarkResult result = {m, n, 0.0, 0, false};
    
    cout << "  Testing " << m << "x" << n << " matrix..." << flush;
    
    // Create random matrix
    mzd_t *A = mzd_init(m, n);
    mzd_randomize(A);
    
    // Warm up
    mzd_t *A_copy = mzd_copy(NULL, A);
    mzd_echelonize(A_copy, 0);
    mzd_free(A_copy);
    
    // Benchmark multiple iterations
    vector<double> times;
    int final_rank = 0;
    
    for (int i = 0; i < iterations; i++) {
        A_copy = mzd_copy(NULL, A);
        
        auto start = high_resolution_clock::now();
        final_rank = mzd_echelonize(A_copy, 0);
        auto end = high_resolution_clock::now();
        
        auto duration = duration_cast<microseconds>(end - start).count();
        times.push_back(duration / 1000.0);
        
        mzd_free(A_copy);
    }
    
    // Calculate median time
    sort(times.begin(), times.end());
    double median_time = times[times.size() / 2];
    
    result.time_ms = median_time;
    result.rank = final_rank;
    result.success = true;
    
    cout << " " << fixed << setprecision(2) << median_time << " ms (rank=" << final_rank << ")" << endl;
    
    mzd_free(A);
    return result;
}

void print_summary_table(const vector<BenchmarkResult>& results) {
    cout << "\n=== Summary Table ===" << endl;
    cout << "| Size (m×n) | Time (ms) | Throughput (ops/sec) | Rank |" << endl;
    cout << "|------------|-----------|----------------------|------|" << endl;
    
    for (const auto& r : results) {
        if (r.success) {
            double ops_per_sec = (r.time_ms > 0) ? 1000.0 / r.time_ms : 0.0;
            
            cout << "| " << setw(4) << r.rows << "×" << setw(5) << left << r.cols << " | "
                 << setw(9) << fixed << setprecision(2) << r.time_ms << " | "
                 << setw(20) << fixed << setprecision(1) << ops_per_sec << " | "
                 << setw(4) << r.rank << " |" << endl;
        }
    }
    cout << endl;
}

int main(int argc, char* argv[]) {
    cout << "=== M4RI Gaussian Elimination Baseline Benchmark ===" << endl;
    cout << "Purpose: Establish competitive baseline for gf2-core RREF implementation" << endl;
    cout << "Algorithm: M4RI mzd_echelonize() - row echelon form computation" << endl;
    cout << endl;
    
    vector<BenchmarkResult> results;
    
    // Standard benchmark sizes (square matrices)
    cout << "--- Standard Sizes (Square Matrices) ---" << endl;
    vector<int> standard_sizes = {256, 512, 1024, 2048};
    
    for (int n : standard_sizes) {
        results.push_back(benchmark_gaussian_elimination(n, n, 5));
    }
    
    cout << endl;
    
    // Medium rectangular matrices
    cout << "--- Medium Rectangular Matrices ---" << endl;
    results.push_back(benchmark_gaussian_elimination(100, 200, 10));
    results.push_back(benchmark_gaussian_elimination(500, 1000, 5));
    results.push_back(benchmark_gaussian_elimination(1000, 2000, 3));
    
    cout << endl;
    
    // DVB-T2 LDPC sizes (if system has enough memory)
    cout << "--- DVB-T2 LDPC Matrix Sizes ---" << endl;
    cout << "These are the actual use case matrices for LDPC encoding." << endl;
    cout << "Warning: Large matrices may require significant memory and time." << endl;
    cout << endl;
    
    // DVB-T2 Short Rate 3/5: 6,480 × 16,200
    cout << "DVB-T2 Short Rate 3/5:" << endl;
    results.push_back(benchmark_gaussian_elimination(6480, 16200, 3));
    
    // DVB-T2 Short Rate 1/2: 9,000 × 16,200
    cout << "DVB-T2 Short Rate 1/2:" << endl;
    results.push_back(benchmark_gaussian_elimination(9000, 16200, 3));
    
    // DVB-T2 Normal (optional - very large, may OOM)
    cout << "\nDVB-T2 Normal Rate 1/2 (32,400×64,800):" << endl;
    cout << "  Skipped (requires ~260MB RAM, ~1-2 minutes)" << endl;
    cout << "  To enable, uncomment in source and rebuild." << endl;
    // Uncomment to test (warning: slow and memory-intensive):
    // results.push_back(benchmark_gaussian_elimination(32400, 64800, 1));
    
    // Print summary
    print_summary_table(results);
    
    // Print performance characteristics
    cout << "=== Performance Characteristics ===" << endl;
    cout << "M4RI uses optimized algorithms including:" << endl;
    cout << "  - Gray code tables for efficient row operations" << endl;
    cout << "  - Cache-aware blocking strategies" << endl;
    cout << "  - Method of Four Russians (M4R) optimization" << endl;
    cout << "  - Hand-tuned assembly in critical paths" << endl;
    cout << endl;
    cout << "For gf2-core RREF implementation:" << endl;
    cout << "  - Target: Within 2-3× of M4RI for dense matrices" << endl;
    cout << "  - Primary goal: <10s for DVB-T2 Short, <60s for DVB-T2 Normal" << endl;
    cout << endl;
    
    return 0;
}
