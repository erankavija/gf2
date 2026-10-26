// C entry points to AFF3CT's BCH encoders for the external arm (jit:1358d6ab).
//
// AFF3CT's public `encode` routes through the StreamPU task runtime; the
// adapters call the protected `_encode` the task ends in, as the `4e732b56`
// survey harness does, so one call encodes one wave of frames.

#include <cstdint>
#include <exception>
#include <string>
#include <vector>

#include <mipp.h>

#include "Module/Encoder/BCH/Encoder_BCH.hpp"
#include "Module/Encoder/BCH/Encoder_BCH_inter.hpp"
#include "Tools/Code/BCH/BCH_polynomial_generator.hpp"
#include "Tools/version.h"

using namespace aff3ct;

namespace
{
struct ScalarBCH : module::Encoder_BCH<int>
{
    using module::Encoder_BCH<int>::Encoder_BCH;
    void wave(const int* u, int* x) { this->_encode(u, x, 0); }
};

struct InterBCH : module::Encoder_BCH_inter<int>
{
    using module::Encoder_BCH_inter<int>::Encoder_BCH_inter;
    void wave(const int* u, int* x) { this->_encode(u, x, 0); }
};

struct Handle
{
    explicit Handle(const tools::BCH_polynomial_generator<int>& generator)
      : generator(generator)
    {
    }
    tools::BCH_polynomial_generator<int> generator;
    ScalarBCH* scalar = nullptr;
    InterBCH* inter = nullptr;
};
} // namespace

extern "C"
{
    // `prim` holds the coefficient of x^i at bit i. Returns null when AFF3CT
    // rejects the parameters or their generator degree is not n - k.
    void* gf2_aff3ct_bch_new(int m, uint32_t prim, int t, int k, int n, int inter)
    {
        try
        {
            std::vector<int> coefficients(m + 1);
            for (int i = 0; i <= m; i++)
                coefficients[i] = (int)((prim >> i) & 1u);
            auto* handle = new Handle(tools::BCH_polynomial_generator<int>((1 << m) - 1, t, coefficients));
            if (handle->generator.get_n_rdncy() != n - k)
            {
                delete handle;
                return nullptr;
            }
            if (inter)
                handle->inter = new InterBCH(k, n, handle->generator);
            else
                handle->scalar = new ScalarBCH(k, n, handle->generator);
            return handle;
        }
        catch (const std::exception&)
        {
            return nullptr;
        }
    }

    int gf2_aff3ct_bch_frames_per_wave(void* handle)
    {
        return static_cast<Handle*>(handle)->inter ? mipp::N<int>() : 1;
    }

    // Encodes `waves` consecutive waves of contiguous frames.
    void gf2_aff3ct_bch_encode(void* handle, const int* u, int* x, int k, int n, int waves)
    {
        auto* h = static_cast<Handle*>(handle);
        const int frames = gf2_aff3ct_bch_frames_per_wave(handle);
        for (int w = 0; w < waves; w++)
        {
            const int* uw = u + (size_t)w * frames * k;
            int* xw = x + (size_t)w * frames * n;
            if (h->inter)
                h->inter->wave(uw, xw);
            else
                h->scalar->wave(uw, xw);
        }
    }

    void gf2_aff3ct_bch_free(void* handle)
    {
        auto* h = static_cast<Handle*>(handle);
        delete h->scalar;
        delete h->inter;
        delete h;
    }

    // Static strings, valid for the life of the process.
    const char* gf2_aff3ct_version()
    {
        static const std::string version = tools::version();
        return version.c_str();
    }

    const char* gf2_aff3ct_instructions() { return mipp::InstructionFullType.c_str(); }
}
