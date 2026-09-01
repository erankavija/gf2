# Prepared record updates for issue `389aa4de` (REQ-03)

REQ-03 requires that no committed record still states
`polynomial.karatsuba_min_degree` is unsweepable. This file holds the prepared
text for every such record, unapplied. Nothing here is committed to the
documents it names.

Apply after the measured calibration run of this issue exists and its receipt is
committed: three of the blocks cite that receipt by path, written below as
`<new receipt>`.

Block E is not part of REQ-03. It records what the DEC-B17 harness-schema bump
must carry when it lands with that run, and it is the one block whose scope is
larger than the records above.

## Rule the blocks follow

Append-only. Committed records under `dev/active/` and `dev/benchmarks/` carry
citations anchored to the tree they were written against, so an amendment is
appended and changes no pre-existing line
(`dev/active/6dc81018-field-capability-dispatch/handoff.md:57`). The section
shape follows the precedents at
`dev/benchmarks/tuning_profiles/across-build-control-arm-v1.md:179` and
`dev/benchmarks/tuning_profiles/selector-non-regression-plan-v1.md:314`.

Nothing in `2026-08-20-host-calibration.md` is rewritten, and in particular the
emitted document quoted byte-for-byte at `:293` and every measured figure stay
as taken. `@/inv/behavioral-evidence-validity` keeps them valid: they record
what one binary, identified by its `binary_sha256`, measured on one host, and
this issue re-measures none of it. What the amendment supersedes is that
receipt's forward-looking claim that the field cannot be swept — not its record
of what was swept.

## Records that state the limitation

| Record | Anchor | Statement |
|---|---|---|
| `dev/benchmarks/tuning_profiles/2026-08-20-host-calibration.md` | `:338`–`:367` (F2) | "no grid point offers both arms and §2.7 rule 1 cannot be executed for this field" |
| `dev/active/6dc81018-field-capability-dispatch/handoff.md` | `:26` (DEC-B6) | "`karatsuba_min_degree` is not sweepable (both arms private …)" |
| `dev/active/7d824b2f/design.md` | `:427`–`:431`, `:455`–`:461` | names the steering technique as one `389aa4de` builds, and cites the field's omission as the precedent for seventeen non-sweepable fields |
| `crates/gf2-core/benches/tuning_calibration.rs` | module docs, `# Arm reachability` | **already rewritten** by this issue's own commits; listed so the inventory is complete |

## Block A — append at the end of `dev/benchmarks/tuning_profiles/2026-08-20-host-calibration.md`

```markdown
## Amendment — issue `389aa4de` (<date>)

This section is appended at the end of the receipt and changes no line above it.

F2 records `polynomial.karatsuba_min_degree` as uncalibrated at this receipt's
revision: `FieldPoly::mul` resolved its arm from the compiled-in
`KARATSUBA_THRESHOLD`, so no grid point offered both arms, and F2 names issue
`389aa4de` as the tracked owner of the gap together with the mechanism that
closes it.

That condition is discharged. The polynomial cutover `697fc55b` has `mul_impl`
resolve the threshold from `tuning::active()`, and the calibration harness now
reaches both arms at every grid point by installing a profile that forces one.
Each arm is timed in a child process, because `tuning::install` resolves the
process-wide profile once, and each child reports the arm the production
selector `gf2_core::field::poly::mul_route` picks under the installed profile
along with digests of its operands and its product, which the calibrating
process checks. `<new receipt>` records that sweep and the value it selected.

The field is therefore sweepable and no longer omitted from an emitted profile.
Every figure above, the emitted document quoted under §Emitted profile, and
F2's account of what this run measured stand as taken.
```

## Block B — append as a bullet to the current session section of `dev/active/6dc81018-field-capability-dispatch/handoff.md`

