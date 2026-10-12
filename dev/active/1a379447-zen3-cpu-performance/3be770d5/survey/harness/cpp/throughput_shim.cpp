// Worker replication and the sum-product INTRA decoder for the pinned AFF3CT
// v4.7.0 [Cassagne2019] LDPC decoders (jit:3be770d5).
//
// Includes the `c077a88b` shim unchanged, so every decoder, update rule,
// quantizer and C entry point stays the one that survey validated, and adds
// two entries. `a3_clone` is a new handle around the decoder's own public
// `clone()`, the route AFF3CT's multi-threaded simulations replicate a module
// with. `a3_intra_new` builds `Decoder_LDPC_BP_flooding_SPA`, which `a3_new`
// does not, with the arguments AFF3CT's factory passes
// (`src/Factory/Module/Decoder/LDPC/Decoder_LDPC.cpp`, the `BP_FLOODING` /
// `INTRA` branch). A clone of an iteration-counting decoder is AFF3CT's plain
// decoder: the counter only observes iterations and the timed calls do not
// read it.

#include "aff3ct_shim.cpp"

#include "Module/Decoder/LDPC/BP/Flooding/SPA/Decoder_LDPC_BP_flooding_SPA.hpp"

namespace
{

/// Clones `source` when it owns a `Decoder`; `Copy` is the type `clone()`
/// returns for it.
template<typename Q, class Decoder, class Copy>
Handle_base* clone_as(Handle_base* source)
{
    Owned_handle<Q, Decoder>* owned = dynamic_cast<Owned_handle<Q, Decoder>*>(source);
    if (owned == 0) return 0;
    Copy* copy = owned->decoder->clone();
    return new Owned_handle<Q, Copy>(copy, 0, owned->K, owned->N, owned->wave, owned->scale, owned->limit);
}

/// Every decoder type `build_flooding` and `build_layered` construct for one
/// update rule.
template<typename Q, class Rule>
Handle_base* clone_rule(Handle_base* source)
{
    typedef typename Bit_type<Q>::type B;
    typedef aff3ct::module::Decoder_LDPC_BP_flooding<B, Q, Rule> Flooding;
    typedef aff3ct::module::Decoder_LDPC_BP_horizontal_layered<B, Q, Rule> Layered;
    Handle_base* copy = clone_as<Q, Counting_flooding<Q, Rule>, Flooding>(source);
    if (copy == 0) copy = clone_as<Q, Flooding, Flooding>(source);
    if (copy == 0) copy = clone_as<Q, Layered, Layered>(source);
    return copy;
}

template<typename Q>
Handle_base* clone_precision(Handle_base* source)
{
    typedef typename Bit_type<Q>::type B;
    typedef aff3ct::module::Decoder_LDPC_BP_horizontal_layered_ONMS_inter<B, Q> Inter;
    Handle_base* copy = clone_rule<Q, aff3ct::tools::Update_rule_MS<Q> >(source);
    if (copy == 0) copy = clone_rule<Q, aff3ct::tools::Update_rule_NMS<Q> >(source);
    if (copy == 0) copy = clone_rule<Q, aff3ct::tools::Update_rule_OMS<Q> >(source);
    if (copy == 0) copy = clone_as<Q, Inter, Inter>(source);
    return copy;
}

typedef aff3ct::module::Decoder_LDPC_BP_flooding_SPA<Bit_type<float>::type, float> Spa_intra;

/// Counts AFF3CT's own sum-product iterations without changing them.
class Counting_spa_intra : public Spa_intra
{
  public:
    Iteration_counter counter;

    Counting_spa_intra(const int K,
                       const int N,
                       const int n_ite,
                       const aff3ct::tools::Sparse_matrix& H,
                       const std::vector<unsigned>& info_bits_pos,
                       const bool enable_syndrome,
                       const int syndrome_depth)
      : Spa_intra(K, N, n_ite, H, info_bits_pos, enable_syndrome, syndrome_depth)
      , counter()
    {
    }

  protected:
    void _decode_single_ite(const std::vector<float>& msg_var_to_chk, std::vector<float>& msg_chk_to_var)
    {
        this->counter.iterations += 1;
        Spa_intra::_decode_single_ite(msg_var_to_chk, msg_chk_to_var);
    }
};

} // namespace

extern "C" {

/// Builds the iteration-counting INTRA decoder from an AList, or returns null
/// with the reason in `err`. The wave is the frame count AFF3CT reports for
/// one native decode.
void* a3_intra_new(const char* alist_path,
                   int K,
                   int N,
                   int n_ite,
                   int enable_syndrome,
                   int syndrome_depth,
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
        Counting_spa_intra* decoder = new Counting_spa_intra(
          K, N, n_ite, H, iota_positions(K), enable_syndrome != 0, syndrome_depth);
        const int wave = (int)decoder->get_n_frames_per_wave();
        return new Owned_handle<float, Counting_spa_intra>(decoder, &decoder->counter, K, N, wave, 1.f, 0.f);
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

/// Returns an independent handle decoding exactly as `h` does, or null when
/// `h` is null, not a shim handle, or AFF3CT's clone throws.
void* a3_clone(void* h)
{
    try
    {
        if (h == 0) return 0;
        Handle_base* source = static_cast<Handle_base*>(h);
        Handle_base* copy = clone_precision<float>(source);
        if (copy == 0) copy = clone_precision<int16_t>(source);
        if (copy == 0) copy = clone_precision<int8_t>(source);
        if (copy == 0) copy = clone_as<float, Counting_spa_intra, Spa_intra>(source);
        if (copy == 0) copy = clone_as<float, Spa_intra, Spa_intra>(source);
        return copy;
    }
    catch (...)
    {
        return 0;
    }
}

} // extern "C"
