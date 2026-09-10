// C-ABI shim exposing AFF3CT v4.7.0's 5G NR LLR de-rate-matching (jit:eda07788).
//
// The shim constructs `aff3ct::module::Puncturer_5G<int32_t, float>` with the
// arguments AFF3CT's own codec wiring passes: K, N, the mother-codeword length
// N_cw = N_LDPC of the base graph `tools::build_5G_base_graph(K, N)` derives,
// and an empty pattern. Each call goes through the public
// `Puncturer::depuncture` entry point, which runs the module's task and its
// protected `_depuncture` hook. The shim adds no selection logic of its own.
//
// (int32_t, float) is AFF3CT's (B_32, Q_32) pair, one of the explicit
// instantiations the multi-precision static library carries.
//
// It must be compiled with the definitions and language standard the pinned
// static library was built with (see `../build.rs`); the library's class
// layouts depend on them. AFF3CT reports invalid arguments by throwing, so
// every entry point catches and returns a status instead of unwinding across
// the C ABI.

#include <cstdint>
#include <vector>

#include "Module/Puncturer/LDPC/Puncturer_5G.hpp"
#include "Tools/Code/LDPC/Standard/5G/5G_base_graph.hpp"

namespace {

typedef aff3ct::module::Puncturer_5G<int32_t, float> Depuncturer;

} // namespace

extern "C"
{

    /// Parameters AFF3CT derives for one (K, N).
    struct gf2_aff3ct_nr_base_graph
    {
        int32_t base_graph;
        int32_t lifting;
        int32_t index_list;
        int32_t k_ldpc;
        int32_t n_ldpc;
    };

    /// Fills `out` with AFF3CT's derivation for (K, N). Returns 0 when AFF3CT
    /// accepts the pair and 1 when it throws.
    int gf2_aff3ct_nr_base_graph_for(int32_t k, int32_t n, gf2_aff3ct_nr_base_graph* out)
    {
        try
        {
            const aff3ct::tools::Std_5G_base_graph derived = aff3ct::tools::build_5G_base_graph(k, n);
            out->base_graph = derived.Bg;
            out->lifting = derived.Zc;
            out->index_list = derived.index_list;
            out->k_ldpc = derived.K_LDPC;
            out->n_ldpc = derived.N_LDPC;
            return 0;
        }
        catch (...)
        {
            return 1;
        }
    }

    /// Constructs the 5G depuncturer for (K, N), or returns null when AFF3CT
    /// rejects the pair.
    void* gf2_aff3ct_nr_depuncturer_new(int32_t k, int32_t n)
    {
        try
        {
            const aff3ct::tools::Std_5G_base_graph derived = aff3ct::tools::build_5G_base_graph(k, n);
            return new Depuncturer(k, n, derived.N_LDPC, std::vector<bool>());
        }
        catch (...)
        {
            return 0;
        }
    }

    void gf2_aff3ct_nr_depuncturer_free(void* handle)
    {
        delete static_cast<Depuncturer*>(handle);
    }

    /// Reads N channel LLRs and writes into the N_LDPC-long `full` buffer the
    /// positions AFF3CT's depuncture writes. Returns 0 on success.
    int gf2_aff3ct_nr_depuncture(void* handle, const float* channel, float* full)
    {
        try
        {
            static_cast<Depuncturer*>(handle)->depuncture(channel, full);
            return 0;
        }
        catch (...)
        {
            return 1;
        }
    }
}
