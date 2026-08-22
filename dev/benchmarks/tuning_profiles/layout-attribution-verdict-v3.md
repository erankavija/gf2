# Layout-attribution amendment v3: the realized candidate-arm lattice

This document amends how v2 §A4's realized-construction verification reads the
candidate arm of the next measured post-cutover session of epic `6dc81018`, and
nothing else. It is filed by the epic's execution lead under owner decision
DEC-J of 2026-08-22, and it is committed after that session's build phase and
before any of its timed windows, so it is shaped by build-time facts alone —
the same standing v2 claims for its own axis G, which pilot 2's build-time
facts fixed — and by no number any timed execution produced.

[`layout-attribution-verdict-v1.md`](layout-attribution-verdict-v1.md) and
[`layout-attribution-verdict-v2.md`](layout-attribution-verdict-v2.md) stand as
written and stay authoritative. **This document changes none of their text.**
Everything they fix that this document does not name is what governs. τ_cell
stays 5 %. τ_set stays 2 %. The pinned set keeps all thirty-four cells. The
schema token `selector-non-regression-v1` is not bumped. K stays 256, and v2
§A3's axes, formulas and `RUSTFLAGS` stay exactly as written. No predeclared
value moves.

## B1. What sends this document here

The session-4 build phase of 2026-08-21 built both arms' complete 256-member
ensembles under v2 §A3 exactly as written: the reference arm at revision
`0c072d73` in `.agents/worktrees/control-0c072d73`, the candidate arm at
revision `ec857d2f` in the main checkout. The pre-timed verification v2 §A4
requires then read, from the build ledgers:

- **Reference arm: PASS on every check.** 256 distinct binaries, 256 distinct
  `.text` page offsets, the affine law with base(0) = 0x1cbc0 and
  base(1) = 0x1f5d0, four realized cache-line offsets {0, 16, 32, 48} at 32
  members per half each, equal L1i set-index multisets covering all 64 sets
  twice per half, and full axis balance.
- **Candidate arm: FAIL on two checks, PASS on the rest.** The realized bases
  are 0x1e8a0 at G = 0 and 0x221e0 at G = 1 (127 of 128 members), whose
  separation is ≡ 0 (mod 32). The two levels of G therefore realize the *same*
  set of 128 page offsets — every one a multiple of 32 — so the arm shows 128
  distinct `.text` page offsets rather than 256, and two realized cache-line
  offsets {0, 32} rather than four. One member, `j = 227` (E = 113, G = 1),
  sits exactly one page (4,096 bytes) below its group's plane, so the strict
  affine check also fails. Every other check passed: 256 distinct binaries,
  halves holding 64 members at each realized cache-line offset, equal L1i
  set-index multisets covering all 64 sets twice per half, and full axis
  balance. The session aborted before taking the lock; no timed execution of
  session 4 exists.

v2 §A4's four-offset lattice was a realized fact of the *reference* revision:
its dead-code retention happens to separate the two G bases by ≡ 16 (mod 32).
Nothing in v2 pins that residue, and on the candidate revision it comes out
≡ 0 (mod 32). v1 §6.6 fixes the consequence class in advance — the ensemble's
axes are the subject of a tracked amendment before the next measured run — and
this is that amendment.

## B2. Why a 16-byte separation is unreachable on the candidate revision

Two probe builds of the candidate revision, made after the verification failure
and before any timed window, establish that no translation can restore the
four-offset lattice:

- `-C link-dead-code -C link-arg=-Wl,--build-id=0x<72 zeros>` — the G = 1,
  E = 0 member with its build-id payload lengthened by exactly 16 bytes —
  places `.text` at 0x221e0, byte-identical placement to the member without
  the padding. The 16-byte translation is rounded away.
- `-C link-dead-code -C link-arg=-Wl,--sort-section=name` places `.text` at
  0x221e0 as well.

The mechanism is in the section table: `.rodata` carries `Align = 32` in the
candidate binary, and it sits between the `.note.gnu.build-id` note (the E
knob) and `.text`. Any upstream padding is rounded to a multiple of 32 at
`.rodata`, so the reachable translations of `.text` are exactly the multiples
of 32 — which is why axis E's 32-byte steps land exactly while a 16-byte step
lands nowhere. The content between `.rodata` and `.text` (`.gcc_except_table`,
`.eh_frame_hdr`, `.eh_frame`) is fixed by the revision's own code, so the base
residue mod 32 at each level of G is a fact of the revision, not a knob. The
candidate revision's build inputs are the thing under test and cannot be
edited to move it.

## B3. The realized-lattice reading of §A4

For the candidate arm of the next measured session, v2 §A4's verification is
read as follows. Each numbered item replaces or restates exactly the check it
names; every check not named reads as v2 wrote it.

