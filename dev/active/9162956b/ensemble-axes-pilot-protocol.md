# Predeclared pilot: the coverage-repairing ensemble axis

This fixes the pilot that decides whether the amended axis set of issue
`9162956b` reaches the coverage floor
[`layout-attribution-verdict-v1.md`](/dev/benchmarks/tuning_profiles/layout-attribution-verdict-v1.md)
§6.4 states. It is written before the pilot's first timed window exists, so the
rule cannot be shaped by the numbers it governs.

The pilot decides an **axis**. It decides no verdict, measures no candidate
revision, and moves nothing v1 predeclares: τ_cell stays 5 %, τ_set stays 2 %,
the pinned set keeps its thirty-four cells, the schema token stays
`selector-non-regression-v1`, and the attribution margin stays three standard
errors.

## 1. What the pilot answers

[`2026-08-20-post-cutover-receipt-3.md`](/dev/benchmarks/tuning_profiles/2026-08-20-post-cutover-receipt-3.md)
§"The two cells that fail coverage" records the ensemble's reference-arm
dispersion falling below the σ̂/2 floor at two cells:

| Cell | `s_R` at K = 128 | Floor σ̂/2 | Fraction reached |
|---|---:|---:|---:|
| `bit_backend/or_inplace/words=1` | 0.047448 | 0.059077 | 80.3 % |
| `bit_backend/xor_inplace/words=1` | 0.038850 | 0.049556 | 78.4 % |

v1 §6.6 sends that failure to a tracked amendment of the ensemble's axes. The
pilot measures whether the amended axis set of §3 clears both floors.

## 2. The mechanism this axis answers to

σ̂ is defined by the natural pair of v1 §8: the committed baseline build of
revision `0c072d73ca65cf50af98b8c4b61ed876f8218df6`, made in the main checkout,
and receipt 2's control rebuild of the same revision, made in
`.agents/worktrees/control-0c072d73`. The build-time study of §2 of the
pilot receipt establishes what separates those two binaries' placement: the
bench target embeds its `CARGO_MANIFEST_DIR` as a string literal in `.rodata`,
the two checkouts' manifest directories differ in length by 35 characters, and
the whole program image is therefore translated — every text symbol at a
different address, all by one constant, every address changed modulo 64 and
modulo 4096, and no symbol moved relative to any other.

v1 §3.1's three axes never translate the image. They pad inside it. That is the
degree of freedom the amended axis adds.

## 3. The amended axis set under test

Member `j` of the amended enumeration, `j` from 0 to 255, takes `A`, `B`, `C`
and `D` from v1 §6.6's K = 256 rung, unchanged:

    D(j) = ⌊j / 128⌋
    A(j) = ⌊(j mod 128) / 16⌋            B(j) = ⌊((j mod 128) mod 16) / 4⌋
    C(j) = ((j mod 4) + A(j) + B(j) + D(j)) mod 4

and adds one axis:

    E(j) = ⌊j / 2⌋

At `E = 0` the option is omitted. At `E ≥ 1` the member carries

    -C link-arg=-Wl,--build-id=0x<2·(20 + 32·E(j)) hexadecimal digits, all zero>

which lengthens the `.note.gnu.build-id` payload to `20 + 32·E` bytes and
translates the loaded image by 32·E bytes.

## 4. The three pilot groups

Every pilot build is of the reference revision
`0c072d73ca65cf50af98b8c4b61ed876f8218df6` in
`.agents/worktrees/control-0c072d73`, under `cargo +1.95.0`, the `simd`
feature, and the pinned set of thirty-four cells. Each build records **one
execution of one repetition** at `--target-ms 250`, as v1 §5.1 fixes. No
candidate revision is built or measured.

- **Group V1 — the control.** Thirty-two members of v1 §3.1's K = 128
  enumeration, `j = 4i + (i mod 2)` for `i` from 0 to 31:
  0, 5, 8, 13, 16, 21, 24, 29, 32, 37, 40, 45, 48, 53, 56, 61, 64, 69, 72, 77,
  80, 85, 88, 93, 96, 101, 104, 109, 112, 117, 120, 125. Sixteen even and
  sixteen odd.
- **Group SH — the new axis alone.** Thirty-two members at `A = B = C = D = 0`
  with `E = 4i + (i mod 4)` for `i` from 0 to 31: 0, 5, 10, 15, 16, 21, 26, 31,
  32, 37, 42, 47, 48, 53, 58, 63, 64, 69, 74, 79, 80, 85, 90, 95, 96, 101, 106,
  111, 112, 117, 122, 127. Every residue modulo 4 is represented, so the group
  spans the 32-, 64- and 128-byte offset classes.