```markdown
- **DEC-B6 discharged by `389aa4de`**: `karatsuba_min_degree` is sweepable
  after the polynomial cutover. Both arms come off `FieldPoly::mul` when a
  child process installs a profile forcing the threshold and the production
  route reporter confirms the arm, so an emitted profile now states a measured
  value for the field instead of omitting it. `<new receipt>` records the
  sweep.
```

## Block C — append at the end of `dev/active/7d824b2f/design.md`

Recommended rather than required by REQ-03's wording: §5.1 condition 2 states
the mechanism in the future tense and §5.2 cites the field's omission as a
current precedent, and `@/inv/present-tense-prose` makes both stale once the
sweep lands.

```markdown
## Amendment — issue `389aa4de` (<date>)

This section is appended at the end of the document and changes no line above
it.

§5.1 condition 2 names the profile-steered technique as one issue `389aa4de`
builds. It is built: `crates/gf2-core/benches/tuning_calibration.rs` reaches
both arms of a runtime threshold field by spawning one child process per arm,
each installing a profile that forces its arm before any selection boundary
runs, and each reporting the arm back through the family's production route
reporter so the parent checks it rather than assuming it. A follow-on field
that satisfies §5.1's three conditions uses that mechanism, as the section
requires, and no second steering mechanism exists.

§5.2's closing paragraph cites `karatsuba_min_degree`'s omission as the
precedent for the seventeen non-sweepable fields. Their standing rule is design
220cab0b §5 condition 5 itself, which each satisfies for the reason §5.2's
table records; `karatsuba_min_degree` is no longer an instance of it, and
`<new receipt>` records its sweep.
```

## What `<new receipt>` must carry for REQ-02

The harness prints each of these, so the receipt transcribes rather than
derives them.

- The grid and, per grid point, both arms' medians, spreads and window counts,
  with the margin and the noise band.
- The selected value with its selecting margin and band, or — when the rule
  keeps the default on a tie, a non-monotone crossover, or no winning grid
  point — the fallback reason and the default kept. §2.7 rule 3 and REQ-02 both
  ask for that case explicitly.
- The forcing policy, which the sweep header prints beside the arm names: the
  top of the admissible range for the schoolbook arm, the grid point itself for
  the Karatsuba arm.
- The omission inventory: the harness prints one row per omitted schema field
  with its inherited value and whether no sweep covers it or no grid point
  offered both arms.
- What this field's asymptotic arm is, per the section below.

## What the forced Karatsuba arm measures

The schoolbook arm forces `usize::MAX`, the top of the field's admissible
range, where no grid point routes to Karatsuba. The Karatsuba arm forces **the
grid point itself**, per owner decision DEC-B16.

`mul_karatsuba_raw` recurses on the same profile value, so the value forced
decides which algorithm is timed. Forcing the grid point makes the recursion
split once at that degree and hand its sub-operands, at about half the degree,
to the schoolbook base case — the algorithm the dispatcher runs when
`karatsuba_min_degree` is set to that grid point. Each grid point therefore
compares the two arms the selection rule chooses between at that point, so the
receipt may read the selected value as a threshold recommendation without a
recursion-shape caveat.

## Block D — DEC-B16, the mechanism the earlier records describe

Three records describe the forcing as minimal-against-maximal: the issue's own
Background, `dev/benchmarks/tuning_profiles/2026-08-20-host-calibration.md`
F2's tracked-owner paragraph at `:361`–`:367`, and
`dev/active/7d824b2f/design.md` §5.1 condition 2 at `:427`–`:431`. DEC-B16
supersedes that detail. The paragraph below carries the correction; append it
to Block A and to Block C, and apply it to the issue Background.

```markdown
Owner decision DEC-B16 fixes one detail of the mechanism these records
describe. The forcing is not minimal-against-maximal. The schoolbook arm
installs the top of the admissible range, where no grid point routes to
Karatsuba, and the Karatsuba arm installs the grid point itself.
`mul_karatsuba_raw` recurses on the same profile value, so forcing the bottom
of the range would time a Karatsuba recursion carried to its degree-0 base
case — an algorithm no threshold produces — while forcing the grid point times
the single split over schoolbook base cases that a threshold at that grid point
does produce. Both forced values stay ordinary admissible values and reserve no
sentinel.
```

