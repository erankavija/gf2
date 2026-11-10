// C ABI over srsRAN's NR LDPC encoder and rate matcher, built from the LDPC
// translation units alone (see ../build.rs). The encoder is the one
// `ldpc_encoder_factory_sw("auto")` selects: AVX2 when the CPU has it.

#include <cstdint>
#include <memory>

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
};
} // namespace

extern "C" {

/// 1 when the AVX2 backend is selected, 0 for the generic one.
int32_t gf2_srsran_nr_avx2(void)
{
    return __builtin_cpu_supports("avx2") ? 1 : 0;
}

/// Systematic bits, fillers included, that srsRAN's graph for (bg, z) reads.
int32_t gf2_srsran_nr_k_ldpc(int32_t bg, int32_t z)
{
    const auto     lifting  = static_cast<srsran::ldpc::lifting_size_t>(z);
    const uint8_t  position = srsran::ldpc::get_lifting_size_position(lifting);
    const unsigned skip     = (bg == 2) ? srsran::ldpc::NOF_LIFTING_SIZES : 0;
    return static_cast<int32_t>(srsran::ldpc::graph_array[skip + position].get_nof_BG_info_nodes()) * z;
}

void* gf2_srsran_nr_new(void)
{
    auto* comparator = new Comparator{};
    if (gf2_srsran_nr_avx2())
        comparator->encoder = std::make_unique<srsran::ldpc_encoder_avx2>();
    else
        comparator->encoder = std::make_unique<srsran::ldpc_encoder_generic>();
    return comparator;
}

void gf2_srsran_nr_free(void* handle)
{
    delete static_cast<Comparator*>(handle);
}

/// Encodes and rate-matches one codeblock at redundancy version 0, one bit
/// per symbol. `in_bytes` holds k_ldpc bits MSB first with fillers
/// [k, k_ldpc) zero; `out_bytes` receives n bits in the same layout.
void gf2_srsran_nr_encode(void* handle, int32_t bg, int32_t z, int32_t k, int32_t n, const uint8_t* in_bytes,
                          uint8_t* out_bytes)
{
    auto*          comparator = static_cast<Comparator*>(handle);
    const unsigned k_ldpc     = static_cast<unsigned>(gf2_srsran_nr_k_ldpc(bg, z));
    srsran::span<uint8_t> in_span(const_cast<uint8_t*>(in_bytes), (k_ldpc + 7) / 8);
    srsran::bit_buffer    input = srsran::bit_buffer::from_bytes(in_span).first(k_ldpc);
    srsran::span<uint8_t> out_span(out_bytes, (static_cast<unsigned>(n) + 7) / 8);
    srsran::bit_buffer    output = srsran::bit_buffer::from_bytes(out_span).first(static_cast<unsigned>(n));

    srsran::codeblock_metadata metadata{};
    metadata.tb_common.base_graph = (bg == 1) ? srsran::ldpc_base_graph_type::BG1 : srsran::ldpc_base_graph_type::BG2;
    metadata.tb_common.lifting_size      = static_cast<srsran::ldpc::lifting_size_t>(z);
    metadata.tb_common.rv                = 0;
    metadata.tb_common.mod               = static_cast<srsran::modulation_scheme>(1);
    metadata.tb_common.Nref              = 0;
    metadata.cb_specific.nof_filler_bits = k_ldpc - static_cast<unsigned>(k);

    const srsran::ldpc_encoder_buffer& encoded =
        comparator->encoder->encode(input, {metadata.tb_common.base_graph, metadata.tb_common.lifting_size, 0});
    comparator->matcher.rate_match(output, encoded, metadata);
}
}
