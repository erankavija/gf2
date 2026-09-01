// External-baseline harness: AFF3CT binary BCH encoding.
//
// Measures the two workloads of the ae03bcd0 workload-selection contract
// against AFF3CT's BCH encoders:
//
//   W1 batch-encode  -- module::Encoder_BCH<int>       (scalar LFSR)
//                       module::Encoder_BCH_inter<int> (MIPP SIMD, interleaved)
//   W2 genmatrix     -- materialize the k x n systematic generator matrix by
//                       encoding the k unit messages and packing rows into
//                       64-bit words.
//
// Every fact the harness prints is observed at run time or is one of the
// protocol constants declared in this file; nothing is transcribed from a
// prior run.
//
// Output: CSV on stdout, provenance preamble on stderr.

#include <algorithm>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <numeric>
#include <random>
#include <string>
#include <vector>

#include <mipp.h>

#include "Module/Encoder/BCH/Encoder_BCH.hpp"
#include "Module/Encoder/BCH/Encoder_BCH_inter.hpp"
#include "Tools/Code/BCH/BCH_polynomial_generator.hpp"
#include "Tools/version.h"

using namespace aff3ct;

// ---------------------------------------------------------------- protocol
// Protocol constants. These are the harness's own declared parameters; the
// survey document restates them and no other source defines them.
static const int TRIALS = 7;      // independent trials per cell
static const int WARMUP_REPS = 2; // untimed repetitions before each cell
static const uint64_t SEED = 0xAE03BCD0ull;
// Each timed region repeats the measured call until it spans at least this
// long, so a cell whose single call is near the clock's resolution is still
// resolved; the reported figure is always per one call.
static const double MIN_TIMED_NS = 5e6;
// Whole-cell wall budget. A cell takes fewer than TRIALS trials when one
// repetition already costs more than the budget allows; the trial count of
// every cell is recorded in its rows.
static const double CELL_BUDGET_S = 90.0;

struct CodeSpec
{
    const char* name; // contract row label
    int m;            // extension degree of the mother field GF(2^m)
    int t;            // designed correction power
    int k;            // shortened message length (information bits)
    int n;            // shortened codeword length
    uint32_t prim;    // primitive polynomial of GF(2^m), bit i = coeff of x^i
};

// The five W1/W2 rows of the workload-selection contract. `prim` matches
// gf2-core's primitive_polys.rs::standard(m) so the generator polynomial --
// and hence the encoding work -- is identical on both sides.
static const CodeSpec CODES[] = {
    { "B1", 4, 3, 5, 15, 0b10011u },
    { "B2", 7, 10, 64, 127, 0b10000011u },
    { "B3", 8, 4, 223, 255, 0b100011101u },
    { "T2S", 14, 12, 7032, 7200, 0b100000000101011u },
    { "T2N", 16, 12, 32208, 32400, 0b10000000000101101u },
};
static const int N_CODES = (int)(sizeof(CODES) / sizeof(CODES[0]));

static const int BATCHES[] = { 1, 16, 256, 4096 };
static const int N_BATCHES = (int)(sizeof(BATCHES) / sizeof(BATCHES[0]));

// ------------------------------------------------------------------ helpers

// AFF3CT's Galois takes the primitive polynomial as m+1 coefficients,
// p[i] = coefficient of x^i.
static std::vector<int>
prim_coeffs(int m, uint32_t poly)
{
    std::vector<int> p(m + 1, 0);
    for (int i = 0; i <= m; i++)
        p[i] = (int)((poly >> i) & 1u);
    return p;
}

static double
median(std::vector<double> v)
{
    std::sort(v.begin(), v.end());
    const size_t n = v.size();
    return (n % 2) ? v[n / 2] : 0.5 * (v[n / 2 - 1] + v[n / 2]);
}

// FNV-1a over the codeword bits, so the survey can assert that the AFF3CT and
// gf2 sides encoded the same code rather than merely codes of equal shape.
static uint64_t
fnv1a_bits(const std::vector<int>& bits)
{
    uint64_t h = 1469598103934665603ull;
    for (int b : bits)
    {
        h ^= (uint64_t)(b & 1);
        h *= 1099511628211ull;
    }
    return h;
}

