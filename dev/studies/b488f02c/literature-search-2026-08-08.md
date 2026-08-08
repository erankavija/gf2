# Literature search: prior numerics for $\Pr[\mathrm{per}(A) = 0]$ over $\mathbb{F}_q$

Receipt for the novelty claim in `feasibility-study.md` §7.6. It records what was
searched, when, and what was found, so the claim rests on a documented search
rather than on an unsupported assertion.

**Searched:** 2026-08-08. **Engines:** the Claude Code `WebSearch` tool (a
general web index) and direct retrieval of arXiv abstracts and full texts. Not
searched: MathSciNet, zbMATH, Web of Science, or any paywalled full text (see
Limitations).

**Question.** Are there published numerical estimates — Monte Carlo or exact
enumeration — of $\Pr[\mathrm{per}(A) = 0]$ for a uniformly random
$A \in \mathbb{F}_q^{n \times n}$ at $q = 5$ or $q = 7$?

## Queries

Result counts are the number of results the engine returned and that were
scanned, not a count of matching documents in any corpus.

| # | Query | Returned |
|---|---|---|
| 1 | permanent of random matrix over finite field probability zero numerical experiments | 10 |
| 2 | permanental rank random matrices finite fields F_5 F_7 numerics | 8 |
| 3 | Monte Carlo simulation permanent zero fraction F_5 F_7 finite field matrices computation | 9 |
| 4 | Scheinerman fast computation permanents F_3 citing follow-up larger fields | 16 |
| 5 | "permanent" random matrix "F_q" distribution experiments table simulation odd characteristic q=5 | 9 |
| 6 | arxiv permanent random matrices finite field numerical data q=5 q=7 zero probability empirical study 2024 2025 2026 | 8 |
| 7 | uniformity distribution permanent modulo p computational experiments beyond F_3 finite field survey | 9 |
| 8 | "zero permanent" OR "permanent is zero" random matrices "F_5" OR "GF(5)" OR "F_7" OR "GF(7)" computational enumeration counts | 8 |
| 9 | Budrevich "number of matrices with nonzero permanent over a finite field" exact counts small n | 10 |
| 10 | Budrevich Guterman permanent has less zeros than determinant finite fields explicit values q n table | 10 |

## Relevant hits examined

| Work | What it contains | Overlap with $q \in \{5,7\}$ numerics |
|---|---|---|
| [Scheinerman2024] arXiv:2407.20205, *Fast computation of permanents over $\mathbb{F}_3$ via $\mathbb{F}_2$ arithmetic* | Exact counts for $n \le 5$ and Monte Carlo for $6 \le n \le 30$ | **None.** $\mathbb{F}_3$ only; the method is specific to representing $\mathbb{F}_3$ in pairs of $\mathbb{F}_2$ words |
| [HKS2026] arXiv:2603.15856, Hunter–Kwan–Sauermann | Purely theoretical: Theorem 1.3 and Theorem 1.4, no tables or simulations (full text retrieved and checked) | None |
| [GGK2025] arXiv:2512.03221, Ghasemi–Gross–Kopparty | Theoretical; permanental vs determinantal rank. Abstract states results asymptotically, no numerics surfaced | None found (see Limitations) |
| Budrevich, *The number of matrices with nonzero permanent over a finite field*, J. Math. Sci. 232(6):752–759, 2018 | Method for **lower bounds** on the count of nonzero-permanent matrices | Bounds, not measured or enumerated zero fractions. Full text not accessed |
| Budrevich & Guterman, *Permanent has less zeros than determinant over finite fields*, Contemp. Math. 579, AMS 2011 | Proves the zero-permanent probability is strictly below the zero-determinant probability for odd characteristic, $n \ge 3$ | An inequality between two probabilities, not an estimate of either. Full text not accessed |
| Kogan, *Computing permanents over fields of characteristic 3*; Tarin, polynomial-time permanent in $\mathrm{GF}(3^q)$ | Algorithms for characteristic 3 | Algorithmic, characteristic 3, no distributional numerics |
| *On the Pólya permanent problem over finite fields*, arXiv:1003.1984 | Pólya convertibility | Different question; no zero-fraction estimates |
| SUperman (arXiv:2502.16577), *A New Fast Computation of a Permanent* (arXiv:1908.06371) | Permanent computation performance | Algorithms over other domains; no $\mathbb{F}_q$ zero-fraction statistics |

## Corroboration from the literature itself

[HKS2026] §1 makes the negative statement directly, and its authors are domain
specialists reviewing this exact literature:

> "Permanents of random matrices over finite fields have received quite some
> interest in the computer science community […] due to a phenomenon called
> random self-reducibility, but surprisingly we were not able to find any study
> of the asymptotic distribution of $\mathrm{per}(A)$ in this literature."

The only computational evidence [HKS2026] cites for Conjecture 1.2 is
[Scheinerman2024], described as "backed by quite convincing computational
evidence" — and that evidence is $\mathbb{F}_3$ only.

## Limitations

1. No paywalled full text was read. The two Budrevich items are the closest
   candidates for exact small-$n$ counts at $q \in \{5, 7\}$, and neither was
   accessible; both are described by their abstracts and by [HKS2026]'s summary
   as bounding or comparing probabilities rather than estimating them.
2. [GGK2025]'s full text was not exhaustively checked for an appendix of
   numerics; only the abstract and landing page were retrieved.
3. A general web index is not a systematic bibliographic search. MathSciNet and
   zbMATH subject searches would strengthen this, and neither was run.
4. Absence of evidence over ten queries is weak evidence of absence. The claim
   this receipt supports is therefore about what *this* search found, not about
   what exists.

## Conclusion

This search found no published numerical estimates of
$\Pr[\mathrm{per}(A) = 0]$ over $\mathbb{F}_5$ or $\mathbb{F}_7$, exact or
Monte Carlo. Combined with [HKS2026]'s own statement that its authors found no
study of the asymptotic distribution of $\mathrm{per}(A)$ in this literature,
that supports the campaign's $q \in \{5, 7\}$ arms being unmeasured ground as
far as a documented search reaches. It does not establish priority, and §7.6
states the claim at that strength.
