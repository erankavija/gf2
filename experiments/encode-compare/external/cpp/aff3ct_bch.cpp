// C ABI over AFF3CT 4.7.0's binary BCH encoders at the int8 and int32 bit
// types the multi-precision library instantiates: `Encoder_BCH`, the
// bit-serial LFSR one frame per wave, and `Encoder_BCH_inter`, the same
// recurrence across mipp::N<B>() frames per wave in SIMD registers.
//
// AFF3CT's public `encode` routes through the StreamPU task runtime; the
// adapters call the protected `_encode` kernel that task runs, so the cost is
// the encoder's own. A codeword is [parity | message] with entry j holding
// the coefficient of x^j, the reverse of gf2's descending layout.

#include <cstdint>
#include <vector>

#include <mipp.h>

#include "Module/Encoder/BCH/Encoder_BCH.hpp"
#include "Module/Encoder/BCH/Encoder_BCH_inter.hpp"
#include "Tools/Code/BCH/BCH_polynomial_generator.hpp"

namespace {
template <typename B>
struct Scalar : aff3ct::module::Encoder_BCH<B>
{
    using aff3ct::module::Encoder_BCH<B>::Encoder_BCH;
    void wave(const B* u, B* x) { this->_encode(u, x, 0); }
};

template <typename B>
struct Inter : aff3ct::module::Encoder_BCH_inter<B>
{
    using aff3ct::module::Encoder_BCH_inter<B>::Encoder_BCH_inter;
    void wave(const B* u, B* x) { this->_encode(u, x, 0); }
};

struct Base
{
    virtual ~Base() = default;
    virtual int32_t frames() const = 0;
    virtual void encode(const void* u, void* x, int32_t waves) = 0;
    virtual void generator(uint64_t* rows, int32_t words_per_row) = 0;
};

// AFF3CT derives the generator over Galois<B>, whose tables overflow an
// int8 element above m = 7. The generator is derived at int, and the
// encoder's generator object at B is built over GF(8) and given it.
template <typename B>
struct Generator : aff3ct::tools::BCH_polynomial_generator<B>
{
    explicit Generator(const std::vector<int>& g)
      : aff3ct::tools::BCH_polynomial_generator<B>(7, 1)
    {
        this->g.assign(g.begin(), g.end());
    }
};

template <typename B, typename E>
struct Comparator : Base
{
    // Declared before the encoder, which keeps a reference to its generator.
    Generator<B> poly;
    E encoder;
    int32_t k, n, lanes;
    std::vector<B> u, x;

    Comparator(int32_t k, int32_t n, const std::vector<int>& g, int32_t lanes)
      : poly(g)
      , encoder(k, n, poly)
      , k(k)
      , n(n)
      , lanes(lanes)
      , u((size_t)lanes * k, 0)
      , x((size_t)lanes * n, 0)
    {
    }

    int32_t frames() const override { return lanes; }

    void encode(const void* u_in, void* x_out, int32_t waves) override
    {
        const B* uu = static_cast<const B*>(u_in);
        B* xx = static_cast<B*>(x_out);
        for (int32_t w = 0; w < waves; w++)
            encoder.wave(uu + (size_t)w * lanes * k, xx + (size_t)w * lanes * n);
    }

    // Row i of G is the codeword of gf2 message bit i, which AFF3CT reads as
    // u[k - 1 - i]; gf2 codeword bit c is AFF3CT entry n - 1 - c.
    void generator(uint64_t* rows, int32_t words_per_row) override
    {
        for (int32_t first = 0; first < k; first += lanes)
        {
            const int32_t count = (k - first < lanes) ? k - first : lanes;
            for (int32_t f = 0; f < count; f++)
                u[(size_t)f * k + (k - 1 - (first + f))] = 1;
            encoder.wave(u.data(), x.data());
            for (int32_t f = 0; f < count; f++)
            {
                u[(size_t)f * k + (k - 1 - (first + f))] = 0;
                uint64_t* row = rows + (size_t)(first + f) * words_per_row;
                const B* cw = x.data() + (size_t)f * n;
                for (int32_t c = 0; c < n; c++)
                    if (cw[n - 1 - c])
                        row[c >> 6] |= 1ull << (c & 63);
            }
        }
    }
};

template <typename B>
Base* make(int32_t k, int32_t n, int32_t m, int32_t t, uint32_t prim, int32_t inter)
{
    try
    {
        std::vector<int> p(m + 1);
        for (int32_t i = 0; i <= m; i++)
            p[i] = (int)((prim >> i) & 1u);
        const aff3ct::tools::BCH_polynomial_generator<int> derived((1 << m) - 1, t, p);
        if (derived.get_n_rdncy() != n - k)
            return nullptr;
        if (inter)
            return new Comparator<B, Inter<B>>(k, n, derived.get_g(), mipp::N<B>());
        return new Comparator<B, Scalar<B>>(k, n, derived.get_g(), 1);
    }
    catch (...)
    {
        return nullptr;
    }
}
} // namespace

extern "C" {
// Null when AFF3CT rejects the code or derives a generator of another degree.
void* gf2_aff3ct_bch_new_i8(int32_t k, int32_t n, int32_t m, int32_t t, uint32_t prim, int32_t inter)
{
    return make<int8_t>(k, n, m, t, prim, inter);
}

void* gf2_aff3ct_bch_new_i32(int32_t k, int32_t n, int32_t m, int32_t t, uint32_t prim, int32_t inter)
{
    return make<int32_t>(k, n, m, t, prim, inter);
}

int32_t gf2_aff3ct_bch_frames(void* handle) { return static_cast<Base*>(handle)->frames(); }

void gf2_aff3ct_bch_encode(void* handle, const void* u, void* x, int32_t waves)
{
    static_cast<Base*>(handle)->encode(u, x, waves);
}

// Writes the k x n generator into `rows`, which the caller zeroes.
void gf2_aff3ct_bch_generator(void* handle, uint64_t* rows, int32_t words_per_row)
{
    static_cast<Base*>(handle)->generator(rows, words_per_row);
}

void gf2_aff3ct_bch_free(void* handle) { delete static_cast<Base*>(handle); }
}