// AFF3CT's public `encode` routes through the StreamPU task/socket runtime.
// The survey measures the encoder kernels themselves, so these adapters expose
// the protected `_encode` entry point that the task ultimately calls. One call
// processes exactly one wave: 1 frame for the scalar encoder, mipp::N<B>()
// frames for the interleaved one.
struct ScalarBCH : module::Encoder_BCH<int>
{
    using module::Encoder_BCH<int>::Encoder_BCH;
    static const int FRAMES_PER_WAVE = 1;
    void wave(const int* U_K, int* X_N) { this->_encode(U_K, X_N, 0); }
};

struct InterBCH : module::Encoder_BCH_inter<int>
{
    using module::Encoder_BCH_inter<int>::Encoder_BCH_inter;
    void wave(const int* U_K, int* X_N) { this->_encode(U_K, X_N, 0); }
};

struct Row
{
    std::string workload, algorithm, code;
    int n, k, t, batch, trial;
    double ns_per_frame;
    double info_mbit_per_s;
    uint64_t digest;
};

static void
emit(const Row& r)
{
    std::printf("aff3ct,%s,%s,%s,%s,%d,%d,%d,%d,%d,%.3f,%.4f,%016llx\n",
                tools::version().c_str(),
                r.workload.c_str(),
                r.algorithm.c_str(),
                r.code.c_str(),
                r.n,
                r.k,
                r.t,
                r.batch,
                r.trial,
                r.ns_per_frame,
                r.info_mbit_per_s,
                (unsigned long long)r.digest);
}

// ----------------------------------------------------------------------- W1

template<typename ENC>
static void
run_w1(ENC& enc, int frames_per_wave, const CodeSpec& cs, int batch, const char* algorithm, std::vector<Row>& out)
{
    const int k = cs.k, n = cs.n;
    const int waves = batch / frames_per_wave;

    std::mt19937_64 rng(SEED ^ ((uint64_t)batch << 32) ^ (uint64_t)cs.m);
    std::vector<int> U((size_t)batch * k), X((size_t)batch * n);
    for (auto& u : U)
        u = (int)(rng() & 1ull);

    auto encode_batch = [&]()
    {
        for (int w = 0; w < waves; w++)
            enc.wave(U.data() + (size_t)w * frames_per_wave * k, X.data() + (size_t)w * frames_per_wave * n);
    };

    for (int w = 0; w < WARMUP_REPS; w++)
        encode_batch();

    const uint64_t digest = fnv1a_bits(X);

    // Calibrate the inner repetition count against the observed cost of one
    // batch, so every timed region clears MIN_TIMED_NS.
    const auto c0 = std::chrono::steady_clock::now();
    encode_batch();
    const auto c1 = std::chrono::steady_clock::now();
    const double one_ns = std::chrono::duration<double, std::nano>(c1 - c0).count();
    long reps = (long)(MIN_TIMED_NS / (one_ns > 0.0 ? one_ns : 1.0)) + 1;
    if (reps < 1) reps = 1;

    double spent = 0.0;
    for (int trial = 0; trial < TRIALS && spent < CELL_BUDGET_S; trial++)
    {
        const auto t0 = std::chrono::steady_clock::now();
        for (long rep = 0; rep < reps; rep++)
            encode_batch();
        const auto t1 = std::chrono::steady_clock::now();

        const double total_ns = std::chrono::duration<double, std::nano>(t1 - t0).count();
        spent += total_ns * 1e-9;
        const double ns = total_ns / (double)reps;

        Row r;
        r.workload = "W1";
        r.algorithm = algorithm;
        r.code = cs.name;
        r.n = n;
        r.k = k;
        r.t = cs.t;
        r.batch = batch;
        r.trial = trial;
        r.ns_per_frame = ns / (double)batch;
        r.info_mbit_per_s = ((double)batch * (double)k) / ns * 1e3;
        r.digest = digest;
        out.push_back(r);
    }
    std::fprintf(stderr, "#   %s %-16s batch=%-5d reps=%ld spent=%.2fs\n", cs.name, algorithm, batch, reps, spent);
}

// ----------------------------------------------------------------------- W2
//
// Generator-matrix materialization: row i of G is the encoding of the i-th
// unit message. Rows are packed into 64-bit words so the measured work
// includes the same bit-packing the gf2 BitMatrix path performs.

