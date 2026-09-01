# Handoff 10 — `eaae1b56` preclaim plan contradiction

## State

Issue `389aa4de` is done with all configured gates passed, so all nine
dependencies of `eaae1b56` are met. No claim, lease, worktree, implementation
edit, measurement, or JIT issue mutation has been made for `eaae1b56`.

## Blocking evidence

The governing design's §5.1 sweepability rule requires a size grid straddling
the conservative default and both arms reachable at every point. Section 3.3
and `M4rmSelectors::try_new` give `m4rm.tiled_min_stride_words` an admissible
floor and conservative default of 4. The tiled schedule needs at least one
four-word tile. Values 1 through 3 are rejected, so no admissible below-default
point can expose the tiled arm. A grid beginning at 4 does not satisfy the
reviewed straddling rule.

The design and issue also name no numeric grids or representative fixture
shapes for any of the eleven selectors. Those choices must be reviewed and
committed before timing; a worker must not select them during a producing run.

## Owner choices

1. **Boundary-clipped rule (recommended):** amend the convention source and
   issue so a threshold whose default equals its admissible boundary may use a
   one-sided grid beginning at that boundary when both arms remain forceable.
   Retain all eleven selectors and predeclare every numeric grid and fixture.
2. **Ten-field sweep:** reclassify `tiled_min_stride_words` as non-sweepable,
   amend the issue from eleven to ten, and omit the field from the measured
   owner.
3. **Production-contract redesign:** separately design a lower admissible value
   or kernel-contract change before returning to the measurement issue.

Do not add a private forcing hook, bypass `M4rmSelectors::try_new`, silently
publish ten fields while claiming eleven, or start measurement before the
grids and fixtures are predeclared.

## Invoker decision

The invoker selected option A. Amend the shared rule at its convention source:
when a threshold's conservative default equals an admissible boundary, a
domain-clipped grid may begin at that boundary if both arms remain forceable at
every point. Retain all eleven selectors.

Before claim or implementation, commit and independently review the exact
numeric grids, representative fixture shapes, forced-arm values, route
witnesses, seed domains, feature/thread requirements, and execution budget.
This approval does not authorize a private forcing hook, a validation bypass,
or a producing measurement from an unreviewed protocol.
