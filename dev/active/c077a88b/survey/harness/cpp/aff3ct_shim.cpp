// C-ABI shim exposing the pinned AFF3CT v4.7.0 LDPC decoders (jit:c077a88b).
//
// Every decoder this shim builds is an AFF3CT class constructed with the same
// arguments the AFF3CT factory `src/Factory/Module/Decoder/LDPC/Decoder_LDPC.cpp`
// passes; the shim adds no update rule and no decoding loop of its own. The one
// derived class here, `Counting_flooding`, overrides the virtual
// `_decode_single_ite` of `Decoder_LDPC_BP_flooding` only to increment a counter
// before delegating to the base implementation, so the decoding itself stays
// AFF3CT's.
//
// It must be compiled with the definitions the pinned static library was built
// with (-DAFF3CT_EXT_STRINGS -DAFF3CT_MULTI_PREC -DAFF3CT_POLAR_BIT_PACKING
// -DMIPP_ENABLE_BACKTRACE -DSPU_COLORS -DSPU_STACKTRACE -DNDEBUG) and with
// -std=gnu++11: the library's class layouts are gated on those definitions and
// a mismatched translation unit corrupts its heap on the first construction.

#include <algorithm>
#include <cmath>
#include <cstring>
#include <fstream>
#include <numeric>
#include <string>
#include <vector>

#include "Module/Decoder/LDPC/BP/Flooding/Decoder_LDPC_BP_flooding.hpp"
#include "Module/Decoder/LDPC/BP/Horizontal_layered/Decoder_LDPC_BP_horizontal_layered.hpp"
#include "Module/Decoder/LDPC/BP/Horizontal_layered/ONMS/Decoder_LDPC_BP_horizontal_layered_ONMS_inter.hpp"
#include "Tools/Code/LDPC/AList/AList.hpp"
#include "Tools/Code/LDPC/Update_rule/MS/Update_rule_MS.hpp"
#include "Tools/Code/LDPC/Update_rule/NMS/Update_rule_NMS.hpp"
#include "Tools/Code/LDPC/Update_rule/OMS/Update_rule_OMS.hpp"

namespace {

/// AFF3CT pairs one bit type with each real type; its explicit
/// instantiations are (B_8, Q_8) = (int8_t, int8_t), (B_16, Q_16) =
/// (int16_t, int16_t) and (B_32, Q_32) = (int32_t, float)
/// (`src/.../Decoder_LDPC_BP_horizontal_layered_ONMS_inter.cpp` lines 562-565).
/// A shim that mixed the pair would reference a symbol the pinned library does
/// not carry.
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

/// Quantizes a recorded f32 LLR into the decoder's own representation.
template<typename Q>
struct Quantize
{
    static Q apply(float value, float scale, Q limit)
    {
        const float scaled = std::floor(value * scale + 0.5f);
        const float bounded =
          std::min(std::max(scaled, -(float)limit), (float)limit);
        return (Q)bounded;
    }
};

template<>
struct Quantize<float>
{
    static float apply(float value, float /*scale*/, float /*limit*/) { return value; }
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
};

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
        // An AFF3CT inter decoder consumes a whole wave per call, so the
        // module must carry as many frames as its wave holds; a module left at
        // one frame decodes only the first frame of each wave and leaves the
        // rest of the output untouched.
        decoder->set_n_frames((size_t)wave_);
    }

    ~Owned_handle() { delete decoder; }

    int frames_per_wave() const { return wave; }
    const char* name() const { return decoder_name.c_str(); }
    bool has_counter() const { return counter != 0; }

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
                // A decoder AFF3CT exposes no iteration hook for reports the
                // syndrome outcome instead, encoded negatively: -1 means the
                // syndrome passed and -2 that it never did. The caller then
                // observes the convergence iteration by repeating the decode
                // under a smaller iteration cap.
                ites[w] = counter != 0 ? counter->iterations : -1 - status;
            }
        }
        return 0;
    }
};

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

