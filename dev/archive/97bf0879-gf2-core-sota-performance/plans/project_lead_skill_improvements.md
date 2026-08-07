# project-lead skill improvements (deferred from 47698404 post-mortem)

## Origin

Epic `97bf0879` produced a post-mortem on excessive code-review cycle counts —
`47698404` closed at R11, with R5–R8 being a textbook "surgical-only fix
each round, reviewer surfaces new internal contradiction next round" pattern
and R9 being a textbook "argue the contract is non-blocking" mistake. The
full diagnosis is in this session's chat history; this document captures
the three remediation proposals the lead deferred (per user direction
2026-05-05) so they can be picked up as a separate skill-improvement issue.

The fourth remediation — *No-argue discipline* paragraph in `SKILL.md`
§ 7 — was landed inline on 2026-05-05 because the user requested it
specifically. The three deferred proposals follow.

## 1. Pre-dispatch sweep checklist (procedural enforcement of Tier 2.5/2.75/3)

**Problem.** SKILL.md § 7 already says "Before dispatching any rework, the
lead must have completed Tiers 1.5 and 2.75". The rule is *stated* but not
*procedural* — there is no checklist or gate that forces the lead to
actually run the sweep before invoking `jit gate pass <id> code-review`.
In practice, the lead reads the rule, internalizes "be holistic," then
makes a surgical edit and dispatches. R5–R8 in `47698404` was four
rounds of this pattern.

**Proposal.** Add `references/pre-dispatch-sweep.md` with a per-tier
executable checklist. Replace the prose tier description in § 7 with a
table that names each tier and links to a per-tier executable section
of the checklist file. The checklist is the artifact the lead produces
and stores in the worktree before invoking `jit gate pass`; review-rounds
without a fresh checklist file are skill-disallowed.

Per-tier checks the file enumerates as command-line operations:

- **Tier 1.5 prior-findings regression:** `grep -nF "<each prior-finding
  citation>"` over the affected files; the lead pastes the result into
  the checklist and confirms each prior finding is closed at HEAD.
- **Tier 2.5 stale-narrative sweep:** `grep -niE 'PENDING|TODO|until .*
  lands|deferred|future work|to be wired|not yet'` over the affected
  docs; for each match, decide and record: kept-as-explicit-non-scope
  OR removed.
- **Tier 2.75 deferred-items audit:** for every linked design/protocol
  doc, `grep -niE 'shall|must|required|mandatory|hard'`; verify each
  constraint is satisfied OR explicitly waived in the issue's Non-goals.
- **Tier 3 cross-section coherence:** for every concept named more than
  once in the artifact (canonical reference, marker class, status string,
  line citation), the checklist enumerates each occurrence and confirms
  agreement.

**Anti-cheating.** The checklist must include *concrete grep output*, not
"I checked." The R5–R8 cycle pattern in `47698404` failed because the
lead said "I'll be holistic" and then wasn't.

**Estimated impact.** Closes the surgical-edit-drift failure mode
(R5–R8). Net cycles saved on a typical multi-round review odyssey: 2–4.

## 2. Wave-time governing-document audit (Section 4 addition)

**Problem.** When a design doc endorses a new architectural pattern that
the upstream protocol/governance document doesn't yet permit, the
implementation-issue's code-review becomes the discovery mechanism for
the governance mismatch. R10 in `47698404` is a textbook example:
`sparse_benchmark_corpus.md` § 4 endorsed the shared-smoke-harness
pattern in Wave 0 (`a3412e15`); `sota_reference_acceptance_protocol.md`
§ 6 still said "each candidate's harness must include `--smoke`" until
R10 forced Amendment 3. Wave 0 should have shipped Amendment 3 alongside
the design doc.

**Proposal.** Add Step 3b to SKILL.md § 4 wave planning, after Step 3
"Resolve open design questions":

> **Step 3b — Governing-doc audit.** Before dispatching any wave whose
> implementation will land a new pattern (a new harness shape, a new
> evidence-file location, a new oracle architecture, etc.), audit every
> governance document that could constrain the pattern: acceptance
> protocols, project-level CLAUDE.md, convention docs, prior-issue
> success-criteria templates. For each governance paragraph that the
> new pattern violates literally, draft an amendment now and land it
> as a sibling commit in the same wave. Do not let the implementation
> issue's code-review be the discovery mechanism for governance
> mismatch.

**How to discover the new pattern.** The breakdown analysis already
enumerates implementation issues; the audit hooks into the same
analysis output by adding "list every governance document this issue
references in its design doc" as a breakdown sub-task.

**Estimated impact.** Closes the protocol-lag failure mode (R10 in
`47698404`). Saves 1 cycle per "new architectural pattern" wave.

## 3. Cycle-count-triggered mode switch (Section 8 addition)

**Problem.** Surgical-only mode is correct for the first rework but
becomes counterproductive once the lead has missed two consecutive
holistic-coherence sweeps. Without an explicit signal, the lead stays
in surgical mode through round 5–8.

**Proposal.** When `rework_counts[issue_id] >= 2`, the rework dispatcher
switches from surgical mode to holistic mode:

- **Surgical mode (rework 1):** "the reviewer cited finding X; fix X."
- **Holistic mode (rework ≥ 2):** "the reviewer cited finding X; rewrite
  every section in the affected artifact that names the same concept as
  X, and verify all named constraint documents."

**Implementation.** Local change to `references/rework-prompt-template.md`:
at rework count ≥ 2, prepend a section requiring the worker (or the lead
doing direct edits) to perform a full Tier 2.5/2.75/3 sweep before any
edit, not just on the cited section.

**Estimated impact.** Reinforces (1) at the rework-prompt layer. Net
cycles saved on a multi-round odyssey where (1) was followed loosely:
1–2.

## Memory-vs-skill migration

Two memories saved during this session describe project-agnostic lessons
and should ideally migrate from user-memory (`~/.claude/projects/.../memory/`)
into the skill so they become portable across users + projects:

- `feedback_holistic_pre_dispatch_sweep.md` → addressed by proposal #1.
- `feedback_amend_protocol_with_design.md` → addressed by proposal #2.

Once #1 and #2 are landed in the skill, these two memory files should be
removed from user-memory to avoid duplicate-source-of-truth.
`feedback_pre_dispatch_criterion_audit.md` and `feedback_dispatch_prompt_facts.md`
already in user-memory are similarly project-agnostic and would be
candidates for skill migration in the same pass.

## Owner

Not assigned. Suggested home: a new `meta:project-lead-improvements` epic
or attach as a stand-alone task linked to the user's `.claude/skills/`
maintenance backlog.

## Status

Open as of 2026-05-05. The "No-argue discipline" paragraph was landed
inline in `SKILL.md` § 7 (between "Before dispatching any rework" and
"Section 8: Rework") as a one-line skill change at the user's
direction; the three proposals above are the deferred items.
