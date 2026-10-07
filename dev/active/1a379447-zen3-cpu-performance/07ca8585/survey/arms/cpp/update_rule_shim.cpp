// C-ABI shim exposing one AFF3CT v4.7.0 flooding check-node pass (jit:07ca8585).
//
// The pinned `c077a88b` shim builds whole AFF3CT decoders only, so an isolated
// check-node comparison has nothing to call. This unit adds exactly that entry
// point and nothing else: `a3u_check_pass` runs the loop the body of
// `aff3ct::module::Decoder_LDPC_BP_flooding::_decode_single_ite`
// (`include/Module/Decoder/LDPC/BP/Flooding/Decoder_LDPC_BP_flooding.hxx`
// lines 234-256) performs over one prepared variable-to-check message array,
// through AFF3CT's own `tools::Update_rule_MS`, `Update_rule_NMS` or
// `Update_rule_OMS`. The update rule and the scan order are AFF3CT's; this unit
// adds no arithmetic.
//
// The transpose index array is the one that decoder's constructor builds (same
// file, lines 41-71): for the k-th check-major branch it holds the
// variable-major slot of the same edge. The constructor recomputes each slot's
// variable-major base with an inner loop over every earlier variable, which is
// quadratic in the node count; this unit accumulates the same bases in one
// prefix pass, which yields the identical array.
//
// Its C entry points are named `a3u_*`, disjoint from the pinned shims' `a3_*`,
// so a binary may link this unit beside them.
//
// It must be compiled with the definitions the pinned static library was built
// with (-DAFF3CT_EXT_STRINGS -DAFF3CT_MULTI_PREC -DAFF3CT_POLAR_BIT_PACKING
// -DMIPP_ENABLE_BACKTRACE -DSPU_COLORS -DSPU_STACKTRACE -DNDEBUG) and with
// -std=gnu++11: the library's class layouts are gated on those definitions and
// a mismatched translation unit corrupts its heap on the first construction.

#include <cstring>
#include <fstream>
#include <string>
#include <vector>

#include "Tools/Code/LDPC/AList/AList.hpp"
#include "Tools/Code/LDPC/Update_rule/MS/Update_rule_MS.hpp"
#include "Tools/Code/LDPC/Update_rule/NMS/Update_rule_NMS.hpp"
#include "Tools/Code/LDPC/Update_rule/OMS/Update_rule_OMS.hpp"

namespace {

/// What every handle exposes to the C ABI.
struct Pass_base
{
    virtual ~Pass_base() {}
    virtual int checks() const = 0;
    virtual int variables() const = 0;
    virtual int edges() const = 0;
    /// Copies the check-major-to-variable-major edge permutation out.
    virtual void transpose_into(unsigned* out) const = 0;
    virtual const char* name() const = 0;
    /// One flooding check-node pass over a variable-major message array.
    virtual void check_pass(const float* var_to_chk, float* chk_to_var) = 0;
};

/// The prepared graph and one AFF3CT update rule.
template<class Rule>
struct Pass : Pass_base
{
    aff3ct::tools::Sparse_matrix H;
    Rule rule;
    /// Variable-major slot of each check-major branch.
    std::vector<unsigned> transpose;
    /// Degree of each check node, in check order.
    std::vector<int> degrees;
    std::string rule_name;

    Pass(const aff3ct::tools::Sparse_matrix& matrix, const Rule& up_rule)
      : H(matrix)
      , rule(up_rule)
      , transpose(matrix.get_n_connections())
      , degrees()
      , rule_name()
    {
        typedef aff3ct::tools::Sparse_matrix::Idx_t Idx;
        const std::vector<std::vector<Idx> >& chk_to_var = H.get_col_to_rows();
        const std::vector<std::vector<Idx> >& var_to_chk = H.get_row_to_cols();
        const size_t variables = var_to_chk.size();
        std::vector<unsigned> base(variables + 1, 0u);
        for (size_t v = 0; v < variables; v++)
            base[v + 1] = base[v] + (unsigned)var_to_chk[v].size();
        std::vector<unsigned> filled(variables, 0u);
        size_t k = 0;
        degrees.reserve(chk_to_var.size());
        for (size_t c = 0; c < chk_to_var.size(); c++)
        {
            degrees.push_back((int)chk_to_var[c].size());
            for (size_t j = 0; j < chk_to_var[c].size(); j++)
            {
                const Idx var_id = chk_to_var[c][j];
                transpose[k++] = base[var_id] + filled[var_id];
                filled[var_id]++;
            }
        }
        rule_name = "aff3ct Update_rule_" + rule.get_name() + " flooding check pass";
        // The decoder calls `begin_decoding` once per decode; for the min-sum
        // family it only records the iteration cap.
        rule.begin_decoding(1);
    }

