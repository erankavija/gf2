#include <flint/flint.h>
#include <flint/fmpz_poly.h>
#include <flint/nmod_poly.h>
#include <chrono>
#include <iostream>
#include <iomanip>

using namespace std;
using namespace std::chrono;

void benchmark_poly_multiplication(int degree, flint_rand_t state) {
    nmod_poly_t a, b, c;
    
    // Use modulus 2 for GF(2)
    nmod_poly_init(a, 2);
    nmod_poly_init(b, 2);
    nmod_poly_init(c, 2);
    
    // Generate random polynomials
    nmod_poly_randtest(a, state, degree);
    nmod_poly_randtest(b, state, degree);
    
    // Warm up
    nmod_poly_mul(c, a, b);
    
    // Benchmark
    int iterations = (degree <= 100) ? 10000 : (degree <= 500) ? 1000 : 100;
    
    auto start = high_resolution_clock::now();
    for (int i = 0; i < iterations; i++) {
        nmod_poly_mul(c, a, b);
    }
    auto end = high_resolution_clock::now();
    
    auto duration = duration_cast<nanoseconds>(end - start).count();
    double us_per_op = static_cast<double>(duration) / iterations / 1000.0;
    
    cout << "Polynomial multiplication (degree " << degree << "): " 
         << fixed << setprecision(2) << us_per_op << " µs/op" << endl;
    
    nmod_poly_clear(a);
    nmod_poly_clear(b);
    nmod_poly_clear(c);
}

void benchmark_poly_gcd(int degree, flint_rand_t state) {
    nmod_poly_t a, b, g;
    
    nmod_poly_init(a, 2);
    nmod_poly_init(b, 2);
    nmod_poly_init(g, 2);
    
    // Generate random polynomials
    nmod_poly_randtest(a, state, degree);
    nmod_poly_randtest(b, state, degree);
    
    // Warm up
    nmod_poly_gcd(g, a, b);
    
    // Benchmark
    int iterations = (degree <= 100) ? 1000 : 100;
    
    auto start = high_resolution_clock::now();
    for (int i = 0; i < iterations; i++) {
        nmod_poly_gcd(g, a, b);
    }
    auto end = high_resolution_clock::now();
    
    auto duration = duration_cast<nanoseconds>(end - start).count();
    double us_per_op = static_cast<double>(duration) / iterations / 1000.0;
    
    cout << "Polynomial GCD (degree " << degree << "): " 
         << fixed << setprecision(2) << us_per_op << " µs/op" << endl;
    
    nmod_poly_clear(a);
    nmod_poly_clear(b);
    nmod_poly_clear(g);
}

void benchmark_poly_evaluation(int degree, flint_rand_t state) {
    nmod_poly_t p;
    nmod_poly_init(p, 2);
    
    nmod_poly_randtest(p, state, degree);
    
    // Warm up
    ulong result = nmod_poly_evaluate_nmod(p, 1);
    
    // Benchmark
    int iterations = (degree <= 100) ? 100000 : 10000;
    
    auto start = high_resolution_clock::now();
    for (int i = 0; i < iterations; i++) {
        result = nmod_poly_evaluate_nmod(p, 1);
    }
    auto end = high_resolution_clock::now();
    
    auto duration = duration_cast<nanoseconds>(end - start).count();
    double ns_per_op = static_cast<double>(duration) / iterations;
    
    cout << "Polynomial evaluation (degree " << degree << "): " 
         << fixed << setprecision(2) << ns_per_op << " ns/op" << endl;
    
    nmod_poly_clear(p);
}

int main(int argc, char* argv[]) {
    cout << "=== FLINT Polynomial Operations Benchmark ===" << endl;
    cout << "FLINT Version: " << FLINT_VERSION << endl << endl;
    
    // Initialize random state
    flint_rand_t state;
    flint_randinit(state);
    
    // Test different polynomial degrees
    int degrees[] = {50, 100, 200, 500};
    
    for (int degree : degrees) {
        cout << "--- Degree: " << degree << " ---" << endl;
        benchmark_poly_multiplication(degree, state);
        benchmark_poly_gcd(degree, state);
        benchmark_poly_evaluation(degree, state);
        cout << endl;
    }
    
    flint_randclear(state);
    return 0;
}