### Evidence for DEC-B16

Both readings were measured on the same binary and host. The numbers below come
from unpinned smoke runs on a contended host — no lock wrapper, three windows,
minimum-of-three reported — so they are evidence of a direction, not of a
crossover, and no receipt may cite them as a measurement.

Forcing the bottom of the range put the Karatsuba arm behind the schoolbook arm
at **every** grid point, by 12.3x at degree 4 falling to 2.9x at degree 256, so
the §2.7 rule kept the conservative default for want of a winning grid point.

Forcing the grid point reverses it:

| Operand degree | schoolbook ns | karatsuba ns | karatsuba / schoolbook |
|---:|---:|---:|---:|
| 16 | 346.6 | 373.3 | 1.077 |
| 31 | 1288.8 | 1083.2 | 0.841 |
| 32 | 1267.8 | 1204.3 | 0.950 |
| 64 | 5053.8 | 4087.8 | 0.809 |
| 128 | 19260.6 | 15504.5 | 0.805 |

The Karatsuba arm loses at 16 and wins from 31 upward, so a crossover exists
between those degrees. That agrees with what `2026-08-20-host-calibration.md`
recorded from the dispatcher curve alone at `:188`–`:193`: `FieldPoly::mul`
cost 1248.166 ns at degree 31 on the schoolbook arm and 1135.357 ns at degree
32 on the Karatsuba arm, a step **down** for one more coefficient, which that
receipt noted as consistent with the crossover lying below 32 while claiming
no crossover from it. The measured run decides the value.

## Block E — what the DEC-B17 harness-schema bump must carry

DEC-B17 raises `HarnessSchema::SUPPORTED` to `tuning-calibration-v2` and has
`HarnessSchema::parse` accept exactly that token, because this issue changed the
harness's timing path for one field and the emitted document's field set —
the behavioural identity `@/inv/behavioral-evidence-validity` has that token
pin. DEC-B17 as amended lands the bump in the same change that commits the
measured run's v2 profile, so no committed artifact is left unloadable between
the two.

The bump was applied as a probe on this branch, verified, and reverted. What it
breaks is wider than the loader, because
`crates/gf2-core/data/tuning-profiles/gf2-5ecc9bf8-calibration-e202c080.json`
is not only a committed record: it is the **drift anchor of the whole baked
follow-on mechanism** of `dev/active/7d824b2f/design.md`, and four test binaries
plus the library's own unit tests read it. Its `harness_schema` is
`tuning-calibration-v1`, so a loader that rejects that token rejects the file.

### Every reader of the committed calibrated profile

| Reader | How it reads the file | Effect of the bump |
|---|---|---|
| `crates/gf2-core/tests/tuning_profile_committed.rs:21` `every_committed_tuning_profile_is_valid` | directory glob, `from_json().unwrap()` | fails, `Malformed` |
| `crates/gf2-core/tests/tuning_profile_committed.rs:32` `committed_calibrated_profile_inherits_follow_on_defaults` | `from_json().unwrap()` | fails, `Malformed` |
| `crates/gf2-core/src/tuning/mod.rs:2426` `calibrated_profile_round_trips` | fixture document embedding the v1 token | fails, `Malformed` |
| `crates/gf2-core/src/tuning/mod.rs:2528` `bounded_fields_report_their_family_and_field` | fixture document embedding the v1 token | fails, `Malformed` |
| `crates/gf2-core/src/tuning/baked.rs:110` `baked_constant_matches_committed_profile_file` | `include_str!`, parsed as raw JSON | survives the bump; see the re-pinning hazard below |
| `crates/gf2-core/src/tuning/baked.rs:120` `committed_profile()`, feeding eight `assert_matches_profile_or_default` call sites | `include_str!`, parsed as raw JSON | same |
| `crates/gf2-core/tests/gemm_tiles_baked.rs:50` | `include_str!` | same |
| `crates/gf2-core/tests/prime_route_baked.rs:36` | reads the file at run time | same |
| `crates/gf2-core/tests/field_vec_baked.rs:31` | reads the file at run time | same |