static void
run_w2(ScalarBCH& enc, const CodeSpec& cs, std::vector<Row>& out)
{
    const int k = cs.k, n = cs.n;
    const size_t words_per_row = (size_t)((n + 63) / 64);

    std::vector<int> U((size_t)k), X((size_t)n);

    // fresh-alloc cell per the workload-selection cache-state contract: the
    // measured materialization allocates and returns its own packed result,
    // matching the gf2 side's owned-return generator_matrix. The encoder
    // scratch U/X stays outside as encoder-internal workspace.
    auto materialize = [&]() -> std::vector<uint64_t>
    {
        std::vector<uint64_t> G((size_t)k * words_per_row, 0ull);
        for (int i = 0; i < k; i++)
        {
            std::fill(U.begin(), U.end(), 0);
            U[i] = 1;
            enc.wave(U.data(), X.data());
            uint64_t* row = &G[(size_t)i * words_per_row];
            for (int j = 0; j < n; j++)
                if (X[j])
                    row[j >> 6] |= 1ull << (j & 63);
        }
        return G;
    };

    // Probe the per-row cost on a 64-row prefix and extrapolate, so a cell
    // whose full materialization would overrun the budget is detected before
    // it is run rather than after.
    const int probe_rows = (k < 64) ? k : 64;
    const auto p0 = std::chrono::steady_clock::now();
    for (int i = 0; i < probe_rows; i++)
    {
        std::fill(U.begin(), U.end(), 0);
        U[i] = 1;
        enc.wave(U.data(), X.data());
    }
    const auto p1 = std::chrono::steady_clock::now();
    const double est_s =
      std::chrono::duration<double>(p1 - p0).count() * (double)k / (double)probe_rows;
    std::fprintf(stderr, "#   %s W2 estimated %.3f s per materialization\n", cs.name, est_s);

    const int warmups = (est_s < 2.0) ? WARMUP_REPS : 0;
    std::vector<uint64_t> last;
    for (int w = 0; w < warmups; w++)
        last = materialize();
    if (warmups == 0)
        last = materialize(); // one pass so the digest has a populated G

    uint64_t digest = 1469598103934665603ull;
    for (uint64_t word : last)
    {
        digest ^= word;
        digest *= 1099511628211ull;
    }
    last.clear();
    last.shrink_to_fit();

    // Keeps every timed materialization observable so the compiler cannot
    // elide the fresh allocation or the row stores.
    uint64_t sink = 0;

    // Calibrate the inner repetition count just as W1 does. Small generator
    // matrices otherwise fall below the protocol's minimum timed region.
    const auto c0 = std::chrono::steady_clock::now();
    sink ^= materialize().front();
    const auto c1 = std::chrono::steady_clock::now();
    const double one_ns = std::chrono::duration<double, std::nano>(c1 - c0).count();
    long reps = (long)(MIN_TIMED_NS / (one_ns > 0.0 ? one_ns : 1.0)) + 1;
    if (reps < 1) reps = 1;

    double spent = 0.0;
    for (int trial = 0; trial < TRIALS && spent < CELL_BUDGET_S; trial++)
    {
        const auto t0 = std::chrono::steady_clock::now();
        for (long rep = 0; rep < reps; rep++)
            sink ^= materialize().front();
        const auto t1 = std::chrono::steady_clock::now();

        const double total_ns = std::chrono::duration<double, std::nano>(t1 - t0).count();
        spent += total_ns * 1e-9;
        const double ns = total_ns / (double)reps;
        Row r;
        r.workload = "W2";
        r.algorithm = "basis-encode-pack";
        r.code = cs.name;
        r.n = n;
        r.k = k;
        r.t = cs.t;
        r.batch = k; // one full G materialization = k rows
        r.trial = trial;
        r.ns_per_frame = ns / (double)k;
        r.info_mbit_per_s = ((double)k * (double)n) / ns * 1e3; // matrix bits/s
        r.digest = digest;
        out.push_back(r);
    }
    std::fprintf(stderr, "#   %s basis-encode-pack batch=%d reps=%ld spent=%.2fs sink=%016llx\n",
                 cs.name, k, reps, spent, (unsigned long long)sink);
}

// ---------------------------------------------------------------------- main

