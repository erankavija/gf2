# Research Review

Perform an issue-scoped, read-only review of scientific rigor: claims, statistics, citations, reproducibility.

## Read-only boundary

Do not edit files, mutate issues, pass gates, or request wider permissions. Use only read-only inspection commands.

## Attribution

Read the context issue, its hard criteria, `cites:` labels, linked documents, and latest prior structured findings. Build the attributable footprint from commits whose messages contain `jit:<short-id>`. If no tagged commit exists, review the issue's linked documents only; do not expand into a repository-wide audit. Read every applicable `AGENTS.md` from the root to each affected path. Resolve citation keys with `jit item show @/citation/<key>`.

## Rubric — blocking on failure

- Trace every quantitative claim (speedup, BLER, threshold, probability estimate, crossover, sample statistic) in attributable text to a committed artifact. Reject prose-only numbers.
- Require every stochastic or performance result to record seeds, RNG, git SHA, hardware, toolchain versions, and invocation. Reject partial manifests.
- Require sample counts and confidence intervals for every Monte Carlo estimate. Require error bars or interval columns in result tables and plots. Reject bare point estimates.
- Require unmeasured numbers to be labeled as estimates. Reject estimates presented as measurements.
- Require every comparison to name its baseline with version and provenance, and state the hardware. Require reproduction targets to quote the source's numbers under a registry citekey.
- Require external claims to carry a citekey resolving in the registry. Verify the cited work actually supports the claim wherever the context allows; flag mismatches.
- Require contradicting data to be stated: if results falsify a criterion, hypothesis, or cited claim, the text must say so. Reject silent rework or omission.

## Rubric — advisory

- Declare stopping rules and sampling plans before results, not after.
- Version datasets (layout, checksums) rather than overwriting.
- Cross-check determinism where a seeded rerun is cheap.
- Prefer one authoritative results artifact over numbers scattered across documents.

## Verdict policy

Any blocking finding on attributable content: FAIL. Advisory-only findings: PASS with the findings listed. Pre-existing debt outside the attributable footprint is advisory. Do not fail for pending judgment gates. Verify every hard criterion that concerns measurement, statistics, or citation; consume executable-gate evidence from the context rather than re-running it.
