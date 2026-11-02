# Check-variant nomination

Record for @/issue/a72f2b3f. No variant is nominated. The comparison is exploratory, on one host (AMD Ryzen 9 5900X), in a pilot of twelve pairs per cell. It supports no performance claim.

## Variants

Two cumulative scalar variants of `min_sum_check_row` in `crates/gf2-coding/src/ldpc/min_sum.rs`, each a patch built as its own executable from an otherwise unchanged tree, as the `check-variants` contract of `plan.md` specifies:

- `cached-signed` computes the normalized signed minima once per check and selects among them.
- `select-reduced` adds comparison and select reduction to `cached-signed`.

The patches are `experiments/aff3ct-ldpc-gaps/patches/cached-signed.patch` and `select-reduced.patch`. Production source is unchanged.

## Contract and posterior equality (REQ-01, REQ-02)

- Check-row contract: both patched trees pass `ldpc_check_update_contract` (13 tests run, 13 passed), whose row outputs equal the independent scalar reference bit for bit, covering strict ties, signed zero, NaN, infinity, low degrees and every three-element row over a ten-value special, finite and subnormal alphabet. The three-element coverage is the test added by this change.
- Cross-executable posteriors: each variant's dump equals the unchanged-production executable's byte for byte (433,492,256 bytes, identical output SHA-256 `7dec553e4b63bf8ec0dc0b29a3497f9f5060766d40de5af78a47fac291ddb4b8`). The dump covers 2,384 decoded cases and 108,367,104 posterior words per executable: all 128 corpus frames of each of DVB-T2 R1/2 and NR BG1 Z=384 at cap 50, seven special-value patterns per code at caps 0, 1 and 8, four rules and both stopping settings. It also compares iteration counts and terminal flags. Both variants were timed.

## Pilot (REQ-03)

Campaign `55c3e433-check-variants-pilot`, family `55c3e433-check-variants`, receipt SHA-256 `64dc191d5d728b430a9c624ee2afcf5139f81175b1232d543f0efc73c79366aa`, Rust 1.95, 8 sessions, verdict Accepted, no flagged window in any cell (0/120 each). Speedup is unchanged-production time divided by variant time; the bound is the lower end of the 0.975 interval. Cells `w1-b1` and `p12-b8` on both codes.

On the 5900X host, `cached-signed` is slower than unchanged production on every cell and `select-reduced` is faster on every cell. The receipt holds each cell's speedup and interval.

## Branches in the reduction (REQ-04)

From disassembly of each measured executable's check row, preserved outside the tracked tree:

- `cached-signed` keeps both data-dependent minima branches in every reduction clone. No conditional branch of the reduction is removed.
- `select-reduced` removes the second-minimum branch, which becomes `vminss`, and keeps the first-minimum branch in every clone; position selection uses `cmova`. The reduction's conditional branches are reduced from two to one, not removed.

## Nomination rule (REQ-05)

A variant is eligible when its bits match, its lower bound exceeds $1/1.1 = 0.9091$ on all four cells, and its lower bound reaches 1.1 on at least one cell.

- `cached-signed`: bits match; lower bounds exceed 0.9091 on all four cells (the smallest, 0.9092, by 0.0001); the largest lower bound is 0.9480, below 1.1. Not eligible.
- `select-reduced`: bits match; lower bounds exceed 0.9091 on all four cells; the largest lower bound is 1.0788 (NR `w1-b1`), below 1.1. Not eligible.

**No variant is nominated.** The negative result is preserved: the check-update variants fall short of the 1.1 margin on this host and these cells. Production is unchanged.

## Load at the pilot's start

The job started under load left by the NR pilot's 24-thread cells, which ended moments earlier; the window log records the figure. The receipt flags no window in any cell. The owner reviewed this and accepted the receipt.
