# Handoff — Harden and generalize BCH codes over finite fields (ae03bcd0) — session 2, final

**Date:** 2026-09-01T10:35+03:00
**Session number:** 2 (second handoff; supersedes handoff-2.md's "what to do next")
**Prior handoffs:** handoff.md, handoff-2.md — their Traps sections remain in force in full.

## Current state

- **Waves 1, 2, and 3 are CLOSED.** Eleven issues done this session: 4e732b56 (survey, after 3 rework rounds + lead-direct measurement/protocol closures), 7a3a6738 (bch-api-design), c3d5cea5 (extension trait), ebdbd228 (error surface), 19fe9394 (irreducibility), fc2fa0b6 (exact order), 0c4d84cf (block-code traits + binary-code-v1 boundary), 997f0ab9 (follow-ups 1642af1c/1a8f6acd/b4d7a25d created and wired), plus wave 1's ad6778e8/3243bc1f/8f51d6cf. Every done issue passed all its configured gates; all merges carried merged-tree cargo-ci.
- `current_wave` = 4 in progress.json. **Not dispatched** (owner directed session end at wave close). Wave 4: 33812b86 coordinate-provenance, 6aac5c44 minpoly-subfield, 992c6c6e type-erased-handles (easy → Luna xhigh), 769c3144 quotient-ext-runtime (hard → Sol xhigh per alternation; last hard native was Opus).
- **New project invariant (owner-accepted): `caller-trusted-fast-paths`** — in .jit/invariants.toml, projected into AGENTS.md, first enforced by c3d5cea5 (labeled `enforces:@/inv/caller-trusted-fast-paths`, cited from extension.rs rustdoc at the module walkthrough and both `from_certificate_unchecked` contracts). Born from the c3d5cea5 escalation: three review rounds ratcheted certificate reuse toward security hardening; the owner ruled "HPC, not security" (R-17), the reuse path became the `_unchecked` idiom, the policing machinery was deleted, and the reviewer then cited the invariant approvingly.
- Worktrees: wave-2/3 worktrees reclaimed (agent-0c4d84cf and agent-7a3a6738 possibly still finishing harvest in a background reclaim — verify `git worktree list` on resume and reclaim any stragglers). **agent-4e732b56 deliberately KEPT** (built external baselines for wave-11+ perf work). Peer worktrees (agent-eaae1b56*, agent-02b8137c*, others) are not ours to touch.
- Peer (agent:codex, epic 6dc81018): eaae1b56 in_progress in isolated worktrees; coordination protocol working smoothly — lock + announce, split windows (their merges vs my gate runs), no courtesy holds per owner policy. My 389aa4de facilitation (I executed their calibration measurement; two protocol defects found, corrected with approval, disclosed) is fully closed — their issue is done.

## What to do next (wave 4 onward)

- [ ] Dispatch wave 4 per progress.json. Bake into prompts: extension-design module-map fences; R-04 (conway-registry tower rule, for wave 5); R-10 (axiom harness now enforces every law — a previously-"passing" carrier may legitimately fail); the `caller-trusted-fast-paths` invariant for any API with validation-skipping paths; R-08 lib.rs/bch-mod.rs serialization chain for gf2-coding writers.
- [ ] 769c3144 + 0e706bf5 (quotient extensions): the R-01 residual is in surfaced_pitfalls — an out-of-tree ExtConfig with a wrong non-residue builds a non-field ConstExt; epic REQ-02's compile-time reconciliation must address or explicitly scope it (owner approval for scoping).
- [ ] Standing: 88ca7d2f (pin baseline receipt) before cutover-benches (wave 12) merges; survey carry-forwards (genmatrix-perf replaces the basis-vector algorithm per survey §8.1 — note the survey's final numbers made M4RI's repository-order fresh-alloc route the measured target; avx2-batch-kernels checks generated code first; perf-receipts re-establishes absolutes on the kept governor).
- [ ] b438e7de (wide_config.rs:22 comment, priority high): now load-bearing — BinaryPrimeExt::new depends on the explicit leading term the comment misdescribes. Cheap fix; fold into any gf2-core wave or do lead-direct with code-review.
- [ ] Pre-existing repo divergences not ours: 049a89af + ef18c60b membership labels (advisory), and 049a89af was wired to archived epic ae82bd73 (safe direction) to satisfy repository-integrity.

## Traps — new this session-half (handoff.md + handoff-2.md traps all still in force)

- **Review ratchets need an owner ruling, not a fourth round.** Three rounds escalated the same root cause (certificate trust) one level each; each literal fix moved the "attack" up. When a review direction implies hardening a non-adversarial API, escalate the FRAME to the owner instead of fixing level N+1. The fix that ended it was a vocabulary change (the `_unchecked` idiom) plus an invariant the reviewer could cite — reviewers follow codified authority.
- **Sweep every instance of a doc claim the reviewer cites once.** The `O(1)` identity-match claim existed in three places; the reviewer cited one. Same lesson as the survey's nice-posture sweep.
- **`cmd | tail -N` masks exit codes** — it made failed claims/updates/gate-evaluates look green repeatedly (997f0ab9's failed done-transition, the swallowed claim errors). Never pipe a jit mutation before checking `$?`; prefer `if cmd; then` and print output separately.
- **Done-transitions respect the DAG**: an issue with all gates passed still refuses `done` while a dependency is in_progress (fc2fa0b6 waited on c3d5cea5). Order transitions by dependency.
- **`jit project render`** regenerates AGENTS.md's invariants block after editing .jit/invariants.toml; `jit validate` flags the stale projection until run.
- **repo-validate's repository-integrity check blocks on ANY isolated issue repo-wide**, including other leads'. Wire your own creations at creation time (follow-ups → depend on the epic; standalone bugs → depend on the issue that made them relevant); for foreign isolated issues, ask the peer, and prefer the safe edge direction (live issue depends on archived anchor, never a child added to a closed container).
- **The external process-killer was never identified** (killed forum listeners ×3 and one CI task; peer denies pkill). The self-healing Monitor-loop listener (`while true; do forum.sh recv ...; done`, persistent) survives it; use that pattern from the start.

## Open questions needing invoker input

None. Wave 4 dispatch is the next action and is fully specified.

## Reference artefacts

- progress.json (current: waves 1-3 done, wave 4 next); plan.md; breakdown.json; bch-api-design.md; extension-design.md; investigation.md.
- Landed API surfaces for wave-4+ workers: `gf2-core/src/field/{extension.rs,irreducibility.rs}` (FieldExtension, certificates, prove_irreducible, canonical_generator, element_of_exact_order), `gf2-coding/src/{error.rs,bch/error.rs}` (CodeError/BchError), `gf2-coding/src/traits.rs` (traits::block + traits::compat::binary_v1), `gf2-coding/src/linear.rs` (allocation-free canonical encoder).
- Survey deliverables + kept baselines: dev/active/4e732b56/, dev/bench_results/4e732b56/, worktree agent-4e732b56.
- Peer coordination: forum bus ~/Projects/forum-poc/forum.sh (FORUM_DIR=/tmp/jit-forum), lock /tmp/gf2-main-leads.lock, owner minimal-serialization policy in progress.json coordination notes.
