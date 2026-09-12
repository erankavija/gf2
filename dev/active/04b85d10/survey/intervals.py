"""Interval estimators shared by the survey's summary generators (jit:04b85d10).

`median_interval` is the distribution-free interval for a median from order
statistics. For n independent draws from a continuous distribution, the
number of draws below the population median is Binomial(n, 1/2), so the
interval from the j-th smallest to the j-th largest draw covers the median
with probability 1 - 2 P(Binomial(n, 1/2) <= j - 1). The function takes the
narrowest such interval whose coverage reaches the requested level and
returns the coverage it achieves.

`wilson_interval` is the Wilson score interval for a binomial proportion
[Wilson1927].

`bootstrap_ratio` is the percentile bootstrap of a ratio of two medians
measured in separate cells. Each replicate resamples both cells' values
independently with replacement and the interval is the nearest-rank
percentile pair of the sorted replicates. The generator is CPython's
`random.Random` (MT19937) seeded with the value the caller records beside the
interval.
"""

import math
import platform
import random
import statistics

Z_95 = 1.959963984540054
BOOTSTRAP_RESAMPLES = 10000
# The resampling generator, named wherever a bootstrap seed is printed.
BOOTSTRAP_GENERATOR = f"CPython {platform.python_version()} random.Random (MT19937)"


def median_coverage(n, j):
    """Coverage of [X_(j), X_(n+1-j)] for the median of n continuous draws."""
    tail = sum(math.comb(n, k) for k in range(j)) / 2**n
    return 1.0 - 2.0 * tail


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
