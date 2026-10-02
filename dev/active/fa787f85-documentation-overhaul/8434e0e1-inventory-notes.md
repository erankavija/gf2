# Open-epic inventory notes (8434e0e1)

- `aed96ef9-finite-blocklength-bounds/external-review-2026-08-07.md` has five owners (aed96ef9, 55087229, b7157be6, c7cfd37e, cce5da8c) with one linked document each; the tie goes to aed96ef9, whose id names the entry, so the file stays in place.
- `fa787f85-documentation-overhaul` files linked from issues under epics fa787f85 and 6dc81018 record every owner; fa787f85 owns most linked documents and the entry stays in place.
- A path naming several issue ids (dispatch and rework prompts) records every named issue as owner; all resolve to one epic.
- A file with a depth-relative `REPO` or Cargo path (`../..`, `parents[n]`) is code-pinned by its own location and lists that line as its consumer.
- The three `ae03bcd0-general-bch/oracle` generating sources are digest-pinned by the digest table of `oracle-receipt.md`.
- A flat entry moves to `dev/active/<epic dir>/<entry>/<relative path>`; the entry directory name keeps generic file names collision-free.
