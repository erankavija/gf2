// Float sum-product INTRA decoder of the pinned AFF3CT v4.7.0 [Cassagne2019]
// for the batch decoder comparison (jit:ed3d490e).
//
// Includes the `3be770d5` unit unchanged, and through it the `c077a88b` shim,
// so the handle, its decode loop and every C entry point stay the ones the
// pinned arms run. `a3_new` builds no INTRA decoder; the two entries below
// add AFF3CT's `Decoder_LDPC_BP_flooding_SPA` with the arguments its factory
// passes (`src/Factory/Module/Decoder/LDPC/Decoder_LDPC.cpp`, the
// `BP_FLOODING` / `INTRA` branch). A clone of the iteration-counting
// prototype is AFF3CT's plain decoder, as for the flooding arms.

#include "throughput_shim.cpp"

#include "Module/Decoder/LDPC/BP/Flooding/SPA/Decoder_LDPC_BP_flooding_SPA.hpp"

namespace
{

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

/// Builds the INTRA decoder from an AList, or returns null with the reason in
/// `err`. The wave is the frame count AFF3CT reports for one native decode.
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

/// Returns an independent handle decoding exactly as the INTRA handle `h`
/// does, or null when `h` is no INTRA handle or AFF3CT's clone throws.
void* a3_intra_clone(void* h)
{
    try
    {
        if (h == 0) return 0;
        Handle_base* source = static_cast<Handle_base*>(h);
        Handle_base* copy = clone_as<float, Counting_spa_intra, Spa_intra>(source);
        if (copy == 0) copy = clone_as<float, Spa_intra, Spa_intra>(source);
        return copy;
    }
    catch (...)
    {
        return 0;
    }
}

} // extern "C"
