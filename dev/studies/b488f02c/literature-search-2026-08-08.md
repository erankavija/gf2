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

Every work characterized here is registered in `.jit/references.toml` and cited
by its key. **Reading depth** is stated per work, because most of these
characterizations rest on abstracts rather than full texts, and a characterization
from an abstract can be wrong about what a paper's tables contain.

| Work | Reading depth | What it contains | Overlap with $q \in \{5,7\}$ numerics |
|---|---|---|---|
| `@/citation/Scheinerman2024` | **full text** | Exact counts for $n \le 5$, Monte Carlo for $6 \le n \le 30$ | **None.** $\mathbb{F}_3$ only; the method encodes $\mathbb{F}_3$ in pairs of $\mathbb{F}_2$ words |
| `@/citation/HKS2026` | **full text** (retrieved and read) | Theory only: Theorem 1.3 (eqs. 1.2–1.4), Theorem 1.4. No tables, no simulations | None |
| `@/citation/GGK2025` | **abstract and landing page only** | Theory: permanental vs determinantal rank | None found; an appendix of numerics was not ruled out |
| `@/citation/Budrevich2018` | **abstract only** | A method for **lower bounds** on the count of nonzero-permanent matrices | Bounds, not estimated or enumerated zero fractions. **Closest unexamined candidate for exact small-$n$ counts** |
| `@/citation/BudrevichGuterman2012` | **not read**; characterized from `@/citation/HKS2026`'s summary of it | Proves the zero-permanent probability is strictly below the zero-determinant probability at odd characteristic, $n \ge 3$ | An inequality between two probabilities, not an estimate of either |
| `@/citation/Bassalygo2013` | **not read**; found via the same thread | Counting nonzero permanents at odd characteristic | Same class as the two above: counting and bounds |
| `@/citation/Kogan1996` | **abstract only** | Why permanents are hard over characteristic 3 | Complexity, not distributional numerics |
| `@/citation/Tarin2007` | **abstract only** | Claims a polynomial-time permanent in $\mathrm{GF}(3^q)$, and draws a complexity conclusion large enough that the claim should be treated as unverified here | Algorithmic, characteristic 3, no numerics |
| `@/citation/Dolinar2011` | **abstract only** | The Pólya permanent problem over finite fields | Convertibility, a different question; no zero-fraction estimates |
| `@/citation/Elbek2025` | **abstract only** | GPU permanent computation, real/complex/binary matrices | Not $\mathbb{F}_q$; no zero-fraction statistics |
| `@/citation/Niu2019` | **abstract only**; **withdrawn by its authors in 2020** | A general permanent algorithm | Not $\mathbb{F}_q$-distributional; withdrawn, so carries no weight either way |

## Corroboration from the literature itself

`@/citation/HKS2026` §1 makes the negative statement directly, and its authors
are domain specialists reviewing this exact literature:

> "Permanents of random matrices over finite fields have received quite some
> interest in the computer science community […] due to a phenomenon called
> random self-reducibility, but surprisingly we were not able to find any study
> of the asymptotic distribution of $\mathrm{per}(A)$ in this literature."

The only computational evidence `@/citation/HKS2026` cites for Conjecture 1.2 is
`@/citation/Scheinerman2024`, described as "backed by quite convincing
computational evidence" — and that evidence is $\mathbb{F}_3$ only. Note the
limit of that corroboration: it is a statement about the *asymptotic
distribution* literature as those authors surveyed it, not a guarantee that no
table of finite-$n$ counts exists in a counting paper they had no reason to
cite.

## Limitations

1. No paywalled full text was read. `@/citation/Budrevich2018`,
   `@/citation/BudrevichGuterman2012` and `@/citation/Bassalygo2013` are the
   closest candidates for exact small-$n$ counts at $q \in \{5, 7\}$, and none
   was accessible; all three are characterized from abstracts or from
   `@/citation/HKS2026`'s summary as bounding or comparing probabilities rather
   than estimating them. A counting paper is exactly the kind of work that might
   carry a small table without advertising it in an abstract, so this is the
   likeliest place for this search to be wrong.
2. `@/citation/GGK2025`'s full text was not exhaustively checked for an
   appendix of numerics; only the abstract and landing page were retrieved.
3. A general web index is not a systematic bibliographic search. MathSciNet and
   zbMATH subject searches would strengthen this, and neither was run.
4. Absence of evidence over ten queries is weak evidence of absence. The claim
   this receipt supports is therefore about what *this* search found, not about
   what exists.

## Conclusion

This search found no published numerical estimates of
$\Pr[\mathrm{per}(A) = 0]$ over $\mathbb{F}_5$ or $\mathbb{F}_7$, exact or
Monte Carlo. Combined with `@/citation/HKS2026`'s statement that its authors
found no study of the asymptotic distribution of $\mathrm{per}(A)$ in this
literature, that is consistent with the campaign's $q \in \{5, 7\}$ arms being
unmeasured ground.

**What this does and does not license.** It licenses "a search recorded here
found no prior numerics, subject to the limits above". It does not license
"none exist", "every measured cell is new", or any claim of priority: three of
the closest candidate works were never read, and one general web index is not a
systematic bibliographic search. §7.6 states the claim at the first strength and
not the others.
