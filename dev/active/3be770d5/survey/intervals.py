"""Interval estimators of the profile summaries (jit:3be770d5).

`median_interval` is the distribution-free interval for a median from order
statistics: for n independent sessions from a continuous distribution, the
count below the population median is Binomial(n, 1/2), so [x(j), x(n+1-j)]
covers the median with probability 1 - 2 P(Binomial(n, 1/2) <= j - 1). The
narrowest interval reaching the requested coverage is returned with that
coverage.

`wilson_interval` is the Wilson score interval for a binomial proportion
[Wilson1927], the interval the protocol uses for FER.

`bootstrap_ratio` is the percentile bootstrap of a ratio of two medians whose
per-session values come from separate cells: each replicate resamples both
value lists independently with replacement, and the interval is the
nearest-rank percentile pair of the sorted replicates. The generator is
CPython's `random.Random` (MT19937), seeded with the value printed beside
each interval.
"""

import math
import platform
import random
import statistics

Z_95 = 1.959963984540054
BOOTSTRAP_RESAMPLES = 10000
BOOTSTRAP_GENERATOR = f"CPython {platform.python_version()} random.Random (MT19937)"


def median_coverage(n, j):
    """Coverage of [X_(j), X_(n+1-j)] for the median of n continuous draws."""
    return 1.0 - 2.0 * sum(math.comb(n, k) for k in range(j)) / 2**n


def median_interval(values, level=0.95):
    """Returns (median, lower, upper, coverage) of `values`."""
    ordered = sorted(values)
    n = len(ordered)
    if n == 0 or median_coverage(n, 1) < level:
        raise ValueError(f"{n} values cannot give a {level:.0%} median interval")
    j = 1
    while j + 1 <= n // 2 and median_coverage(n, j + 1) >= level:
        j += 1
    return statistics.median(ordered), ordered[j - 1], ordered[n - j], median_coverage(n, j)


def wilson_interval(successes, trials, z=Z_95):
    """Returns (lower, upper) of the Wilson score interval."""
    if trials == 0:
        return float("nan"), float("nan")
    p = successes / trials
    denominator = 1.0 + z * z / trials
    centre = (p + z * z / (2 * trials)) / denominator
    half = z * math.sqrt(p * (1 - p) / trials + z * z / (4 * trials * trials)) / denominator
    return max(0.0, centre - half), min(1.0, centre + half)


def nearest_rank(quantile, count):
    """Zero-based index of the nearest-rank `quantile` of `count` sorted values."""
    return min(count - 1, max(0, math.ceil(quantile * count) - 1))


def bootstrap_ratio(numerator, denominator, seed, level=0.95, resamples=BOOTSTRAP_RESAMPLES):
    """Returns (estimate, lower, upper) of median(numerator) / median(denominator)."""
    if len(numerator) < 2 or len(denominator) < 2:
        raise ValueError("a bootstrap needs at least two values per cell")
    generator = random.Random(seed)
    replicates = sorted(
        statistics.median(generator.choices(numerator, k=len(numerator)))
        / statistics.median(generator.choices(denominator, k=len(denominator)))
        for _ in range(resamples)
    )
    alpha = 1.0 - level
    return (
        statistics.median(numerator) / statistics.median(denominator),
        replicates[nearest_rank(alpha / 2, resamples)],
        replicates[nearest_rank(1 - alpha / 2, resamples)],
    )
