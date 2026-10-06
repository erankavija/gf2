// Worker replication for the pinned AFF3CT v4.7.0 LDPC decoders (jit:3be770d5).
//
// Includes the `c077a88b` shim unchanged, so every decoder, update rule,
// quantizer and C entry point stays the one that survey validated, and adds
// `a3_clone`: a new handle around the decoder's own public `clone()`, the
// route AFF3CT's multi-threaded simulations replicate a module with. A clone
// of the survey's iteration-counting flooding decoder is AFF3CT's plain
// flooding decoder: the counter only observes iterations and the timed calls
// do not read it.

#include "aff3ct_shim.cpp"

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

} // namespace

extern "C" {

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
        return copy;
    }
    catch (...)
    {
        return 0;
    }
}

} // extern "C"
