# doc-review of 495807a3 at 9f19a30f9 (known documentation defect)

Prompt: `contrib/gates/doc-review-prompt.md` at c24d80ec7. Wrapper: `contrib/gates/ai-review.sh` with the gate's configured `REVIEWER_AGENT`, run in a checkout of 9f19a30f9. Wrapper result: FAILED.

Attribution: tagged commits `1ba785af`, `4133f415`, `a446d741`, `b1172fa3`, `84584b92`, `6bf63409`; current checkout contains the first three  
Policy sources: `AGENTS.md`; `.jit/reference/content-standards.md`; `.jit/config.toml`  
Resolved items: `@/invariant/{single-source-prose,present-tense-prose,non-obvious-comments,current-state-scope,teaching-rustdoc-examples,active-document-layout,agents-md-scope}` from registry-first `.jit/invariants.toml`  
Invariants applied: `@/inv/single-source-prose`, `@/inv/present-tense-prose`, `@/inv/non-obvious-comments`, `@/inv/current-state-scope`, `@/inv/teaching-rustdoc-examples`, `@/inv/active-document-layout`, `@/inv/agents-md-scope`  
Gate evidence: `repo-validate` passed; `code-review` pending; no prior structured findings in the supplied run history  
Documentation impact: `.jit/invariants.toml` and the `AGENTS.md` projection are correct; the linked active draft remains stale and needs current-state wording  
Accuracy: fail [F1]  
Canonical placement: pass []  
Research relevance: pass []  
Aggregate concision: fail [F2]  
Current-state prose: fail [F1, F2, F3]  
Evidence: pass []  
Example value: pass []  

1. **F1 — blocking, issue-impact, high:** [495807a3-invariant-draft.md:3](dev/active/fa787f85-documentation-overhaul/495807a3-invariant-draft.md:3) says the texts are awaiting approval under nonexistent `DEC-03` and that nothing is registered, while the current registry and `AGENTS.md` projection contain all five approved invariants and the issue records approval in DEC-01. The later “Proposed invariants” framing has the same stale state. This violates `@/inv/current-state-scope` and `@/inv/single-source-prose`.

2. **F2 — blocking, issue-impact, medium:** [495807a3-invariant-draft.md:63](dev/active/fa787f85-documentation-overhaul/495807a3-invariant-draft.md:63) retains a future enforcement handoff to a nonexistent `docs-mechanical` item, and [line 103](dev/active/fa787f85-documentation-overhaul/495807a3-invariant-draft.md:103) retains a counterfactual in which the removed subtree-scope prose remains. These are speculative, unnecessary branches contrary to DEC-02 and `@/inv/current-state-scope`.

3. **F3 — blocking, issue-impact, medium:** [495807a3-invariant-draft.md:86](dev/active/fa787f85-documentation-overhaul/495807a3-invariant-draft.md:86) presents historical “superseded” line locations and a 199-line snapshot rather than the current 193-line `AGENTS.md`. Replace the change-history framing with the current registered/projection state, as required by `@/inv/current-state-scope` and `@/inv/present-tense-prose`.

Total findings: 3

<<<JIT-FINDINGS-JSON
{"verdict":"fail","summary":"The linked invariant draft is stale relative to the registered and projected current state.","findings":[{"id":"F1","severity":"high","summary":"The linked draft incorrectly says approval is pending under DEC-03 and that no invariant is registered.","file":"dev/active/fa787f85-documentation-overhaul/495807a3-invariant-draft.md","line":3,"disposition":"blocking","origin":"issue-impact","references":["@/inv/current-state-scope","@/inv/single-source-prose"]},{"id":"F2","severity":"medium","summary":"The draft retains speculative handoff and counterfactual scope-prose branches contrary to the approved current state.","file":"dev/active/fa787f85-documentation-overhaul/495807a3-invariant-draft.md","line":63,"disposition":"blocking","origin":"issue-impact","references":["@/inv/current-state-scope"]},{"id":"F3","severity":"medium","summary":"The draft retains historical AGENTS.md line references and a 199-line snapshot instead of current-state documentation.","file":"dev/active/fa787f85-documentation-overhaul/495807a3-invariant-draft.md","line":86,"disposition":"blocking","origin":"issue-impact","references":["@/inv/current-state-scope","@/inv/present-tense-prose"]}]}
JIT-FINDINGS-JSON>>>
VERDICT: FAIL
