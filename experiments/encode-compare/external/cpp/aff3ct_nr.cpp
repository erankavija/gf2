// C ABI over AFF3CT 4.7.0's NR encoder `Encoder_LDPC_QC_fast` and
// `Puncturer_5G`, wired as `factory::Encoder_LDPC::build` does, at the int8
// and int32 bit types the multi-precision library instantiates. AFF3CT
// derives its base graph and lifting size from (K, N).

#include <cstdint>
#include <string>
#include <vector>

#include "Module/Encoder/LDPC/QC/Encoder_LDPC_QC_fast.hpp"
#include "Module/Puncturer/LDPC/Puncturer_5G.hpp"
#include "Tools/Code/LDPC/Standard/5G/5G_base_graph.hpp"

namespace {
template <typename B, typename Q>
struct Comparator
{
    aff3ct::module::Encoder_LDPC_QC_fast<B>* encoder;
    aff3ct::module::Puncturer_5G<B, Q>*      puncturer;
    std::vector<B>                           mother;
};

template <typename B, typename Q>
void* make(int32_t k, int32_t n, const char* conf_root)
{
    try {
        const auto derived = aff3ct::tools::build_5G_base_graph(k, n);
        const std::string path = std::string(conf_root) + "/enc/LDPC/5G/NR_" + std::to_string(derived.Bg) + "_" +
                                 std::to_string(derived.index_list) + "_" + std::to_string(derived.Zc) + ".txt";
        auto* encoder =
            new aff3ct::module::Encoder_LDPC_QC_fast<B>(k, derived.N_LDPC, derived.Zc, path.c_str(), derived.K_LDPC);
        auto* puncturer = new aff3ct::module::Puncturer_5G<B, Q>(k, n, derived.N_LDPC, std::vector<bool>());
        return new Comparator<B, Q>{encoder, puncturer, std::vector<B>(derived.N_LDPC)};
    } catch (...) {
        return nullptr;
    }
}

template <typename B, typename Q>
void destroy(void* handle)
{
    auto* c = static_cast<Comparator<B, Q>*>(handle);
    if (c == nullptr) return;
    delete c->puncturer;
    delete c->encoder;
    delete c;
}

template <typename B, typename Q>
int32_t encode(void* handle, const B* message, B* transmitted)
{
    auto* c = static_cast<Comparator<B, Q>*>(handle);
    try {
        c->encoder->encode(message, c->mother.data());
        c->puncturer->puncture(c->mother.data(), transmitted);
        return 0;
    } catch (...) {
        return 1;
    }
}
} // namespace

extern "C" {

/// AFF3CT's base graph and lifting size for (K, N); 0 when it accepts the
/// pair.
int32_t gf2_aff3ct_nr_derive(int32_t k, int32_t n, int32_t* bg, int32_t* z)
{
    try {
        const auto derived = aff3ct::tools::build_5G_base_graph(k, n);
        *bg = derived.Bg;
        *z  = derived.Zc;
        return 0;
    } catch (...) {
        return 1;
    }
}

void* gf2_aff3ct_nr_new_i8(int32_t k, int32_t n, const char* conf) { return make<int8_t, int8_t>(k, n, conf); }
void* gf2_aff3ct_nr_new_i32(int32_t k, int32_t n, const char* conf) { return make<int32_t, float>(k, n, conf); }
void  gf2_aff3ct_nr_free_i8(void* h) { destroy<int8_t, int8_t>(h); }
void  gf2_aff3ct_nr_free_i32(void* h) { destroy<int32_t, float>(h); }
int32_t gf2_aff3ct_nr_encode_i8(void* h, const int8_t* m, int8_t* t) { return encode<int8_t, int8_t>(h, m, t); }
int32_t gf2_aff3ct_nr_encode_i32(void* h, const int32_t* m, int32_t* t) { return encode<int32_t, float>(h, m, t); }
}