/// Builds a scalar flooding decoder with AFF3CT's own update rule.
template<typename Q>
Handle_base* build_flooding(const std::string& implem,
                            int K,
                            int N,
                            int n_ite,
                            const aff3ct::tools::Sparse_matrix& H,
                            float norm_factor,
                            float offset,
                            bool enable_syndrome,
                            int syndrome_depth,
                            float scale,
                            Q limit,
                            std::string& failure)
{
    const std::vector<unsigned> positions = iota_positions(K);
    if (implem == "MS")
    {
        typedef aff3ct::tools::Update_rule_MS<Q> Rule;
        Counting_flooding<Q, Rule>* decoder = new Counting_flooding<Q, Rule>(
          K, N, n_ite, H, positions, Rule(), enable_syndrome, syndrome_depth);
        return new Owned_handle<Q, Counting_flooding<Q, Rule> >(
          decoder, &decoder->counter, K, N, 1, scale, limit);
    }
    if (implem == "NMS")
    {
        typedef aff3ct::tools::Update_rule_NMS<Q> Rule;
        Counting_flooding<Q, Rule>* decoder = new Counting_flooding<Q, Rule>(
          K, N, n_ite, H, positions, Rule(norm_factor), enable_syndrome, syndrome_depth);
        return new Owned_handle<Q, Counting_flooding<Q, Rule> >(
          decoder, &decoder->counter, K, N, 1, scale, limit);
    }
    if (implem == "OMS")
    {
        typedef aff3ct::tools::Update_rule_OMS<Q> Rule;
        Counting_flooding<Q, Rule>* decoder = new Counting_flooding<Q, Rule>(
          K, N, n_ite, H, positions, Rule((Q)offset), enable_syndrome, syndrome_depth);
        return new Owned_handle<Q, Counting_flooding<Q, Rule> >(
          decoder, &decoder->counter, K, N, 1, scale, limit);
    }
    failure = "flooding implem " + implem + " is not one of MS, NMS, OMS";
    return 0;
}

/// Builds a scalar horizontal-layered decoder with AFF3CT's own update rule.
template<typename Q>
Handle_base* build_layered(const std::string& implem,
                           int K,
                           int N,
                           int n_ite,
                           const aff3ct::tools::Sparse_matrix& H,
                           float norm_factor,
                           float offset,
                           bool enable_syndrome,
                           int syndrome_depth,
                           float scale,
                           Q limit,
                           std::string& failure)
{
    const std::vector<unsigned> positions = iota_positions(K);
    if (implem == "MS")
    {
        typedef aff3ct::tools::Update_rule_MS<Q> Rule;
        typedef aff3ct::module::Decoder_LDPC_BP_horizontal_layered<typename Bit_type<Q>::type, Q, Rule> Decoder;
        Decoder* decoder =
          new Decoder(K, N, n_ite, H, positions, Rule(), enable_syndrome, syndrome_depth);
        return new Owned_handle<Q, Decoder>(decoder, 0, K, N, 1, scale, limit);
    }
    if (implem == "NMS")
    {
        typedef aff3ct::tools::Update_rule_NMS<Q> Rule;
        typedef aff3ct::module::Decoder_LDPC_BP_horizontal_layered<typename Bit_type<Q>::type, Q, Rule> Decoder;
        Decoder* decoder =
          new Decoder(K, N, n_ite, H, positions, Rule(norm_factor), enable_syndrome, syndrome_depth);
        return new Owned_handle<Q, Decoder>(decoder, 0, K, N, 1, scale, limit);
    }
    if (implem == "OMS")
    {
        typedef aff3ct::tools::Update_rule_OMS<Q> Rule;
        typedef aff3ct::module::Decoder_LDPC_BP_horizontal_layered<typename Bit_type<Q>::type, Q, Rule> Decoder;
        Decoder* decoder =
          new Decoder(K, N, n_ite, H, positions, Rule((Q)offset), enable_syndrome, syndrome_depth);
        return new Owned_handle<Q, Decoder>(decoder, 0, K, N, 1, scale, limit);
    }
    failure = "layered implem " + implem + " is not one of MS, NMS, OMS";
    return 0;
}

