# Check-variant nomination

Record for @/issue/a72f2b3f. No variant is nominated. The comparison is exploratory, on one host (AMD Ryzen 9 5900X), in a pilot of twelve pairs per cell. It supports no performance claim.

## Variants

Two cumulative scalar variants of `min_sum_check_row` in `crates/gf2-coding/src/ldpc/min_sum.rs`, each a patch built as its own executable from an otherwise unchanged tree, as the `check-variants` contract of `plan.md` specifies:

- `cached-signed` computes the normalized signed minima once per check and selects among them.
- `select-reduced` adds comparison and select reduction to `cached-signed`.

The patches are preserved outside the tracked tree with family `55c3e433-check-variants`. Production source is unchanged.

## Contract and posterior equality (REQ-01, REQ-02)

- Check-row contract: both patched trees pass `ldpc_check_update_contract` (13 tests run, 13 passed), whose row outputs equal the independent scalar reference bit for bit, covering strict ties, signed zero, NaN, infinity, low degrees and every three-element row over a ten-value special, finite and subnormal alphabet. The three-element coverage is the test added by this change.
- Cross-executable posteriors: each variant's dump equals the unchanged-production executable's byte for byte (433,492,256 bytes, identical output SHA-256 `7dec553e4b63bf8ec0dc0b29a3497f9f5060766d40de5af78a47fac291ddb4b8`). The dump covers 2,384 decoded cases and 108,367,104 posterior words per executable: all 128 corpus frames of each of DVB-T2 R1/2 and NR BG1 Z=384 at cap 50, seven special-value patterns per code at caps 0, 1 and 8, four rules and both stopping settings. It also compares iteration counts and terminal flags. Both variants were timed.

## Pilot (REQ-03)

Campaign `55c3e433-check-variants-pilot`, family `55c3e433-check-variants`, receipt SHA-256 `64dc191d5d728b430a9c624ee2afcf5139f81175b1232d543f0efc73c79366aa`, Rust 1.95, 8 sessions, verdict Accepted, no flagged window in any cell (0/120 each). Speedup is unchanged-production time divided by variant time; the bound is the lower end of the 0.975 interval. Cells `w1-b1` and `p12-b8` on both codes.

| Variant | Code | Cell | Speedup | Lower bound |
|---|---|---|---:|---:|
| `cached-signed` | DVB-T2 R1/2 | `w1-b1` | 0.9495 | 0.9480 |
| `cached-signed` | DVB-T2 R1/2 | `p12-b8` | 0.9462 | 0.9419 |
| `cached-signed` | NR BG1 Z=384 | `w1-b1` | 0.9242 | 0.9223 |
| `cached-signed` | NR BG1 Z=384 | `p12-b8` | 0.9237 | 0.9092 |
| `select-reduced` | DVB-T2 R1/2 | `w1-b1` | 1.0740 | 1.0719 |
| `select-reduced` | DVB-T2 R1/2 | `p12-b8` | 1.0554 | 1.0484 |
| `select-reduced` | NR BG1 Z=384 | `w1-b1` | 1.0805 | 1.0788 |
| `select-reduced` | NR BG1 Z=384 | `p12-b8` | 1.0727 | 1.0704 |

## Branches in the reduction (REQ-04)

From disassembly of each measured executable's check row, preserved outside the tracked tree:

- `cached-signed` keeps both data-dependent minima branches in every reduction clone. No conditional branch of the reduction is removed.
- `select-reduced` removes the second-minimum branch, which becomes `vminss`, and keeps the first-minimum branch in every clone; position selection uses `cmova`. The reduction's conditional branches are reduced from two to one, not removed.

## Nomination rule (REQ-05)

A variant is eligible when its bits match, its lower bound exceeds $1/1.1 = 0.9091$ on all four cells, and its lower bound reaches 1.1 on at least one cell.

- `cached-signed`: bits match; lower bounds exceed 0.9091 on all four cells (the smallest, 0.9092, by 0.0001); the largest lower bound is 0.9480, below 1.1. Not eligible.
- `select-reduced`: bits match; lower bounds exceed 0.9091 on all four cells; the largest lower bound is 1.0788 (NR `w1-b1`), below 1.1. Not eligible.

**No variant is nominated.** The negative result is preserved: the check update variants fall short of the 1.1 margin on this host and these cells, with the strongest point speedup 1.0805. Production is unchanged.

## Load at the pilot's start

The job started at 1-minute load 14.47 (`window.log`), immediately after the NR pilot's 24-thread cells ended; that pilot's last session recorded 1-minute load 12.83. The check-variant sessions recorded 1-minute loads of 8.58 to 12.47. No window was flagged (0 of 120 per cell, 192 runs), and the unchanged-production medians agree with the NR pilot's single-frame medians within about 2%: 27.05 ms against 26.47 ms at NR `w1-b1`, and 215.4 ms against 212.3 ms at NR `p12-b8`. The two pilots use separate unchanged builds. The owner approved the load finding.
