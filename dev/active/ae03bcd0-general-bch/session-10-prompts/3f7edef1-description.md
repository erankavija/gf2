Establish agreement of the predeclared corpus's construction and encoding results with the two named external oracles and with authoritative standards vectors, committing the fixtures with provenance.

## Background

The evidence-protocol contract predeclares the corpus rows and the two oracles (SageMath `codes.BCHCode`; GAP with GUAVA `BCHCode`), both applicable to the full corpus by their documented domains; exact oracle versions are recorded in the provenance document at fixture generation. Existing DVB-T2 verification vectors remain in place. Both oracles are required on every corpus row; a row where an oracle fails to produce a result is a blocking finding, not a recorded gap. On corpus row B4 the GAP/GUAVA result is the `BCHCode` generator derivation and GUAVA's cyclic-code polynomial encoding map, with the run's bounded `BCHCode` attempt recorded, as Amendment 2 of the evidence protocol fixes; every other row requires the full code object.

## Success Criteria

- [hard] REQ-01: Committed fixtures record generator polynomials and codewords from both named oracles agreeing with this implementation across the predeclared corpus (B4 per the amended protocol), with generation scripts, versions, and provenance.
- [hard] REQ-02: Authoritative standards vectors are wired into the suite where the protocol names them.

## Decisions

- D-01: On corpus row B4 the GAP/GUAVA oracle result is GUAVA's `BCHCode` generator derivation (`PrimitiveUnityRoot` and the cyclotomic-coset loop over `MinimalPolynomial`) together with its cyclic-code polynomial encoding map, with the generating run's bounded `BCHCode` attempt and the outcome it observed recorded in the fixture and the run receipt, as evidence-protocol Amendment 2 fixes. Ground: GUAVA's `BCHCode` materializes the $65343 \times 65535$ generator matrix through `GeneratorPolCode`, and the bounded attempt with a 50 GiB heap on the generating host exhausted it (attempt record under `oracle/attempts/2026-09-02-b4-heap-50g/`), so a B4 code object is a host-memory question rather than a mathematical one. Owner ruling at the lead's escalation.
