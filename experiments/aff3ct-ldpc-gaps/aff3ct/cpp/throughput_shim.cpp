// AFF3CT v4.7.0 NMS decoders: scalar flooding with a counting replay prototype,
// and the flooding and horizontal-layered decoders with inter-frame SIMD in
// f32, i16 and i8.
#include <algorithm>
#include <cmath>
#include <cstring>
#include <fstream>
#include <numeric>
#include <string>
#include <vector>

#include <typeinfo>

#include "Module/Decoder/LDPC/BP/Flooding/Decoder_LDPC_BP_flooding.hpp"
#include "Module/Decoder/LDPC/BP/Flooding/Decoder_LDPC_BP_flooding_inter.hpp"
#include "Module/Decoder/LDPC/BP/Horizontal_layered/Decoder_LDPC_BP_horizontal_layered.hpp"
#include "Module/Decoder/LDPC/BP/Horizontal_layered/Decoder_LDPC_BP_horizontal_layered_inter.hpp"
#include "Module/Decoder/LDPC/BP/Horizontal_layered/ONMS/Decoder_LDPC_BP_horizontal_layered_ONMS_inter.hpp"
#include "Tools/Code/LDPC/Update_rule/NMS/Update_rule_NMS_simd.hpp"
#include "Tools/Code/LDPC/AList/AList.hpp"
#include "Tools/Code/LDPC/Update_rule/NMS/Update_rule_NMS.hpp"

namespace {

// AFF3CT pairs each LLR type with a decision type of the same width.
template<typename Q>
struct Bit_type;
template<>
struct Bit_type<float>
{
    typedef int32_t type;
};
template<>
struct Bit_type<int16_t>
{
    typedef int16_t type;
};
template<>
struct Bit_type<int8_t>
{
    typedef int8_t type;
};
/// Fixed-point types round `value * scale` and saturate at `limit`.
template<typename Q>
struct Quantize
{
    static Q apply(float value, float scale, Q limit)
    {
        const float scaled = std::nearbyint(value * scale);
        if (scaled > (float)limit) return limit;
        if (scaled < -(float)limit) return (Q)-limit;
        return (Q)scaled;
    }
};
template<>
struct Quantize<float> {
    static float apply(float value, float, float) { return value; }
};

/// The counter a flooding decoder fills while it iterates.
struct Iteration_counter
{
    int iterations;
    Iteration_counter()
      : iterations(0)
    {
    }
};

/// Counts AFF3CT's own flooding iterations without changing them.
template<typename Q, class Update_rule>
class Counting_flooding
  : public aff3ct::module::Decoder_LDPC_BP_flooding<typename Bit_type<Q>::type, Q, Update_rule>
{
    typedef typename Bit_type<Q>::type B;
    typedef aff3ct::module::Decoder_LDPC_BP_flooding<B, Q, Update_rule> Base;

  public:
    Iteration_counter counter;

    Counting_flooding(const int K,
                      const int N,
                      const int n_ite,
                      const aff3ct::tools::Sparse_matrix& H,
                      const std::vector<unsigned>& info_bits_pos,
                      const Update_rule& up_rule,
                      const bool enable_syndrome,
                      const int syndrome_depth)
      : Base(K, N, n_ite, H, info_bits_pos, up_rule, enable_syndrome, syndrome_depth)
      , counter()
    {
    }

  protected:
    void _decode_single_ite(const std::vector<Q>& msg_var_to_chk, std::vector<Q>& msg_chk_to_var)
    {
        this->counter.iterations += 1;
        Base::_decode_single_ite(msg_var_to_chk, msg_chk_to_var);
    }
};

/// What every handle exposes to the C ABI.
struct Handle_base
{
    virtual ~Handle_base() {}
    virtual int frames_per_wave() const = 0;
    virtual const char* name() const = 0;
    /// Whether this decoder reports its own iteration count.
    virtual bool has_counter() const = 0;
    /// Decodes `frames` frames; `ites` may be null when counts are not wanted.
    virtual int decode(const float* llrs, unsigned char* bits, int frames, int* ites) = 0;
    /// A handle over AFF3CT's `clone()` of the decoder; it has no counter.
    virtual Handle_base* clone() const = 0;
};

template<typename Q, class Decoder>
Handle_base* make_handle(Decoder* built, Iteration_counter* counter, int K, int N, int wave, float scale, Q limit);

/// A handle owning one AFF3CT decoder and the buffers its calls need.
template<typename Q, class Decoder>
struct Owned_handle : Handle_base
{
    Decoder* decoder;
    Iteration_counter* counter;
    int K;
    int N;
    int wave;
    float scale;
    Q limit;
    std::string decoder_name;
    std::vector<Q> input;
    std::vector<typename Bit_type<Q>::type> output;

