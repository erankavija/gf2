// External-baseline harness: IT++ binary BCH encoding.
//
// Workload W1 of the ae03bcd0 workload-selection contract, by the polynomial-
// remainder family over GF(2^m) field elements -- the same algorithmic shape
// the pre-cutover gf2 encoder uses, which is why this candidate is measured
// rather than reasoned about.
//
// itpp::BCH::encode is natively a batch call: it splits its input into
// floor(len / k) messages and encodes each. The whole batch is one call.
//
// Only full-length primitive rows are measurable: itpp::BCH represents
// polynomials over GF(n+1), so it requires n + 1 to be a power of two
// (itpp/comm/bch.cpp:60). Shortened rows are reported and skipped.
//
// The installed IT++ headers leave PACKAGE_VERSION empty, so the version this
// harness reports is derived at build time from the installed shared object's
// soname rather than from a header constant.
//
// Output: CSV on stdout, provenance preamble on stderr.

#ifndef ITPP_VERSION_STR
#error "ITPP_VERSION_STR must be defined by the build; see the Makefile"
#endif

#include <itpp/itbase.h>
#include <itpp/itcomm.h>

#include <algorithm>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <random>
#include <string>
#include <vector>

// ---------------------------------------------------------------- protocol
static const int TRIALS = 7;
static const int WARMUP_REPS = 2;
static const uint64_t SEED = 0xAE03BCD0ull;
static const double MIN_TIMED_NS = 5e6;
static const double CELL_BUDGET_S = 90.0;

struct CodeSpec
{
    const char* name;
    int t, k, n;
};

// The same rows as the sibling harnesses.
static const CodeSpec CODES[] = {
    { "B1", 3, 5, 15 },
    { "B2", 10, 64, 127 },
    { "B3", 4, 223, 255 },
    { "T2S", 12, 7032, 7200 },
    { "T2N", 12, 32208, 32400 },
};
static const int N_CODES = (int)(sizeof(CODES) / sizeof(CODES[0]));

static const int BATCHES[] = { 1, 16, 256, 4096 };
static const int N_BATCHES = (int)(sizeof(BATCHES) / sizeof(BATCHES[0]));

static bool
is_pow2_minus_one(int n)
{
    const int v = n + 1;
    return v > 0 && (v & (v - 1)) == 0;
}

int
main()
{
    const char* only_env = std::getenv("GF2_SURVEY_CODES");
    const std::string only = only_env ? only_env : "";

    std::fprintf(stderr, "# harness: itpp_bch_bench\n");
    std::fprintf(stderr, "# itpp_version: %s\n", ITPP_VERSION_STR);
    std::fprintf(stderr, "# trials_per_cell: %d\n", TRIALS);
    std::fprintf(stderr, "# cell_budget_s: %.1f\n", CELL_BUDGET_S);
    std::fprintf(stderr, "# seed: 0x%016llx\n", (unsigned long long)SEED);
    if (!only.empty())
        std::fprintf(stderr, "# codes_selected: %s\n", only.c_str());

    std::printf("lib,version,workload,algorithm,code,n,k,t,batch,trial,ns_per_frame,info_mbit_per_s,digest\n");

    for (int ci = 0; ci < N_CODES; ci++)
    {
        const CodeSpec& cs = CODES[ci];
        if (!only.empty() && only.find(cs.name) == std::string::npos)
            continue;

        if (!is_pow2_minus_one(cs.n))
        {
            std::fprintf(stderr,
                         "# SKIP %s: n=%d is shortened; itpp::BCH requires n+1 to be a power of two\n",
                         cs.name,
                         cs.n);
            continue;
        }

        itpp::BCH bch(cs.n, cs.t, /*sys=*/true);
        if (bch.get_k() != cs.k)
        {
            std::fprintf(stderr,
                         "# SKIP %s: itpp derived k=%d, contract row has k=%d\n",
                         cs.name,
                         bch.get_k(),
                         cs.k);
            continue;
        }
        std::fprintf(stderr, "# code %s: n=%d k=%d t=%d\n", cs.name, cs.n, bch.get_k(), cs.t);

        for (int bi = 0; bi < N_BATCHES; bi++)
        {
            const int batch = BATCHES[bi];

            std::mt19937_64 rng(SEED ^ ((uint64_t)batch << 32) ^ (uint64_t)cs.n);
            itpp::bvec in((int)((size_t)batch * cs.k));
            for (int i = 0; i < in.length(); i++)
                in(i) = (itpp::bin)(rng() & 1ull);
            itpp::bvec out;

            for (int w = 0; w < WARMUP_REPS; w++)
                bch.encode(in, out);

            uint64_t digest = 1469598103934665603ull;
            for (int i = 0; i < out.length(); i++)
            {
                digest ^= (uint64_t)(int)out(i);
                digest *= 1099511628211ull;
            }

            const auto c0 = std::chrono::steady_clock::now();
            bch.encode(in, out);
            const auto c1 = std::chrono::steady_clock::now();
            const double one_ns = std::chrono::duration<double, std::nano>(c1 - c0).count();
            long reps = (long)(MIN_TIMED_NS / (one_ns > 0.0 ? one_ns : 1.0)) + 1;
            if (reps < 1)
                reps = 1;

            double spent = 0.0;
            for (int trial = 0; trial < TRIALS && spent < CELL_BUDGET_S; trial++)
            {
                const auto t0 = std::chrono::steady_clock::now();
                for (long rep = 0; rep < reps; rep++)
                    bch.encode(in, out);
                const auto t1 = std::chrono::steady_clock::now();

                const double total_ns = std::chrono::duration<double, std::nano>(t1 - t0).count();
                spent += total_ns * 1e-9;
                const double ns = total_ns / (double)reps;

                std::printf("itpp,%s,W1,poly-remainder-gfx,%s,%d,%d,%d,%d,%d,%.3f,%.4f,%016llx\n",
                            ITPP_VERSION_STR,
                            cs.name,
                            cs.n,
                            cs.k,
                            cs.t,
                            batch,
                            trial,
                            ns / (double)batch,
                            ((double)batch * (double)cs.k) / ns * 1e3,
                            (unsigned long long)digest);
            }
            std::fprintf(stderr, "#   %s poly-remainder-gfx batch=%-5d reps=%ld spent=%.2fs\n", cs.name, batch, reps,
                         spent);
            std::fflush(stdout);
        }
    }

    return 0;
}