The last five parse the document as raw `serde_json`, not through
`TuningProfile::from_json`, so the token bump alone does not fail them. They are
listed because the *file replacement* reaches them and the loader change does
not. The two `baked.rs` entries run under `--lib` and the three test binaries
run by name, all inside `scripts/cargo-ci.sh`'s required `baked` step (`:236`).

### The re-pinning hazard, which is the part to decide before running

`baked.rs:15` states `SIMD_MIN_WORDS = 4` as the calibrated value "from profile
`gf2-5ecc9bf8-calibration-e202c080`", and
`baked_constant_matches_committed_profile_file` asserts the constant equals that
file's `bit_backend.simd_min_words`. Every other baked follow-on constant is
anchored the same way through `assert_matches_profile_or_default`: the baked
value must equal the profile's field when the profile carries it, and the
conservative default when it omits it — the D5 contract.

Replacing the file with the measured run's profile therefore re-pins DEC-G. If
the new run measures `simd_min_words != 4`, the drift test fails until
`baked::SIMD_MIN_WORDS` moves with it, and moving it moves the baked
bit-backend selection boundary — the value DEC-G ratified on
`2026-08-20-post-cutover-receipt-3.md`'s residual-cost numbers, guarded by the
frozen selector non-regression bracket. That is a performance re-pin with its
own evidence obligation, not a side effect of a token bump. An unpinned smoke
run of this harness emitted `simd_min_words = 2`, so the two values differing is
the expected case rather than the unlikely one.

The same replacement also interacts with this issue's omission set: an emitted
v2 profile states only the swept fields, so every baked follow-on constant whose
field the sweep does not cover moves from "profile omits it, inherits the
default" to the same state under a different file — which the D5 contract
already admits, but which the change must assert rather than assume.

Two shapes are available, and the choice belongs to the owner:

1. **Replace** `gf2-5ecc9bf8-calibration-e202c080.json` with the v2 profile.
   One committed calibrated profile, no stale token anywhere, and DEC-G is
   re-pinned to the new measurement with the evidence that requires.
2. **Add** the v2 profile beside it under a new `profile_id` and keep the v1
   file as DEC-G's anchor. DEC-G is untouched — but the v1 file no longer loads
   under a v2-only loader, so this shape needs the v1 file regenerated with the
   v2 token from its own recorded numbers, or it reintroduces exactly the
   orphaned artifact DEC-B17 was amended to avoid.

### Checklist for the change that lands the bump

- [ ] `crates/gf2-core/src/tuning/mod.rs`: `HarnessSchema::SUPPORTED` to
      `tuning-calibration-v2`; `parse` still accepts exactly `SUPPORTED`; the
      type's rustdoc at `:202` names the new token.
- [ ] `crates/gf2-core/src/tuning/mod.rs:2426` and `:2528`: update the v1 token
      in both f35daec0 fixture documents.
- [ ] `crates/gf2-core/data/tuning-profiles/gf2-5ecc9bf8-calibration-e202c080.json`:
      replaced by, or joined by, the measured v2 profile per the shape chosen
      above.
- [ ] `crates/gf2-core/tests/tuning_profile_committed.rs`: both tests point at
      whichever committed calibrated profile survives.
- [ ] `crates/gf2-core/src/tuning/baked.rs`: the `include_str!` at `:110` and
      `:120`, the profile id in the rustdoc at `:15`, and `SIMD_MIN_WORDS`
      itself if the new measurement moves it.
