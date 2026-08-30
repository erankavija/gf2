# Research Review

Perform an issue-scoped, read-only review of scientific rigor: claims, statistics, citations, reproducibility.

## Read-only boundary

Do not edit files, mutate issues, pass gates, or request wider permissions. Use only read-only inspection commands.

## Attribution

Read the context issue, its hard criteria, `cites:` labels, linked documents, and latest prior structured findings. Build the attributable footprint from commits whose messages contain `jit:<short-id>`. If no tagged commit exists, review the issue's linked documents only; do not expand into a repository-wide audit. Read every applicable `AGENTS.md` from the root to each affected path. Resolve citation keys with `jit item show @/citation/<key>`.

## User decisions

The issue description may record user decisions under a `## Decisions` heading (items `DEC-NN`). Decisions bind this review: treat the state a decision accepts as authoritative, and do not raise a blocking finding whose only remedy the decision forecloses. If evidence contradicts a decision's factual premise, surface that as an advisory finding citing the decision identifier.

## Convergence across rounds

Verify every prior blocking finding's state at HEAD before any new judgment and open the report with a one-line-per-finding closure ledger: closed (cite the closing artifact), open, or regressed; a regression outranks any new finding. After the first round, judge what changed since the reviewed revision plus prior open findings; a new blocking finding against content unchanged since a prior round names the round that could have observed it and why it is material now, else report it as advisory. A `## Decisions` item forecloses its remedy class across the whole attributable footprint — the same foreclosed-evidence class on a sibling surface of the same dataset is re-raised only as a factual-premise advisory citing the decision.

## Rubric — blocking on failure

- Trace every quantitative claim (speedup, BLER, threshold, probability estimate, crossover, sample statistic) in attributable text to a committed artifact. Reject prose-only numbers. A number the text marks as a projection, budget, or worked example, and from which no reported result derives, traces to its derivation rather than to a measurement artifact.
- Require every stochastic or performance result to record the provenance `@/inv/claims-trace-to-artifacts` requires, together with the RNG implementation and version and the invocation. Reject partial manifests.
- Require sample counts and confidence intervals for every Monte Carlo estimate. Require error bars or interval columns in result tables and plots. Reject bare point estimates.
- Require unmeasured numbers to be labeled as estimates. Reject estimates presented as measurements.
- Require every comparison to name its baseline with version and provenance, and state the hardware. Require reproduction targets to quote the source's numbers under a registry citekey.
- Require external claims to carry a citekey resolving in the registry. Verify the cited work actually supports the claim wherever the context allows; flag mismatches.
- Require contradicting data to be stated: if results falsify a criterion, hypothesis, or cited claim, the text must say so. Reject silent rework or omission.

A blocking finding names the published claim it puts in doubt. A rubric item is satisfied when the artifact lets a reader check that claim at its authoritative location; the same evidence need not be restated wherever the claim is mentioned. A gap that cannot change whether a claim is supported is advisory.

## Rubric — advisory

- Declare stopping rules and sampling plans before results, not after.
- Version datasets (layout, checksums) rather than overwriting.
- Cross-check determinism where a seeded rerun is cheap.
- Prefer one authoritative results artifact over numbers scattered across documents.

## Verdict policy

Any blocking finding on attributable content: FAIL. Advisory-only findings: PASS with the findings listed. Pre-existing debt outside the attributable footprint is advisory. Do not fail for pending judgment gates. Verify every hard criterion that concerns measurement, statistics, or citation; consume executable-gate evidence from the context rather than re-running it. Every finding classifies `disposition` (blocking or advisory) and `origin` (issue-impact or pre-existing); the verdict is fail if and only if an unresolved issue-impact blocking finding exists.
