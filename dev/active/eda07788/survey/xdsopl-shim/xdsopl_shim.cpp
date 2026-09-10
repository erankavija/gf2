// Operation-equivalent xdsopl/LDPC DVB-T2 bit-interleaver arm (jit:eda07788).
//
// Each entry point instantiates xdsopl's `PCTITL<TYPE, N, Q, CT>` for one
// ETSI EN 302 755 v1.4.1 §6.1.3 MODCOD: stage 1 parity interleaving composed
// with stage 2 column twisting. The outer `BITL<..., MUX8/MUX12>` wrapper
// xdsopl's own `itls_handler.cc` adds folds in the §6.1.4/§6.1.5 bit-to-cell
// demux, which gf2's module scopes out, so it is deliberately not used here.
//
// N and Q come from ETSI Table 9 and Q = (N - K)/360; xdsopl's `PITL` derives
// K = N - 360*Q, so the pair (N, Q) fixes the same K the gf2 side reads from
// its DVB parameter table. The twist offsets are ETSI Table 10.

#include <cstdint>
#include <vector>

#include "interleaver.hh"

namespace {

// Rate 1/2 Normal FECFRAME: N = 64800, K = 32400, Q = 90.
using Qam16Normal =
    PCTITL<int32_t, 64800, 90, CT8<int32_t, 0, 0, 2, 4, 4, 5, 7, 7>>;
using Qam64Normal =
    PCTITL<int32_t, 64800, 90, CT12<int32_t, 0, 0, 2, 2, 3, 4, 4, 5, 5, 7, 8, 9>>;
// Rate 1/2 Short FECFRAME: N = 16200, K = 7200, Q = 25.
using Qam16Short =
    PCTITL<int32_t, 16200, 25, CT8<int32_t, 0, 0, 0, 1, 7, 20, 20, 21>>;
using Qam64Short =
    PCTITL<int32_t, 16200, 25, CT12<int32_t, 0, 0, 0, 2, 2, 2, 3, 3, 3, 6, 7, 7>>;

template <typename Interleaver>
void forward(const int32_t* input, int32_t* output)
{
    // PCTITL::fwd overwrites its input buffer after the parity stage.
    std::vector<int32_t> mutable_input(input, input + Interleaver::N);
    Interleaver::fwd(output, mutable_input.data());
}

} // namespace

extern "C" void xdsopl_pctitl_qam16_r12_normal_fwd(const int32_t* in,
                                                     int32_t* out)
{
    forward<Qam16Normal>(in, out);
}

extern "C" void xdsopl_pctitl_qam64_r12_normal_fwd(const int32_t* in,
                                                     int32_t* out)
{
    forward<Qam64Normal>(in, out);
}

extern "C" void xdsopl_pctitl_qam16_r12_short_fwd(const int32_t* in,
                                                    int32_t* out)
{
    forward<Qam16Short>(in, out);
}

extern "C" void xdsopl_pctitl_qam64_r12_short_fwd(const int32_t* in,
                                                    int32_t* out)
{
    forward<Qam64Short>(in, out);
}