- [ ] `crates/gf2-core/tests/gemm_tiles_baked.rs:50`,
      `prime_route_baked.rs:36`, `field_vec_baked.rs:31`: same path update.
- [ ] `scripts/cargo-ci.sh`'s `baked` step (`:236`) passes — it is the gate that
      catches a half-applied re-pin.
- [ ] If `simd_min_words` moves: a DEC-G re-pin record, since the baked
      bit-backend boundary is a performance claim under
      `@/inv/benchmark-backed-performance`.
- [ ] `dev/active/7d824b2f/design.md:372` states that
      `gf2-5ecc9bf8-calibration-e202c080.json` "keeps loading unchanged". The
      bump falsifies that sentence; it needs an appended amendment under the
      same append-only rule as Blocks A to C.

### Emitted-artifact supersession for `2026-08-20-host-calibration.md`

The receipt's §Emitted profile quotes the document byte-for-byte at `:293` and
identifies it at `:93` by the `/tmp` path and SHA-256
`f0c3100ddd23fe3bbaadb5bf3b7fde51af78fc6e9929bb76886b1b032a165635`. Both
describe the file that run wrote, not a path under version control, so neither
is invalidated by anything the bump does — and neither may be edited. The
receipt does carry one sentence that the intervening history already overtook:
at `:15`–`:18` it states no calibrated profile is committed under
`crates/gf2-core/data/tuning-profiles/`, which was true when written and stopped
being true at `7163e2a9` (`676f55a2`, DEC-G).

Append this to Block A, or as its own appended section if Block A has already
landed:

```markdown
### Emitted artifact — later history

The paragraph above §Result records that no calibrated profile is committed
under `crates/gf2-core/data/tuning-profiles/`. That was the state at this
receipt's revision. Commit `7163e2a9` (issue `676f55a2`, DEC-G) later committed
this run's emitted document there as
`gf2-5ecc9bf8-calibration-e202c080.json`, where it became the drift anchor for
the baked selector constants of `dev/active/7d824b2f/design.md`.

Under owner decision DEC-B17 the calibration harness's schema token moves to
`tuning-calibration-v2`, because the harness's timing path and emitted field
set changed — the behavioural identity `@/inv/behavioral-evidence-validity` has
that token pin. The committed file carrying this run's `tuning-calibration-v1`
provenance is superseded by `<new receipt>`'s profile in that same change.

The document quoted at §Emitted profile, its SHA-256 in the provenance table,
and every measured figure in this receipt stand as taken. They record what one
identified binary measured on one host, which no later change re-measures.
```

## Format-2 supersession (2026-08-26, issue `b749bdfc`)

Blocks A through E above preserve useful historical diagnosis, but their flat
profile, global harness-token, and transitional reader mechanics are not
applicable verbatim after the atomic format-2 cutover. Issue `389aa4de` steers
core selectors through `CoreTuning` and guarded `fresh_tuning_process`
children, then emits and strictly reopens an owner envelope containing exactly
`gf2-core/selectors` through `CoreTuningCodec`.

The five-field sweep derives its omission complement mechanically from the
core codec. The expected relationship is five measured fields and 32 omitted
fields out of the current 37-field core section; those counts are an assertion
over codec output, not a parallel schema inventory. Calibrated measurement
provenance belongs to the core section, while assembly provenance and the
recomputed content digest describe the owner envelope. The core codec accepts
the `tuning-calibration-v2` behavior token; it is not a global loader token.

The resulting core-owner artifact contains no algebra vocabulary. The exact
v1 evidence is archived without retagging, and neither a compatibility reader
nor dual write is part of this follow-on calibration.

## Prepared pre-measurement protocol correction (2026-09-01)

This is a protocol note, not a measurement record. No `GF2_BENCH=1` action,
host calibration, artifact cutover, supersession-block application, or reader
change is represented here.

