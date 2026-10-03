# Diagnosis brainstorm — mul_fast dispatch-cell excursion (issue eb9b324c)

**Date:** 2026-08-22 · **Participants:** epic 6dc81018 execution lead (Claude)
and gpt-5.6-luna (Codex), over the forum-poc mailbox bus, both grounding
claims in the committed session-5 record and read-only inspection of the two
revisions (`0c072d73` reference, `1a5812c2` candidate) and their committed
ordinary member binaries. No build, bench or test was run; all numbers below
are from the committed arm CSVs, ledgers and static disassembly.

## Established (data + static analysis)

**The excursion is 2-periodic in the translation index E — a VA-bit-5
(64-byte fetch-block phase) effect.** A 32-byte-multiple translation
preserves alignment mod 32 and all pairwise code distances; E parity flips
bit 5. Per-member analysis of the candidate arm: len=32 and len=64 rates
correlate 0.941; lag-1 autocorrelation in E is −0.71/−0.72, lag-2
+0.76/+0.78. Candidate parity means (E even, bit5=1 | E odd, bit5=0; the
candidate base has P mod 64 = 32, the reference P mod 64 = 0, so E parity
maps to opposite absolute phases in the two arms):

| Cell | cand even | cand odd | odd/even | ref odd/even |
|---|---:|---:|---:|---:|
| mul_fast/len=32 | 1217.9 | 1303.2 | 1.0700 | 1.0003 |
| mul_fast/len=64 | 3859.3 | 4143.6 | 1.0737 | 1.0052 |
| mul/len=32 (schoolbook) | 1202.3 | 1219.2 | 1.0141 | 0.9802 |
| mul/len=33 (karatsuba) | 1133.2 | 1205.3 | 1.0636 | 0.9978 |
| mul/len=64 (karatsuba) | 3917.5 | 4157.5 | 1.0613 | 0.9812 |
| mul/len=256 (karatsuba) | 37371.6 | 39605.0 | 1.0598 | 1.0018 |
| mul_fast/len=128 (ntt) | — | — | 0.9889 | 0.9879 |
| xor_inplace/words=8 | — | — | 1.0352 | 1.0018 |
| not_inplace/words=1 | 2.409 | 2.832 | 1.1756 | 0.8492 |

Matched-absolute-phase ratios at mul_fast/32 (cand vs ref): **1.024 at
bit5=1** (the ordinary build's phase — why every single-build draw reads
~1.00) and **1.096 at bit5=0**. The verdict's 1.058 is the phase average,
which is v4's estimand.

**Pre-existing, not the cutover:** `bit_backend/not_inplace/words=1` is
~17 % bit5-sensitive in BOTH revisions with the same fast phase (bit5=1);
the v4 Thue–Morse half-balance cancels it to a pooled 1.0006. Receipt-4's
+10 % there was the unbalanced-construction artifact fc976a80 diagnosed.

**Two distinct cutover-introduced sites:**

- **Site A — fixed per call, isolated by schoolbook len=32.** The candidate
  splits the old inlined dispatch: `mul_fast`'s sub-NTT arm loads/tests
  `tuning::ACTIVE`, compares the dynamic `karatsuba_max_out_len`, epilogues
  and tail-jumps (candidate ordinary binary: 0x5fda0) into a separate
  `mul_impl` (0x5fe30) that repeats the empty checks, loads ACTIVE again and
  compares `karatsuba_min_degree` before tail-jumping to
  `mul_schoolbook_impl`. The reference `mul_fast` (0x512e0) compares against
  immediates 130/33 and calls `mul_schoolbook_impl` directly. No indirect
  call exists on either revision's hot path (H-indirect-BTB rejected).
  Penalty at mul_fast/32: ~85 ns/call at bit5=0, +2.4 % even at bit5=1. The
  double-dispatch structure exists for every sub-NTT call, but the committed
  len=64 numbers do not independently expose a positive additive wrapper
  term (karatsuba/layout effects dominate there): the fixed penalty is
  *isolated* only by schoolbook len=32.
- **Site B — multiplicative ~6 % across a 30× work range (re-entered per
  recursion node).** Karatsuba routes ONCE on both revisions —
  candidate `mul_impl` resolves `karatsuba_min_degree` then calls
  `mul_karatsuba_raw(lhs, rhs, threshold)`, and the recursion is raw-to-raw
  (no per-node profile lookup; "hoist the route" is already the design).
  What changed is codegen/layout of the raw recursion: the threshold became
  a register argument (reference compares immediate 0x21), the function grew
  0x911 → 0x941 bytes, frame 0xd8 → 0xf8. Statically, 64B windows holding
  ≥3 conditional branches over `mul_karatsuba_raw`: candidate 5 (fast
  parity) vs 9 (slow parity), reference 5 vs 6; the extra slow-parity
  windows co-locate hot loop exits, capacity checks and backedges (shifted
  0x5b900/0x5b980, 0x5bc40/0x5bd40). Phrase as: *parameterized raw codegen/
  layout is the only localized cutover change consistent with the recursive
  parity term* — the causal window is not yet proven.

**Rejected en route:** indirect-call/BTB-target hypothesis (no indirect call
exists); 4K-page/iTLB periodicity (response is 2-periodic in E, not
page-periodic); simple ">2 branches per 64B window" as a unifying mechanism
(at the mul_fast/mul_impl entry guards it predicts the WRONG phase: the fast
parity is the one that packs all four guards into one window; it survives
only as the Site-B candidate).

## Remaining discriminating experiment (E2)

perf stat on staged candidate members of opposite parity (j=1/E=0 vs
j=3/E=1, `/tmp/gf2-ens5/cand/`) and reference controls: `ex_ret_brn_misp`,
op-cache miss, IC fetch stall, cycles. **Protocol gap, recorded so the
worker does not overclaim:** `selector_non_regression` has no per-cell
filter, so whole-binary counters aggregate all 34 cells and
mul_fast/mul_karatsuba_raw symbols are shared across several cells.
Cell-isolating evidence needs either an owner-approved harness flag rebuild
(which changes layout and must recreate the parity pair) or a sampling
scheme keyed to per-cell execution (perf record gives symbol-level samples
but still mixes sizes). Whole-suite counter ratios may support, but cannot
site-isolate, the mechanism.

## Fix candidates (for REQ-02, in order)

1. **Collapse the double dispatch, preserving the canonical abstraction:**
   one internal dispatcher accepting an already-resolved selectors/threshold
   pair; `FieldPoly::mul` resolves once and calls it; `mul_fast` resolves
   once, performs the NTT gate, then calls the same inner dispatcher. Do NOT
   make `mul_fast` a second raw-routing implementation. Kills Site A by
   construction (mean and sensitivity).
2. **Site B:** remeasure after (1) first; if the parameterized recursion
   remains the sensitive body, consider an immediate-threshold
   specialization — which must preserve installed-profile semantics (not a
   drop-in).
3. Verification shape: pilot translation ensemble with both parities
   represented, under the standing lock discipline, before any owner
   decision on a next measured session (REQ-03: no measured pinned-set
   session without owner approval).

## Transcript

The full message exchange is preserved in the forum mailboxes
(`/tmp/jit-forum/mailbox/agent_{claude,codex}/cur/`, 2026-08-22T14:16–14:30Z)
and summarized faithfully above; the mailbox is a PoC transport and not
durable, which is why this record exists.
