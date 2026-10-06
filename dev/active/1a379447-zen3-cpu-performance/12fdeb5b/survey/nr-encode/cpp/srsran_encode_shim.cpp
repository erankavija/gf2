// C-ABI shim over srsRAN's 5G NR LDPC encoder and rate matcher (jit:12fdeb5b).
//
// srsRAN's own CMake configuration stops on this host because MbedTLS is
// absent, so the shim compiles the LDPC translation units directly. The
// files it needs are the encoder implementation and its two software
// backends, the Tanner-graph and lifting-table lookups, the rate matcher and
// the two `srsvec` units they call; `../srsran-build.sh` lists them and
// records their digests. None of the security, radio or logging layers that
// need MbedTLS is on that closure.
//
// The shim adds no selection, encoding or interleaving logic. It constructs
// `srsran::ldpc_encoder_generic` or `srsran::ldpc_encoder_avx2` (the choice
// `ldpc_encoder_factory_sw`'s "auto" makes), wraps the caller's byte arrays
// as `srsran::bit_buffer` views without copying, and calls the public
// `ldpc_encoder::encode` and `ldpc_rate_matcher::rate_match` entry points.
//
// srsRAN reports invalid arguments by aborting inside `srsran_assert`, which
// no `catch` can intercept, so every entry point validates the arguments
// srsRAN asserts on before calling it and returns a status instead.

#include <cstdint>
#include <cstring>
#include <string>
#include <vector>

#include "srsran/adt/bit_buffer.h"
#include "srsran/adt/span.h"
#include "srsran/phy/upper/codeblock_metadata.h"
#include "srsran/ran/sch/ldpc_base_graph.h"

#include "phy/upper/channel_coding/ldpc/ldpc_encoder_avx2.h"
#include "phy/upper/channel_coding/ldpc/ldpc_encoder_generic.h"
#include "phy/upper/channel_coding/ldpc/ldpc_graph_impl.h"
#include "phy/upper/channel_coding/ldpc/ldpc_luts_impl.h"
#include "phy/upper/channel_coding/ldpc/ldpc_rate_matcher_impl.h"

namespace {

struct Comparator
{
    std::unique_ptr<srsran::ldpc_encoder> encoder;
    srsran::ldpc_rate_matcher_impl        matcher;
    int32_t                               backend;
};

/// Bytes a packed bit buffer of `bits` bits occupies.
unsigned words_for(unsigned bits)
{
    return (bits + 7u) / 8u;
}

/// True when `z` is one of the 51 lifting sizes TS 38.212 Table 5.3.2-1 lists.
bool lifting_size_known(int32_t z)
{
    for (auto candidate : srsran::ldpc::all_lifting_sizes)
        if (static_cast<int32_t>(candidate) == z) return true;
    return false;
}

} // namespace

