# Survey review: ineligible for confirmation resolution

The protocol validator accepted this receipt mechanically. Subsequent source
inspection found that `ComparisonCode::build()` calls
`nr_5g_rate_matched()`, which invokes `compute_mother_encoding()`. Its NR
whole-call timing therefore includes encoder initialization that the external
decoder arm does not perform. This is not an operation-equivalent decoder
comparison. The receipt and acceptance summary remain unchanged as evidence
of the discovered defect; they do not resolve a confirmation addendum.

The r4 adapter loads the recorded AList directly into `LdpcCode::from_edges`,
so both timed arms perform AList loading, native sparse-matrix construction,
decoder construction, decode and output. No production kernel is changed.
The previous matched confirmation addendum is preserved under
`dev/active/c077a88b/superseded/`; no confirmation ran under it.