int
main(int argc, char** argv)
{
    // Selector: "w1", "w2", "gdump", or "all" (default).
    const std::string what = (argc > 1) ? argv[1] : "all";
    const bool do_w1 = (what == "all" || what == "w1");
    const bool do_w2 = (what == "all" || what == "w2");

    // `gdump` writes the generator polynomial of every contract row as
    // "<name> <deg> <coeff_0><coeff_1>...<coeff_deg>", so the sibling M4RI
    // harness reduces the same generator this harness encodes with.
    if (what == "gdump")
    {
        for (int c = 0; c < N_CODES; c++)
        {
            const CodeSpec& cs = CODES[c];
            tools::BCH_polynomial_generator<int> gf((1 << cs.m) - 1, cs.t, prim_coeffs(cs.m, cs.prim));
            const std::vector<int>& g = gf.get_g();
            std::printf("%s %d %d %d ", cs.name, cs.n, cs.k, gf.get_n_rdncy());
            for (size_t i = 0; i < g.size(); i++)
                std::putchar(g[i] ? '1' : '0');
            std::putchar('\n');
        }
        return 0;
    }

    std::fprintf(stderr, "# harness: aff3ct_bch_bench\n");
    std::fprintf(stderr, "# aff3ct_version: %s\n", tools::version().c_str());
    std::fprintf(stderr, "# mipp_lanes_int32: %d\n", mipp::N<int>());
    std::fprintf(stderr, "# mipp_instr_type: %s\n", mipp::InstructionFullType.c_str());
    std::fprintf(stderr, "# trials_per_cell: %d\n", TRIALS);
    std::fprintf(stderr, "# warmup_reps: %d\n", WARMUP_REPS);
    std::fprintf(stderr, "# seed: 0x%016llx\n", (unsigned long long)SEED);

    std::printf("lib,version,workload,algorithm,code,n,k,t,batch,trial,ns_per_frame,info_mbit_per_s,digest\n");

    // Optional comma-separated allowlist of contract rows, so a run can be
    // bounded to the shapes that fit the available measurement window.
    const char* only_env = std::getenv("GF2_SURVEY_CODES");
    const std::string only = only_env ? only_env : "";
    if (!only.empty())
        std::fprintf(stderr, "# codes_selected: %s\n", only.c_str());

    for (int c = 0; c < N_CODES; c++)
    {
        const CodeSpec& cs = CODES[c];
        if (!only.empty() && only.find(cs.name) == std::string::npos)
            continue;
        const int mother_n = (1 << cs.m) - 1;

        tools::BCH_polynomial_generator<int> gf(mother_n, cs.t, prim_coeffs(cs.m, cs.prim));

        // The shortened code is only well posed when the generator degree
        // matches the declared parity length; report and skip otherwise
        // instead of measuring a different code than the contract names.
        if (gf.get_n_rdncy() != cs.n - cs.k)
        {
            std::fprintf(stderr,
                         "# SKIP %s: n_rdncy=%d but n-k=%d\n",
                         cs.name,
                         gf.get_n_rdncy(),
                         cs.n - cs.k);
            continue;
        }

        std::fprintf(stderr, "# code %s: mother_n=%d deg_g=%d\n", cs.name, mother_n, gf.get_n_rdncy());

        std::vector<Row> rows;

        if (do_w1)
        {
            for (int b = 0; b < N_BATCHES; b++)
            {
                const int batch = BATCHES[b];

                ScalarBCH enc_seq(cs.k, cs.n, gf);
                run_w1(enc_seq, ScalarBCH::FRAMES_PER_WAVE, cs, batch, "lfsr-scalar", rows);

                // The interleaved encoder consumes whole SIMD waves only.
                if (batch % mipp::N<int>() == 0)
                {
                    InterBCH enc_simd(cs.k, cs.n, gf);
                    run_w1(enc_simd, mipp::N<int>(), cs, batch, "lfsr-simd-inter", rows);
                }
                else
                {
                    std::fprintf(stderr,
                                 "# SKIP %s batch=%d lfsr-simd-inter: batch not a multiple of %d lanes\n",
                                 cs.name,
                                 batch,
                                 mipp::N<int>());
                }
            }
        }

        if (do_w2)
        {
            ScalarBCH enc_seq(cs.k, cs.n, gf);
            run_w2(enc_seq, cs, rows);
        }

        for (const Row& r : rows)
            emit(r);
        std::fflush(stdout);

    }

    return 0;
}
