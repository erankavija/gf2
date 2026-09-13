// C-ABI shim over AFF3CT v4.7.0's 5G NR LDPC encoder and puncturer
// (jit:12fdeb5b).
//
// The shim wires the two modules exactly as AFF3CT's own factories do. The
// encoder is `Encoder_LDPC_QC_fast<int32_t>` constructed with the K, the
// mother length `N_LDPC`, the lifting `Zc`, the generator-matrix file
// `conf/enc/LDPC/5G/NR_<Bg>_<index_list>_<Zc>.txt` and the base-graph
// `K_LDPC` that `tools::build_5G_base_graph(K, N)` derives, which is what
// `factory::Encoder_LDPC::build` passes. The puncturer is
// `Puncturer_5G<int32_t, float>` constructed with K, N and that same
// `N_LDPC`. Each call goes through the public `Encoder::encode` and
// `Puncturer::puncture` entry points, which run the modules' tasks and their
// protected hooks. The shim adds no derivation, encoding or selection logic
// of its own.
//
// AFF3CT derives the base graph and the lifting size from (K, N) itself, so a
// caller cannot ask it for another projects's parameters; the validation
// record reports the pairs where that derivation differs.
//
// (int32_t, float) is AFF3CT's (B_32, Q_32) pair, one of the explicit
// instantiations the multi-precision static library carries. It must be
// compiled with the definitions and language standard the pinned static
// library was built with (see `../build.rs`). AFF3CT reports invalid
// arguments by throwing, so every entry point catches and returns a status
// instead of unwinding across the C ABI.

#include <cstdint>
#include <string>
#include <vector>

#include "Module/Encoder/LDPC/QC/Encoder_LDPC_QC_fast.hpp"
#include "Module/Puncturer/LDPC/Puncturer_5G.hpp"
#include "Tools/Code/LDPC/Standard/5G/5G_base_graph.hpp"

namespace {

typedef aff3ct::module::Encoder_LDPC_QC_fast<int32_t> Encoder;
typedef aff3ct::module::Puncturer_5G<int32_t, float>  Puncturer;

struct Comparator
{
    Encoder*   encoder;
    Puncturer* puncturer;
    int32_t    k;
    int32_t    n;
    int32_t    n_ldpc;
};

} // namespace

extern "C"
{

    /// The parameters AFF3CT derives for one (K, N).
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
    int32_t gf2_aff3ct_nr_base_graph_for(int32_t k, int32_t n, gf2_aff3ct_nr_base_graph* out)
    {
        try
        {
            const aff3ct::tools::Std_5G_base_graph derived = aff3ct::tools::build_5G_base_graph(k, n);
            out->base_graph = derived.Bg;
            out->lifting    = derived.Zc;
            out->index_list = derived.index_list;
            out->k_ldpc     = derived.K_LDPC;
            out->n_ldpc     = derived.N_LDPC;
            return 0;
        }
        catch (...)
        {
            return 1;
        }
    }

    /// Constructs the encoder and puncturer AFF3CT's factories would build
    /// for (K, N). `conf_root` names the directory holding AFF3CT's `conf`
    /// tree, whose generator-matrix file the encoder reads. Returns null when
    /// AFF3CT rejects the pair or cannot read that file.
    void* gf2_aff3ct_nr_comparator_new(int32_t k, int32_t n, const char* conf_root)
    {
        Encoder*   encoder   = nullptr;
        Puncturer* puncturer = nullptr;
        try
        {
            const aff3ct::tools::Std_5G_base_graph derived = aff3ct::tools::build_5G_base_graph(k, n);
            const std::string                      path    = std::string(conf_root) + "/enc/LDPC/5G/NR_" +
                                     std::to_string(derived.Bg) + "_" + std::to_string(derived.index_list) + "_" +
                                     std::to_string(derived.Zc) + ".txt";
            encoder   = new Encoder(k, derived.N_LDPC, derived.Zc, path.c_str(), derived.K_LDPC);
            puncturer = new Puncturer(k, n, derived.N_LDPC, std::vector<bool>());
            return new Comparator{ encoder, puncturer, k, n, derived.N_LDPC };
        }
        catch (...)
        {
            delete puncturer;
            delete encoder;
            return nullptr;
        }
    }

    void gf2_aff3ct_nr_comparator_free(void* handle)
    {
        Comparator* comparator = static_cast<Comparator*>(handle);
        if (comparator == nullptr) return;
        delete comparator->puncturer;
        delete comparator->encoder;
        delete comparator;
    }

    /// Encodes `K` information bits into the `N_LDPC`-long mother codeword
    /// and selects the `N` transmitted bits from it. `message` holds K values
    /// in {0, 1}, `mother` receives N_LDPC values and `transmitted` receives
    /// N. Returns 0 on success and 1 when AFF3CT throws.
    int32_t gf2_aff3ct_nr_encode_rate_match(void*          handle,
                                            const int32_t* message,
                                            int32_t*       mother,
                                            int32_t*       transmitted)
    {
        Comparator* comparator = static_cast<Comparator*>(handle);
        try
        {
            comparator->encoder->encode(message, mother);
            comparator->puncturer->puncture(mother, transmitted);
            return 0;
        }
        catch (...)
        {
            return 1;
        }
    }
}