    ~Pass() { rule.end_decoding(); }

    int checks() const { return (int)degrees.size(); }
    int variables() const { return (int)H.get_n_rows(); }
    int edges() const { return (int)transpose.size(); }

    void transpose_into(unsigned* out) const
    {
        std::memcpy(out, transpose.data(), transpose.size() * sizeof(unsigned));
    }

    const char* name() const { return rule_name.c_str(); }

    void check_pass(const float* var_to_chk, float* chk_to_var)
    {
        const unsigned* branch = transpose.data();
        const int n_chk_nodes = (int)degrees.size();
        rule.begin_ite(0);
        for (int c = 0; c < n_chk_nodes; c++)
        {
            const int chk_degree = degrees[(size_t)c];

            rule.begin_chk_node_in(c, chk_degree);
            for (int v = 0; v < chk_degree; v++)
                rule.compute_chk_node_in(v, var_to_chk[branch[v]]);
            rule.end_chk_node_in();

            rule.begin_chk_node_out(c, chk_degree);
            for (int v = 0; v < chk_degree; v++)
                chk_to_var[branch[v]] = rule.compute_chk_node_out(v, var_to_chk[branch[v]]);
            rule.end_chk_node_out();

            branch += chk_degree;
        }
        rule.end_ite();
    }
};

void report(char* err, int errlen, const std::string& message)
{
    if (err == 0 || errlen <= 0) return;
    const int length = (int)(message.size() < (size_t)(errlen - 1) ? message.size()
                                                                  : (size_t)(errlen - 1));
    std::memcpy(err, message.data(), (size_t)length);
    err[length] = '\0';
}

} // namespace

extern "C" {

/// Builds a check-node pass over the AList at `alist_path` with the update rule
/// `implem` names. Returns null and fills `err` on failure.
void* a3u_new(const char* alist_path,
              const char* implem,
              float norm_factor,
              float offset,
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
        const std::string implem_s(implem);
        if (implem_s == "MS")
            return new Pass<aff3ct::tools::Update_rule_MS<float> >(
              H, aff3ct::tools::Update_rule_MS<float>());
        if (implem_s == "NMS")
            return new Pass<aff3ct::tools::Update_rule_NMS<float> >(
              H, aff3ct::tools::Update_rule_NMS<float>(norm_factor));
        if (implem_s == "OMS")
            return new Pass<aff3ct::tools::Update_rule_OMS<float> >(
              H, aff3ct::tools::Update_rule_OMS<float>(offset));
        report(err, errlen, "implem " + implem_s + " is not one of MS, NMS, OMS");
        return 0;
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

int a3u_checks(void* h) { return h == 0 ? -1 : static_cast<Pass_base*>(h)->checks(); }

int a3u_variables(void* h) { return h == 0 ? -1 : static_cast<Pass_base*>(h)->variables(); }

int a3u_edges(void* h) { return h == 0 ? -1 : static_cast<Pass_base*>(h)->edges(); }

const char* a3u_name(void* h) { return h == 0 ? "" : static_cast<Pass_base*>(h)->name(); }

/// Copies the check-major-to-variable-major edge permutation into `out`, which
/// must hold `a3u_edges` entries.
int a3u_transpose(void* h, unsigned* out, int len)
{
    if (h == 0 || out == 0) return 1;
    Pass_base* pass = static_cast<Pass_base*>(h);
    if (len != pass->edges()) return 1;
    pass->transpose_into(out);
    return 0;
}

/// Runs one flooding check-node pass. Both arrays are variable-major and hold
/// `a3u_edges` floats.
int a3u_check_pass(void* h, const float* var_to_chk, float* chk_to_var)
{
    try
    {
        if (h == 0 || var_to_chk == 0 || chk_to_var == 0) return 1;
        static_cast<Pass_base*>(h)->check_pass(var_to_chk, chk_to_var);
        return 0;
    }
    catch (...)
    {
        return 2;
    }
}

void a3u_free(void* h) { delete static_cast<Pass_base*>(h); }

} // extern "C"