The prepared harness keeps the core behavior token exactly
`tuning-calibration-v2`: no valid v2 measurement artifact exists yet, and the
correction completes that producer identity rather than replacing established
v2 evidence. Its structured raw-sample records preserve, in acquisition order,
every `(execution, repetition, calls, elapsed_ns)` observation and pin profile
format 2, `gf2-core/selectors`, section schema 1, and the v2 token. Aggregates
and selection remain derived from those integer observations.

Fixture provenance uses an explicit stable tag for each of the five pilot
fields and named role/stream values under `gf2-calibration-seed-v1`. The
harness emits the root, derivation constants, field tag, grid size, role, and
derived seed for every fixture stream, including all bit-buffer banks, so each
fresh child can be reconstructed without depending on enum declaration order.

Every successful forced child reports the requested and production-observed
route, exact installed threshold, `SectionResolution::Installed`, inherited
section measurement kind, profile and section identities, strict format-2
content and canonical section-wrapper digests, the v2 protocol identity, and
its raw samples. The parent recomputes and validates those facts before it
prints the verified observation. A nonzero child, malformed output, wrong
route/value/digest, missing or frozen section, or invalid sample sequence stays
fatal.

The runnable procedure now records a clean build-time HEAD, builds both release
executables outside the benchmark lock through `cargo-budget` and Rust 1.95,
rechecks the same clean HEAD, and compares the in-lock revision with that build
identity. One outer `--full-host` lock span runs the binaries directly and
composes the complete envelope. The harness independently treats a dirty tree
as a hard error before protocol, grid, seed, probe, or timing output. Runtime
provenance requires `RUSTUP_TOOLCHAIN=1.95.0` and verifies `rustc 1.95.0`; the
lock probe matches the exact `GF2_CCX1_LOCK` path (or
`/tmp/gf2-ccx1.lock`) by device and inode.
All owner, complete, stdout, stderr, composer-log, and hash destinations must
be absent before the run, and post-run SHA-256 records cover both executables,
both artifacts, and all diagnostics.

Publication is conditional on the codec-derived inventory being exactly five
measured fields and the 32-field complement of the current 37-field core
schema. A build without a comparable SIMD arm therefore cannot publish a 4/33
artifact.

## Executed state and publication corrections (2026-09-01)

The measured run exists at
`dev/benchmarks/tuning_profiles/2026-09-01-389aa4de.md`. Blocks A through D are
applied as append-only, format-2-aware amendments: F2 and DEC-B6 are discharged
at `karatsuba_min_degree = 31`; DEC-B16's grid-point forcing is explicit; the
current omission count is 32 rather than the pre-pilot count 33; and DEC-G
remains 4 with v2 provenance. The measured core owner, complete envelope, raw
log, and repository-relative SHA-256 manifest are committed under the paths
named by that receipt.

Block E's transitional flat-profile mechanics are not applied. The live
reader is `CoreTuningCodec`, the owner and complete envelopes share one
logical profile ID, and the core wrapper is preserved byte-for-byte through
composition. The three baked omission witnesses strictly reopen the measured
owner. After the reader/citation sweep finds no remaining crate consumer, the
test-only format-1 helper and archived v1 bytes are deleted. The old receipt
retains their exact historical path and digest without a v1 loader.

The first execution attempt passed unsupported `--release` to `cargo bench`
and then exposed that the driver did not propagate the failed build; it
produced no evidence. The second attempt built successfully but applied
`jq -r .` to a bare path file, failed its guarded preflight, and stopped before
the measured span; it also produced no evidence. The approved third attempt
removed the invalid bench flag, used `cat` for both raw path files, propagated
build/extractor failures, checked nonempty executable paths and hashes, bound
the clean build HEAD, and rejected all seven pre-existing destinations. The
harness module prose is corrected to document that executed driver. These are
documentation-only corrections and do not change the v2 binary's sampling,
timing, evaluation, or output behavior.
