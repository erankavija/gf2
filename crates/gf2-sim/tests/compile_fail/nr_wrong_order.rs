//! Calling `.lifting_size()` before `.base_graph()` must NOT compile: the
//! typestate marker enforces the 5G NR builder call order.
//! `Pipeline::nr_5g()` returns a `Builder<NeedsBaseGraph>`, which has no
//! `lifting_size` method — that method exists only on `Builder<NeedsLifting>`,
//! reached after `.base_graph(...)`.

use gf2_sim::Pipeline;

fn main() {
    let _ = Pipeline::nr_5g().lifting_size(384);
}