    Owned_handle(Decoder* built, Iteration_counter* counter_, int K_, int N_, int wave_, float scale_, Q limit_)
      : decoder(built)
      , counter(counter_)
      , K(K_)
      , N(N_)
      , wave(wave_)
      , scale(scale_)
      , limit(limit_)
      , decoder_name(built->get_name() + " backend=" + mipp::InstructionFullType + " wave=" + std::to_string(wave_))
      , input((size_t)wave_ * (size_t)N_)
      , output((size_t)wave_ * (size_t)K_)
    {
        decoder->set_n_frames((size_t)wave_);
    }

    ~Owned_handle() { delete decoder; }

    int frames_per_wave() const { return wave; }
    const char* name() const { return decoder_name.c_str(); }
    bool has_counter() const { return counter != 0; }
    Handle_base* clone() const { return make_handle<Q>(decoder->clone(), 0, K, N, wave, scale, limit); }

    int decode(const float* llrs, unsigned char* bits, int frames, int* ites)
    {
        const int waves = frames / wave;
        for (int w = 0; w < waves; w++)
        {
            const float* source = llrs + (size_t)w * (size_t)wave * (size_t)N;
            for (size_t i = 0; i < input.size(); i++)
                input[i] = Quantize<Q>::apply(source[i], scale, limit);
            if (counter != 0) counter->iterations = 0;
            const int status = decoder->decode_siho(input.data(), output.data(), -1, false);
            unsigned char* target = bits + (size_t)w * (size_t)wave * (size_t)K;
            for (size_t i = 0; i < output.size(); i++)
                target[i] = (unsigned char)(output[i] != 0 ? 1 : 0);
            if (ites != 0)
            {
                // Negative counts identify a plain clone without a counter.
                ites[w] = counter != 0 ? counter->iterations : -1 - status;
            }
        }
        return 0;
    }
};

template<typename Q, class Decoder>
Handle_base* make_handle(Decoder* built, Iteration_counter* counter, int K, int N, int wave, float scale, Q limit)
{
    return new Owned_handle<Q, Decoder>(built, counter, K, N, wave, scale, limit);
}

/// The information-bit positions AFF3CT uses when no encoder supplies them:
/// `Codec_LDPC` fills the vector with 0..K-1 (`src/Tools/Codec/LDPC/Codec_LDPC.cpp`
/// lines 125-136), which is the information window this survey scores.
std::vector<unsigned> iota_positions(int K)
{
    std::vector<unsigned> positions((size_t)K);
    std::iota(positions.begin(), positions.end(), 0u);
    return positions;
}

void report(char* err, int errlen, const std::string& message)
{
    if (err == 0 || errlen <= 0) return;
    const int length = std::min((int)message.size(), errlen - 1);
    std::memcpy(err, message.data(), (size_t)length);
    err[length] = '\0';
}

/// What `a3_new` passes to every decoder constructor.
struct Build
{
    int K;
    int N;
    int n_ite;
    const aff3ct::tools::Sparse_matrix* H;
    float norm_factor;
    bool enable_syndrome;
    int syndrome_depth;
    float scale;
    float limit;
};

template<typename Q, template<class, class, class> class Family, class Rule>
Handle_base* build_ruled(const Build& b, const Rule& rule, int wave)
{
    typedef typename Bit_type<Q>::type B;
    Family<B, Q, Rule>* decoder =
      new Family<B, Q, Rule>(b.K, b.N, b.n_ite, *b.H, iota_positions(b.K), rule, b.enable_syndrome, b.syndrome_depth);
    return make_handle<Q>(decoder, 0, b.K, b.N, wave, b.scale, (Q)b.limit);
}

/// AFF3CT's fixed-point SIMD rule takes the factor as a count of eighths.
template<typename Q, template<class, class, class> class Family>
Handle_base* build_inter(const Build& b)
{
    using aff3ct::tools::Update_rule_NMS_simd;
    const int wave = mipp::N<Q>();
    if (typeid(Q) == typeid(float))
        return build_ruled<Q, Family, Update_rule_NMS_simd<Q> >(b, Update_rule_NMS_simd<Q>(b.norm_factor), wave);
    const float eighths = b.norm_factor * 8.f;
#define GF2_EIGHTHS(F)                                                                                                 \
    if (eighths == (float)F)                                                                                           \
        return build_ruled<Q, Family, Update_rule_NMS_simd<Q, F> >(b, Update_rule_NMS_simd<Q, F>(b.norm_factor), wave);
    GF2_EIGHTHS(1) GF2_EIGHTHS(2) GF2_EIGHTHS(3) GF2_EIGHTHS(4)
    GF2_EIGHTHS(5) GF2_EIGHTHS(6) GF2_EIGHTHS(7) GF2_EIGHTHS(8)
#undef GF2_EIGHTHS
    throw std::invalid_argument("a fixed-point normalization factor is a whole number of eighths");
}

template<typename Q>
Handle_base* build_onms(const Build& b)
{
    typedef typename Bit_type<Q>::type B;
    typedef aff3ct::module::Decoder_LDPC_BP_horizontal_layered_ONMS_inter<B, Q> Decoder;
    Decoder* decoder = new Decoder(b.K, b.N, b.n_ite, *b.H, iota_positions(b.K), b.norm_factor, (Q)0,
                                   b.enable_syndrome, b.syndrome_depth);
    return make_handle<Q>(decoder, 0, b.K, b.N, mipp::N<Q>(), b.scale, (Q)b.limit);
}

template<typename Q>
Handle_base* build_simd(const std::string& kind, const Build& b)
{
    if (kind == "flooding") return build_inter<Q, aff3ct::module::Decoder_LDPC_BP_flooding_inter>(b);
    if (kind == "layered") return build_inter<Q, aff3ct::module::Decoder_LDPC_BP_horizontal_layered_inter>(b);
    if (kind == "layered-onms") return build_onms<Q>(b);
    throw std::invalid_argument("unknown decoder type " + kind);
}

} // namespace

