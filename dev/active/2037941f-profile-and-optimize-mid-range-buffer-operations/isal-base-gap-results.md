# ISA-L scalar XOR comparison

> **Diátaxis Type:** Reference

ISA-L [IsaL2026] supplies the qualified `xor_gen_base/scalar` comparator. The
[pilot receipt](../../bench_results/2037941f/2037941f-logical-isal-base-gap/v4-r1-pilot/receipt.json)
is accepted as exploratory evidence. Its raw pairs, selected paths, intervals,
and separate setup costs appear in the generated
[cell tables](../../bench_results/2037941f/isal-base-gap-tables.md). The
candidate trails the gf2 peer in every measured cell. The generated
[stopping result](../../bench_results/2037941f/isal-base-gap-outcome.md)
applies the frozen family ceiling to the primary cells and records
`resolution-insufficient`; this family makes no confirmatory reservation or
production selection.

The [ISA-PILOT-6 exception](isal-sampling-discrepancy.md) records the
invoker-approved terminal use of this six-pair pilot despite the prose
addendum's 24-pair requirement. Runner acceptance alone does not establish
compliance with that requirement.

The comparison uses aligned, fresh destinations and the same bytewise XOR
output. The gf2 arm copies the first input and calls the public in-place XOR;
the ISA-L arm forms its pointer array and calls `xor_gen_base`. Both observe the
output inside the timed call. The receipt's kernel-isolated windows and the
tables' separate setup probes answer the isolated question. They do not
establish a whole-consumer speedup. The
[semantic qualification](isal-probe-record.txt) checks the canonical bit mapping,
source preservation, and boundary lengths. Semantic equivalence applies to
the frozen aligned and boundary cells. Offset ISA-L pointers fall outside the
qualified alignment contract; the generated companion availability table
records them as inapplicable with zero samples.

The public SIMD-dispatched ISA-L `xor_gen` route has no qualified executable on
this host. The generated [availability table](../../bench_results/2037941f/isal-dispatch-availability.md)
preserves the NASM-absence and offset-alignment reasons with zero samples from
the qualification records. It does not infer SIMD performance from the scalar
arm.

The [launcher log](../../bench_results/2037941f/2037941f-logical-isal-base-gap/v4-r1-pilot/launcher.log)
and [invocation log](../../bench_results/2037941f/2037941f-logical-isal-base-gap/v4-r1-pilot/invocations.log)
retain the exact run commands. Receipt-local input snapshots pin source,
configuration and toolchain; [arm build provenance](isal-arm-build-record.txt)
pins compiler identity, cleared Make overrides, object `.comment` records,
and the verbose build log.

The [logical addendum](logical-buffer-addendum.md) and
[harness contract](logical-harness.md) place the external availability rows in
the companion table. The accepted runner receipt contains the measured scalar
cells, and its raw bytes remain the evidence for acceptance.
