#include <NTL/GF2E.h>
#include <NTL/GF2X.h>
#include <NTL/GF2XFactoring.h>
#include <NTL/version.h>
#include <chrono>
#include <iostream>
#include <iomanip>

using namespace NTL;
using namespace std;
using namespace std::chrono;

void benchmark_multiplication(int m, int iterations) {
    // Build primitive polynomial
    GF2X modulus = BuildIrred_GF2X(m);
    GF2E::init(modulus);
    
    // Generate random elements
    GF2E a = random_GF2E();
    GF2E b = random_GF2E();
    GF2E c;
    
    // Warm up
    for (int i = 0; i < 1000; i++) {
        c = a * b;
    }
    
    // Benchmark
    auto start = high_resolution_clock::now();
    for (int i = 0; i < iterations; i++) {
        c = a * b;
    }
    auto end = high_resolution_clock::now();
    
    auto duration = duration_cast<nanoseconds>(end - start).count();
    double ns_per_op = static_cast<double>(duration) / iterations;
    
    cout << "GF(2^" << m << ") multiplication: " 
         << fixed << setprecision(2) << ns_per_op << " ns/op" << endl;
}

void benchmark_inversion(int m, int iterations) {
    GF2X modulus = BuildIrred_GF2X(m);
    GF2E::init(modulus);
    
    GF2E a = random_GF2E();
    while (IsZero(a)) a = random_GF2E();
    GF2E inv;
    
    // Warm up
    for (int i = 0; i < 100; i++) {
        inv = NTL::inv(a);
    }
    
    auto start = high_resolution_clock::now();
    for (int i = 0; i < iterations; i++) {
        inv = NTL::inv(a);
    }
    auto end = high_resolution_clock::now();
    
    auto duration = duration_cast<nanoseconds>(end - start).count();
    double ns_per_op = static_cast<double>(duration) / iterations;
    
    cout << "GF(2^" << m << ") inversion: " 
         << fixed << setprecision(2) << ns_per_op << " ns/op" << endl;
}

void benchmark_exponentiation(int m, int iterations) {
    GF2X modulus = BuildIrred_GF2X(m);
    GF2E::init(modulus);
    
    GF2E a = random_GF2E();
    GF2E result;
    ZZ exp;
    exp = (1L << (m/2)); // 2^(m/2)
    
    // Warm up
    for (int i = 0; i < 10; i++) {
        power(result, a, exp);
    }
    
    auto start = high_resolution_clock::now();
    for (int i = 0; i < iterations; i++) {
        power(result, a, exp);
    }
    auto end = high_resolution_clock::now();
    
    auto duration = duration_cast<nanoseconds>(end - start).count();
    double ns_per_op = static_cast<double>(duration) / iterations;
    
    cout << "GF(2^" << m << ") exponentiation (exp=2^" << m/2 << "): " 
         << fixed << setprecision(2) << ns_per_op << " ns/op" << endl;
}

int main(int argc, char* argv[]) {
    cout << "=== NTL Field Operations Benchmark ===" << endl;
    cout << "NTL Version: " << NTL_MAJOR_VERSION << "." 
         << NTL_MINOR_VERSION << "." << NTL_REVISION << endl << endl;
    
    // Test different field sizes
    int field_sizes[] = {4, 8, 16, 32, 64};
    
    for (int m : field_sizes) {
        cout << "--- m = " << m << " ---" << endl;
        
        // Adjust iterations based on field size
        int mult_iters = (m <= 16) ? 1000000 : (m <= 32) ? 100000 : 10000;
        int inv_iters = (m <= 16) ? 10000 : (m <= 32) ? 1000 : 100;
        int exp_iters = (m <= 16) ? 10000 : (m <= 32) ? 1000 : 100;
        
        benchmark_multiplication(m, mult_iters);
        benchmark_inversion(m, inv_iters);
        benchmark_exponentiation(m, exp_iters);
        cout << endl;
    }
    
    return 0;
}