- **Group V2 — the amended enumeration.** Sixty-four members of §3's K = 256
  enumeration, `j = 4i + (i mod 2)` for `i` from 0 to 63: the Group V1 list
  followed by 128, 133, 136, 141, 144, 149, 152, 157, 160, 165, 168, 173, 176,
  181, 184, 189, 192, 197, 200, 205, 208, 213, 216, 221, 224, 229, 232, 237,
  240, 245, 248, 253. Thirty-two even and thirty-two odd, thirty-two at `D = 0`
  and thirty-two at `D = 1`, sixty-four distinct levels of `E`.

Member 0 of Group V1, member 0 of Group V2 and `E = 0` of Group SH are the same
ordinary build under empty `RUSTFLAGS`. It is built once and run once in each
group.

## 5. Order

The 128 executions run in one lock session under
`dev/scripts/ccx1-bench-flock.sh` with `GF2_BENCH=1`, in thirty-two cycles of
four slots:

    V2(2i), V1(i), SH(i), V2(2i+1)      for i from 0 to 31

so the three groups carry mean slot positions 64.5, 64.0 and 65.0 out of 128 and
no warm-up or thermal trend over the session lands on one group.

## 6. The statistic

For a group and a cell, `s_R` is the sample standard deviation, over that
group's builds, of the natural logarithm of the build's pooled rate at that
cell — v1 §6.4's `s_R`, computed over the group instead of over an arm. The
pilot's estimator is validated against the committed evidence before the pilot
runs: applied to
[`2026-08-20-ensemble-reference-arm.csv`](/dev/benchmarks/tuning_profiles/2026-08-20-ensemble-reference-arm.csv)
it must reproduce the audit's `s_R` at every cell.

## 7. The decision rule

Let `floor(c)` be σ̂(c)/2 as receipt 3's audit prints it: 0.059077 at
`bit_backend/or_inplace/words=1` and 0.049556 at
`bit_backend/xor_inplace/words=1`. Neither number moves.

Let `s_V2(c)` be Group V2's statistic at cell `c` and

    L(c) = s_V2(c) · √(63 / χ²₀.₉₅,₆₃) = s_V2(c) · 0.8737

its one-sided 95 % lower confidence bound under a normal model of the logarithm,
with `n = 64`.

- **ADOPT** — `L(c) ≥ floor(c)` at both cells. The amended axis set is carried
  into the v2 amendment. In point-estimate terms this needs
  `s_V2 ≥ 0.067616` and `s_V2 ≥ 0.056719`.
- **INCONCLUSIVE** — `s_V2(c) ≥ floor(c)` at both cells but `L(c) < floor(c)` at
  one or both. The pilot escalates once, and only once, by extending Group V2
  with the sixty-four disjoint members `j = 4i + 3` for even `i` and `j = 4i + 2`
  for odd `i`, `i` from 0 to 63, and re-reading the same rule at `n = 128`,
  where the bound factor is `√(127 / χ²₀.₉₅,₁₂₇) = 0.9072`. Both stages are
  recorded.
- **REJECT** — `s_V2(c) < floor(c)` at either cell. The axis is not carried, the
  measurement is recorded with its numbers, and no v2 amendment is written on
  this axis.

**Calibration guard.** The pilot is read only if Group V1's statistic lies
within `[0.65, 1.55]` of receipt 3's committed `s_R` at both cells — 0.030841 to
0.073544 at `bit_backend/or_inplace/words=1` and 0.025253 to 0.060218 at
`bit_backend/xor_inplace/words=1`. A resampling of the committed 128-member arm
puts a 32-member subset's fifth and ninety-fifth percentiles at 0.85–1.14 and
0.76–1.26 of the full-arm value, so a group outside this band indicts the
session rather than the axis. If the guard fails the pilot establishes nothing
and is recorded as such.

Group SH enters no branch of the rule. It is recorded as the measurement of the
new axis's own dispersion.

## 8. What the pilot does not decide

- It measures no candidate revision and produces no verdict ratio. `50b47eae`
  REQ-01 is untouched by it.
- It does not re-measure the decorrelation precondition, which needs two arms.
- It does not license a fourth measured session. That session runs under the v2
  amendment after the owner of epic `6dc81018` approves it.
- Its groups are not ensembles under v1 §6.5: they carry 32 and 64 members, and
  the audit accepts 128 or 256. The pilot's statistic is computed outside the
  audit by the estimator §6 validates.