extern "C" {

void* a3_new(const char* alist_path,
             int K,
             int N,
             int n_ite,
             const char* kind,
             const char* implem,
             const char* simd,
             const char* precision,
             float norm_factor,
             float /*offset*/,
             int enable_syndrome,
             int syndrome_depth,
             float quant_scale,
             float quant_limit,
             char* err,
             int errlen)
{
    try
    {
        std::ifstream stream(alist_path);
        if (!stream.is_open())
        {
            report(err, errlen, std::string("cannot open ") + alist_path);
            return 0;
        }
        aff3ct::tools::Sparse_matrix H = aff3ct::tools::AList::read(stream);
        if ((int)H.get_n_rows() != N)
        {
            report(err, errlen,
                   "the AList holds " + std::to_string(H.get_n_rows()) +
                     " variable nodes, not " + std::to_string(N));
            return 0;
        }
        if (std::string(implem) != "NMS" || syndrome_depth != 1)
            throw std::invalid_argument("only NMS at syndrome depth one is supported");
        const std::string type(kind), strategy(simd), width(precision);
        const Build b = { K, N, n_ite, &H, norm_factor, enable_syndrome != 0, syndrome_depth, quant_scale, quant_limit };
        if (strategy == "INTER")
        {
            if (width == "f32") return build_simd<float>(type, b);
            if (width == "i16") return build_simd<int16_t>(type, b);
            if (width == "i8") return build_simd<int8_t>(type, b);
            throw std::invalid_argument("unknown precision " + width);
        }
        if (strategy != "" || width != "f32")
            throw std::invalid_argument("a decoder without SIMD is f32");
        if (type == "layered")
        {
            typedef aff3ct::tools::Update_rule_NMS<float> Rule;
            return build_ruled<float, aff3ct::module::Decoder_LDPC_BP_horizontal_layered, Rule>(b, Rule(norm_factor), 1);
        }
        if (type != "flooding") throw std::invalid_argument("unknown decoder type " + type);
        typedef aff3ct::tools::Update_rule_NMS<float> Rule;
        typedef Counting_flooding<float, Rule> Decoder;
        Decoder* decoder = new Decoder(K, N, n_ite, H, iota_positions(K), Rule(norm_factor),
                                        enable_syndrome != 0, syndrome_depth);
        Handle_base* handle = new Owned_handle<float, Decoder>(decoder, &decoder->counter, K, N, 1, 1.f, 0.f);
        return handle;
    }
    catch (const std::exception& error)
    {
        report(err, errlen, error.what());
        return 0;
    }
    catch (...)
    {
        report(err, errlen, "aff3ct threw a non-standard exception");
        return 0;
    }
}

int a3_has_counter(void* h)
{
    if (h == 0) return 0;
    return static_cast<Handle_base*>(h)->has_counter() ? 1 : 0;
}

int a3_frames_per_wave(void* h)
{
    if (h == 0) return -1;
    return static_cast<Handle_base*>(h)->frames_per_wave();
}

const char* a3_name(void* h)
{
    if (h == 0) return "";
    return static_cast<Handle_base*>(h)->name();
}

int a3_decode(void* h, const float* llrs, unsigned char* bits, int frames)
{
    try
    {
        if (h == 0 || llrs == 0 || bits == 0 || frames <= 0) return 1;
        Handle_base* handle = static_cast<Handle_base*>(h);
        const int wave = handle->frames_per_wave();
        if (wave <= 0 || frames % wave != 0) return 1;
        return handle->decode(llrs, bits, frames, 0);
    }
    catch (...)
    {
        return 2;
    }
}

int a3_decode_counted(void* h, const float* llrs, unsigned char* bits, int frames, int* ites)
{
    try
    {
        if (h == 0 || llrs == 0 || bits == 0 || ites == 0 || frames <= 0) return 1;
        Handle_base* handle = static_cast<Handle_base*>(h);
        const int wave = handle->frames_per_wave();
        if (wave <= 0 || frames % wave != 0) return 1;
        return handle->decode(llrs, bits, frames, ites);
    }
    catch (...)
    {
        return 2;
    }
}

void* a3_clone(void* h)
{
    try {
        if (h == 0) return 0;
        return static_cast<Handle_base*>(h)->clone();
    } catch (...) { return 0; }
}

void a3_free(void* h)
{
    delete static_cast<Handle_base*>(h);
}

} // extern "C"
