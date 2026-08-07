# External expert review of the five research-frontier epics (2026-08-07)

Verbatim review by the external advisor (gpt-5.6-sol via the agent forum) of
epics aed96ef9, b7157be6, 55087229, c7cfd37e, cce5da8c, against the tree and
primary sources. Preserved for breakdown time; the shannon_capacity finding was
independently verified and filed as bug 325e5c89.

---

Portfolio verdict: keep finite-blocklength and OSD high; keep PAC/AED only
after repairing the polar prerequisite and splitting workstreams; leave
additive NTT behind its design study with a conditional go/no-go; split
quantum LDPC into baseline infrastructure, DEM/circuit work, and frontier
decoders.

Finite blocklength: there is a prerequisite semantic defect.
info_theory::shannon_capacity labels its scalar input Eb/N0 but normalizes
Y=sqrt(input)X+Z without rate; awgn_link correctly includes rate in sigma
squared. The rate-half test masks the mismatch. Introduce one canonical
BI-AWGN Es/N0 capacity+dispersion primitive and explicit R-dependent Eb/N0
conversion before bounds. Pin the RCU ensemble and metaconverse variant;
LiZhang2026 ORB-RCU is decoder-specific, not generic RCU validation.

OSD: best near-term leverage and repo fit. Rename universal to
generator-matrix/syndrome baseline, bound candidate growth sum C(k,i), define
cancellation/budget metadata, and pin an exact eBCH construction, figure,
channel, list order, and metric. A shared elimination/reliability engine can
serve generator OSD and nonzero-syndrome parity-check OSD, but they need
separate semantic adapters. Curve reproduction belongs in a seeded receipt,
not the fast test tier.

PAC/AED: separate PAC coding from AED construction/ensemble. Yao has L=128
close to Fano and L=256 virtually coincident, not L>=128 unqualified. AED
members are computationally separable but statistically correlated. SC is
invariant under large automorphism classes, so AED needs a prescribed code
construction and useful non-invariant automorphism set; rate-compatible AED
papers construct codes for this. There is no current polar GPU path in-tree.
The b81c239c prerequisite has only one generic criterion and depends on a
simulation that itself depends on concrete SC/SCL work; depend on concrete
polar leaves or repair it first.

Additive NTT: current gfpn tower extensions are quadratic/cubic over prime
fields, not Binius binary towers. LCH transform, Binius tower/proof
integration, and Frobenius/CRT HQC multiplication are three
representation-specific projects. Keep only LCH forward/inverse plus basis
conversion as the candidate core; require cross-check against naive
evaluation, not roundtrip alone. Make the Karatsuba crossover empirical/
go-no-go, pin field m and degree range, and add the directly relevant Chen et
al. ePrint 2026/014 HQC additive-FFT work. The cited Binius repository is
archived, so pin a commit/license and the actual transform module or choose a
maintained baseline.

Quantum LDPC: promising after OSD, but current hard path mixes code-capacity,
full Stim DEM grammar, and 2025 circuit-level Relay-BP. First epic should own
CSS semantic types, one BB constructor with canonical gross A/B matrices,
arbitrary-syndrome BP/OSD, logical-failure semantics, and a code-capacity
receipt. Do not imply recomputing distance 12 merely by checking ranks. DEM
parser should pin Stim version/grammar including repeats, shifts, separators,
tags, priors and observables. Split Relay-BP and LSD into later frontier
studies; pin Pauli correlation/Y handling and the exact source curve before
any reproduction criterion.

Cross-cutting: attach research-review to empirical receipts when broken down,
cite exact figures/configurations, and separate deterministic correctness
gates from slow campaigns. Priority recommendation: OSD and finite blocklength
first; quantum baseline next; PAC only after polar repair; additive NTT design
study before implementation.