/// Builds the inter-frame SIMD horizontal-layered decoder. AFF3CT exposes one
/// class for both normalized and offset min-sum: the factory passes the
/// normalization factor with a zero offset for NMS and a unit factor with the
/// declared offset for OMS.
template<typename Q>
Handle_base* build_layered_inter(const std::string& implem,
                                 int K,
                                 int N,
                                 int n_ite,
                                 const aff3ct::tools::Sparse_matrix& H,
                                 float norm_factor,
                                 float offset,
                                 bool enable_syndrome,
                                 int syndrome_depth,
                                 float scale,
                                 Q limit,
                                 std::string& failure)
{
    typedef aff3ct::module::Decoder_LDPC_BP_horizontal_layered_ONMS_inter<typename Bit_type<Q>::type, Q> Decoder;
    const std::vector<unsigned> positions = iota_positions(K);
    float factor;
    Q rule_offset;
    if (implem == "NMS")
    {
        factor = norm_factor;
        rule_offset = (Q)0;
    }
    else if (implem == "OMS")
    {
        factor = 1.f;
        rule_offset = (Q)offset;
    }
    else if (implem == "MS")
    {
        factor = 1.f;
        rule_offset = (Q)0;
    }
    else
    {
        failure = "layered inter implem " + implem + " is not one of MS, NMS, OMS";
        return 0;
    }
    Decoder* decoder = new Decoder(
      K, N, n_ite, H, positions, factor, rule_offset, enable_syndrome, syndrome_depth);
    const int wave = (int)decoder->get_n_frames_per_wave();
    return new Owned_handle<Q, Decoder>(decoder, 0, K, N, wave, scale, limit);
}

template<typename Q>
Handle_base* build_for_precision(const std::string& kind,
                                 const std::string& implem,
                                 const std::string& simd,
                                 int K,
                                 int N,
                                 int n_ite,
                                 const aff3ct::tools::Sparse_matrix& H,
                                 float norm_factor,
                                 float offset,
                                 bool enable_syndrome,
                                 int syndrome_depth,
                                 float scale,
                                 Q limit,
                                 std::string& failure)
{
    if (kind == "flooding" && simd.empty())
        return build_flooding<Q>(implem, K, N, n_ite, H, norm_factor, offset,
                                 enable_syndrome, syndrome_depth, scale, limit, failure);
    if (kind == "horizontal-layered" && simd.empty())
        return build_layered<Q>(implem, K, N, n_ite, H, norm_factor, offset,
                                enable_syndrome, syndrome_depth, scale, limit, failure);
    if (kind == "horizontal-layered" && simd == "INTER")
        return build_layered_inter<Q>(implem, K, N, n_ite, H, norm_factor, offset,
                                      enable_syndrome, syndrome_depth, scale, limit, failure);
    failure = "the pinned AFF3CT build offers no decoder for type " + kind + " with simd " +
              (simd.empty() ? std::string("none") : simd);
    return 0;
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
             float offset,
             int enable_syndrome,
             int syndrome_depth,
             float quant_scale,
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
        const std::string kind_s(kind);
        const std::string implem_s(implem);
        const std::string simd_s(simd);
        const std::string precision_s(precision);
        std::string failure;
        Handle_base* handle = 0;
        if (precision_s == "f32")
            handle = build_for_precision<float>(kind_s, implem_s, simd_s, K, N, n_ite, H,
                                                norm_factor, offset, enable_syndrome != 0,
                                                syndrome_depth, quant_scale, 0.f, failure);
        else if (precision_s == "i16")
            handle = build_for_precision<int16_t>(kind_s, implem_s, simd_s, K, N, n_ite, H,
                                                  norm_factor, offset, enable_syndrome != 0,
                                                  syndrome_depth, quant_scale, (int16_t)32767,
                                                  failure);
        else if (precision_s == "i8")
            handle = build_for_precision<int8_t>(kind_s, implem_s, simd_s, K, N, n_ite, H,
                                                 norm_factor, offset, enable_syndrome != 0,
                                                 syndrome_depth, quant_scale, (int8_t)127, failure);
        else
            failure = "precision " + precision_s + " is not one of f32, i16, i8";
        if (handle == 0) report(err, errlen, failure);
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

void a3_free(void* h)
{
    delete static_cast<Handle_base*>(h);
}

} // extern "C"
