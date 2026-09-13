# Post-freeze check-node pilot rerun

This immutable receipt records a second execution of campaign
`07ca8585-v4-r1-update-checknode-pilot`. It completed after the branch merged
current `main` and the standalone arms workspace was rebuilt. The rebuild
changed executable digests because dependency updates and formatting-only
changes invalidated Cargo's build products; it did not change this family's
measurement behavior or the prepared check-node inputs and outputs.

The first accepted pilot remains at
`../v4-r1-07ca8585-ldpc-update-checknode-pilot/` and remains the resolution
evidence named by the already-frozen confirmation addendum. The Zen 3
measurement contract makes source and executable identities authoritative for
reproduction, while unrelated repository changes do not invalidate existing
evidence. This rerun is retained as observed evidence but does not rewrite the
frozen addendum or consume an additional confirmatory attempt.