1. **The translation law is read on page offsets.** For one constant `P(G)`
   per level of G, every member satisfies
   `page_offset(j) = (P(G) + 32·E(j)) mod 4096`. A whole-page displacement of
   the image satisfies this law; v2 §A3 already fixes that a whole-page shift
   changes no cache-line offset and no L1i set index. The session-4 candidate
   arm realizes exactly one such displacement — member `j = 227` at E = 113,
   one page below its group's plane, landing at page offset
   (480 + 32·113) mod 4096 = 0, exactly where the law puts it. The
   verification records every whole-page displacement it finds, by member.

2. **Distinct placements are (page offset, G) pairs.** The verification
   requires 256 distinct binaries and, within each level of G, 128 distinct
   page offsets — 256 distinct (page offset, G) placements. When
   `P(0) ≡ P(1) (mod 32)`, as the candidate arm realizes, the two levels share
   their 128 page offsets, and each shared offset holds exactly two members:
   level E of G = 0 and level E + 54 (mod 128) of G = 1. Such a pair is two
   distinct placements, not one placement held twice: G = 1 retains dead code,
   which moves live code relative to live code — v2 §A3's second way two
   natural builds differ — and the pair's binaries differ by ~12.6 % in size
   (member 0: 793,248 bytes; member 1: 893,056 bytes). The duplicate v1 §6.6
   rejects is the opposite case: `--sort-section=name` produced "a different
   binary of identical size that moves no symbol at all", repeating one
   placement exactly. No member of the amended reading repeats another's
   placement.

3. **The half-matching checks read as v2 wrote them, over realized offsets.**
   Each half of the member-index parity split holds equal counts at each
   realized cache-line offset (64 per half at each of {0, 32}, as the ledger
   shows), the two multisets of `(address >> 6) mod 64` are equal, each half
   covers all 64 L1i sets exactly twice, and the axis balance — one member per
   half at each of the 128 levels of E, 64 per half at each level of G — holds
   unchanged. The session-4 candidate arm passed all of these as written.

4. **What the narrowed lattice can and cannot hide is decided by the session's
   own audit.** A two-offset lattice samples cache-line offsets {0, 32} and
   not {16, 48}; whether that under-samples the across-build dispersion the
   record measures is exactly what v1 §6.4's coverage precondition tests,
   cell by cell against `σ̂/2` and on the RMS, from the arm's own measured
   `s_R`. v1 §6.4, §6.5 and v2 §A7's routing stand untouched: a precision or
   half-split-null failure at K = 256 goes to the owner, and a coverage or
   decorrelation failure goes to a further tracked amendment of the axes. This
   document asserts nothing about which way that audit reads.

The reference arm's verification is not amended: it passed v2 §A4 as written,
and under this document's reading it passes identically (its per-G page-offset
lattices are disjoint, so its 256 placements are also 256 distinct page
offsets).

## B4. What this amendment does not touch

- **v1 in full and v2 in full.** Every section of both stands as written. What
  is read through this document is v2 §A4's realized-construction checklist,
  for the candidate arm only.
- **The session-4 build phase.** All 512 built binaries stand as built, with
  their ledgers; no member is rebuilt, dropped or replaced under this
  document. The build-id payloads, flags and enumeration formulas are v2 §A3's
  exactly.
- **Every predeclared value.** τ_cell, τ_set, the pinned set, the schema
  token, the three-standard-error margin, K = 256, `--target-ms 250`, v1
  §5.2's ordering, the pinned host, the wrapper, the affinity, the governor
  and the toolchain.
- **All standing receipts and plans.** None is modified, superseded, re-run or
  adjusted here. Receipt 3 stands as taken; the pilot receipt stands as taken.
- **v1 §7's declared readings and v2 §A7's routing.** All govern the next
  session exactly as written.

## B5. Standing of this document

This rule is written after session 4's build phase and its construction
verification, and before any timed window: the session driver aborted at
2026-08-21T22:04:05Z on the candidate arm's verification, before taking the
benchmark lock, and no timed execution of session 4 exists. The amendment is
therefore shaped by build-time facts — ledgers, section tables and two probe
builds — and by no measurement it governs, the same standing v2 §A6 records
for pilot 2 fixing axis G.

The session's verification script (`verify-construction.py`) is amended to
read §A4 through this document; both its prior text and its amended text are
copied into the session record with their SHA-256, beside the ledgers and the
driver log that record the v2-as-written FAIL this document answers.

Neither v1 nor v2 gains a pointer to this document, for v2 §A10's reason: it
changes none of their text. The read path receipt-4 → this document → v2 → v1
suffices.

The owner of epic `6dc81018` approves this text before the session's timed
phase is dispatched. Until that approval is recorded, no timed window runs
under it.
