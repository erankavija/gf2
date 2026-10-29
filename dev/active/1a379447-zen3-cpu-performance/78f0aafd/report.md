# Integrated profile preparation

> **Diátaxis Type:** Reference

Preparation of `@/issue/78f0aafd`; measurements await the benchmark window.

Decoder: `decoder-cases.tsv`, `preparation/build-identity.json`,
`decoder-window.sh`, and `smoke/`. The build uses Rust 1.95 release,
native target flags and the committed coding envelope. `smoke/README.txt` identifies the untimed diagnostic outputs as non-evidence.
The smoke exercises both codes, each requested worker count, and the
one-frame cases; it uses neither perf nor a timing window.

REQ-02: `accounting.md` lists every primary material-gap cell with its
profile or prepared launcher. `accounting.json` holds the content joins and
the production-drift classifications. Historical decoder coverage joins the
measured arm digest to its profile build record. Related byte-field and
polynomial profiles remain context when content equality is unproved.
The FI-3 and FI-4 entries are structurally pending. After their receipts land,
regenerate the canonical workload set, then this accounting projection;
the generator joins integrated decoder production inputs and BCH executable
and case identities to the prepared series.

REQ-03: `make-accounting.py` projects per-profile counter intervals, sampled
packing shares, worker scaling, adapter stage probes, and committed assembly
stack references through the shared assembly tool. Each unmeasured mechanism
says so. Static stack references do not establish spills. No approach to a
hardware limit is claimed. `allocation-witness.json` and its logs record the
integrated allocation tests, with applicability limited to their workloads.
`verification.md` records commands and check limitations.

## Restored executables

`restore-arms.py build <subject>` rebuilds each distinct measured executable of
the NR encoder, byte-field and polynomial gap cells at its receipt's recorded
build root, from the revision its launcher log names, after checking that
revision against every recorded build-input digest. `restoration.json` records
each attempt with its expected and obtained digests, build inputs and verdict;
every attempt there restored the measured bytes, so all three series profile
historical code, as the accounting's profiled-code column states. The bytes live under
`.agents/ext/78f0aafd/restored`, which the queue lines export as
`GF2_PROFILE_ARM_ROOT`; the temporary build roots are removed.

The `wide-polynomial-competitiveness` receipt declares no RUSTFLAGS for
`gf2-long-product`; its digest reproduces only with the launcher's
`-C target-cpu=native` build, which `restoration.json` records beside the
receipt's declared value.

`restore-arms.py smoke <subject> <runner>` drives each restored arm through
`benchmark-ab-runner smoke` on throwaway plans in `smoke/restored/`. Every
smoke exits non-zero: these arms predate the zero-window validation position
and refuse it after decoding the request. The profile session requests the
receipt's window count, which they implement.

## Queue lines

Estimates only; enqueue decoder and BCH after committing their inputs.

```tsv
78f0aafd	.agents/worktrees/agent-78f0aafd	25	bash dev/active/1a379447-zen3-cpu-performance/78f0aafd/decoder-window.sh
78f0aafd	.agents/worktrees/agent-78f0aafd	12	GF2_PROFILE_ARM_ROOT="$PWD/.agents/ext/78f0aafd/restored" bash dev/active/1a379447-zen3-cpu-performance/78f0aafd/nr-encode-window.sh
78f0aafd	.agents/worktrees/agent-78f0aafd	20	bash dev/active/1a379447-zen3-cpu-performance/78f0aafd/bch-window.sh
78f0aafd	.agents/worktrees/agent-78f0aafd	25	GF2_PROFILE_ARM_ROOT="$PWD/.agents/ext/78f0aafd/restored" bash dev/active/1a379447-zen3-cpu-performance/78f0aafd/byte-field-window.sh
78f0aafd	.agents/worktrees/agent-78f0aafd	12	GF2_PROFILE_ARM_ROOT="$PWD/.agents/ext/78f0aafd/restored" bash dev/active/1a379447-zen3-cpu-performance/78f0aafd/polynomial-window.sh
```

## Open issues

The restored arms have no passing untimed runner smoke; see Restored executables.

`python3 -B dev/bench_results/ed3d490e/test_run_profile.py` fails
`test_refuses_to_resume_over_a_differing_quality_file` with
`AssertionError: 0 != 2`. The same isolated test fails with HEAD's original
launcher. The fixture's redirected quality record does not redirect the
catalogue digest used by `quality_files`. The lead needs to track that shared
fixture/resolution mismatch; this preparation does not alter its semantics.

Path-discipline departures: the initial skill read and a Cargo-directory
listing used absolute paths, read-only. All writes remain inside this worktree.