extern "C"
{

    /// The dimensions srsRAN derives from a base graph and a lifting size.
    struct gf2_srsran_nr_dims
    {
        int32_t k_ldpc;  ///< Systematic bits the encoder reads, including fillers.
        int32_t n_short; ///< Codeblock bits the encoder writes, excluding the 2*Z prefix.
        int32_t n_full;  ///< Mother-codeword bits including the 2*Z prefix.
    };

    /// True when this build's CPU supports the AVX2 backend, which is the
    /// backend `ldpc_encoder_factory_sw("auto")` then selects.
    int32_t gf2_srsran_nr_supports_avx2(void)
    {
#ifdef __x86_64__
        return __builtin_cpu_supports("avx2") ? 1 : 0;
#else
        return 0;
#endif
    }

    /// Fills `out` with srsRAN's dimensions for `(bg, z)`. Returns 0 when
    /// srsRAN carries a graph for the pair and 1 otherwise.
    int32_t gf2_srsran_nr_dims_for(int32_t bg, int32_t z, gf2_srsran_nr_dims* out)
    {
        if ((bg != 1 && bg != 2) || !lifting_size_known(z)) return 1;
        const auto            lifting = static_cast<srsran::ldpc::lifting_size_t>(z);
        const uint8_t         position = srsran::ldpc::get_lifting_size_position(lifting);
        const unsigned        skip = (bg == 2) ? srsran::ldpc::NOF_LIFTING_SIZES : 0;
        const srsran::ldpc_graph_impl& graph = srsran::ldpc::graph_array[skip + position];
        out->k_ldpc  = static_cast<int32_t>(graph.get_nof_BG_info_nodes()) * z;
        out->n_short = static_cast<int32_t>(graph.get_nof_BG_var_nodes_short()) * z;
        out->n_full  = static_cast<int32_t>(graph.get_nof_BG_var_nodes_full()) * z;
        return 0;
    }

    /// Constructs the encoder and rate matcher. `backend` is 0 for the
    /// generic backend and 1 for AVX2; any other value selects the backend
    /// srsRAN's "auto" would. Returns null when the request cannot be served.
    void* gf2_srsran_nr_comparator_new(int32_t backend)
    {
        int32_t selected = backend;
        if (selected != 0 && selected != 1) selected = gf2_srsran_nr_supports_avx2();
        if (selected == 1 && !gf2_srsran_nr_supports_avx2()) return nullptr;
        auto* comparator = new Comparator{nullptr, {}, selected};
#ifdef __x86_64__
        if (selected == 1)
            comparator->encoder = std::make_unique<srsran::ldpc_encoder_avx2>();
        else
#endif
            comparator->encoder = std::make_unique<srsran::ldpc_encoder_generic>();
        return comparator;
    }

    void gf2_srsran_nr_comparator_free(void* handle) { delete static_cast<Comparator*>(handle); }

    /// The backend the handle selected: 0 generic, 1 AVX2.
    int32_t gf2_srsran_nr_backend(void* handle)
    {
        return static_cast<Comparator*>(handle)->backend;
    }

    /// Encodes `k` information bits and rate-matches them to `n` bits.
    ///
    /// `in_bytes` holds `k_ldpc` bits packed most-significant-bit first, the
    /// filler positions `[k, k_ldpc)` already zero, in srsRAN's own
    /// `bit_buffer` layout. `out_bytes` receives `n` bits in that layout.
    /// `rv` is the redundancy version and `qm` the modulation order, both as
    /// TS 38.212 Section 5.4.2 defines them.
    ///
    /// Returns 0 on success and a positive code for a request srsRAN's
    /// assertions reject: 1 an unknown graph, 2 an out-of-range redundancy
    /// version or modulation order, 3 an output length srsRAN cannot serve.
    int32_t gf2_srsran_nr_encode_rate_match(void*          handle,
                                            int32_t        bg,
                                            int32_t        z,
                                            int32_t        k,
                                            int32_t        n,
                                            int32_t        rv,
                                            int32_t        qm,
                                            const uint8_t* in_bytes,
                                            uint8_t*       out_bytes)
    {
        gf2_srsran_nr_dims dims{};
        if (gf2_srsran_nr_dims_for(bg, z, &dims) != 0) return 1;
        if (rv < 0 || rv > 3) return 2;
        if (qm != 1 && qm != 2 && qm != 4 && qm != 6 && qm != 8) return 2;
        if (k <= 0 || n <= 0 || k > dims.k_ldpc) return 3;
        if (n % qm != 0) return 3;
        if (n > static_cast<int32_t>(srsran::ldpc::MAX_CODEBLOCK_RM_SIZE)) return 3;
        // `ldpc_rate_matcher_impl::init` asserts on this bound.
        const int32_t systematic = dims.k_ldpc - 2 * z;
        if (dims.k_ldpc - k >= systematic) return 3;

        auto* comparator = static_cast<Comparator*>(handle);

        srsran::span<uint8_t> in_span(const_cast<uint8_t*>(in_bytes), words_for(static_cast<unsigned>(dims.k_ldpc)));
        srsran::bit_buffer    input = srsran::bit_buffer::from_bytes(in_span).first(static_cast<unsigned>(dims.k_ldpc));

        srsran::span<uint8_t> out_span(out_bytes, words_for(static_cast<unsigned>(n)));
        srsran::bit_buffer    output = srsran::bit_buffer::from_bytes(out_span).first(static_cast<unsigned>(n));

        srsran::codeblock_metadata metadata{};
        metadata.tb_common.base_graph =
          (bg == 1) ? srsran::ldpc_base_graph_type::BG1 : srsran::ldpc_base_graph_type::BG2;
        metadata.tb_common.lifting_size = static_cast<srsran::ldpc::lifting_size_t>(z);
        metadata.tb_common.rv           = static_cast<unsigned>(rv);
        metadata.tb_common.mod          = static_cast<srsran::modulation_scheme>(qm);
        metadata.tb_common.Nref         = 0;
        metadata.cb_specific.nof_filler_bits = static_cast<unsigned>(dims.k_ldpc - k);

        const srsran::ldpc_encoder_buffer& encoded =
          comparator->encoder->encode(input, {metadata.tb_common.base_graph, metadata.tb_common.lifting_size, 0});
        comparator->matcher.rate_match(output, encoded, metadata);
        return 0;
    }

    /// Writes the encoder's unshortened codeblock, `n_short` bits packed the
    /// same way, without rate matching. The validation record uses it to
    /// compare mother codewords position by position.
    int32_t gf2_srsran_nr_encode_only(void*          handle,
                                      int32_t        bg,
                                      int32_t        z,
                                      int32_t        k,
                                      const uint8_t* in_bytes,
                                      uint8_t*       out_bits)
    {
        gf2_srsran_nr_dims dims{};
        if (gf2_srsran_nr_dims_for(bg, z, &dims) != 0) return 1;
        if (k <= 0 || k > dims.k_ldpc) return 3;

        auto* comparator = static_cast<Comparator*>(handle);
        srsran::span<uint8_t> in_span(const_cast<uint8_t*>(in_bytes), words_for(static_cast<unsigned>(dims.k_ldpc)));
        srsran::bit_buffer    input = srsran::bit_buffer::from_bytes(in_span).first(static_cast<unsigned>(dims.k_ldpc));

        const auto base_graph = (bg == 1) ? srsran::ldpc_base_graph_type::BG1 : srsran::ldpc_base_graph_type::BG2;
        const srsran::ldpc_encoder_buffer& encoded =
          comparator->encoder->encode(input, {base_graph, static_cast<srsran::ldpc::lifting_size_t>(z), 0});
        encoded.write_codeblock(srsran::span<uint8_t>(out_bits, static_cast<size_t>(dims.n_short)), 0);
        return 0;
    }
}
