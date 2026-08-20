# Third post-cutover receipt for the pinned selector non-regression set

This is the third post-cutover receipt of issue `50b47eae`, and the first taken
under the layout-attribution amendment that issue `972e2b88` predeclares in
[`layout-attribution-verdict-v1.md`](layout-attribution-verdict-v1.md). Each
side of the plan's §5 comparison is an ensemble of 128 builds of one revision
rather than one build of it, so across-build code layout enters the verdict
statistic as a sampled quantity whose dispersion this session measures.

The procedure runs as the amendment fixes it and as
[`selector-non-regression-plan-v1.md`](selector-non-regression-plan-v1.md)
otherwise stands. The pinned cell set of plan §2, the tolerance of plan §4, the
comparison rule of plan §5, the run protocol of plan §3 and the schema token
`selector-non-regression-v1` are unchanged; τ_cell is 5 % and τ_set is 2 % at
their predeclared values. No tuning profile is installed anywhere in this
session.

The three standing receipts —
[`2026-08-19-pre-cutover-baseline.md`](2026-08-19-pre-cutover-baseline.md),
[`2026-08-20-post-cutover-receipt.md`](2026-08-20-post-cutover-receipt.md) and
[`2026-08-20-post-cutover-receipt-2.md`](2026-08-20-post-cutover-receipt-2.md)
— stand as taken. None is modified, superseded, re-run or adjusted by this
receipt.

## Result

**The session establishes no attributable verdict.** The verdict comparison
reports `RESULT: FAIL` and the attribution audit reports `RESULT: FAIL`, so the
fourth row of the amendment's §7 reading table applies: no attributable verdict,
and the epic's gate stays unmet either way.

- **The verdict comparison fails at four cells and passes the set rule.**
  `bit_backend/popcount/words=1` at 1.206586, `bit_backend/popcount/words=8` at
  1.057456, `bit_backend/xor_inplace/words=16` at 1.051039 and
  `bit_backend/xor_inplace/words=1` at 1.050491 exceed τ_cell. The geometric
  mean of the thirty-four ratios is 1.017669, inside τ_set = 1.02. Both rules
  must hold, so the comparison is `RESULT: FAIL`.
- **The audit fails the coverage precondition at two cells.**
  `bit_backend/or_inplace/words=1` records `s_R` = 0.047448 against a floor of
  `σ̂`/2 = 0.059077, and `bit_backend/xor_inplace/words=1` records 0.038850
  against 0.049556. At both cells the ensemble moves the reference arm less than
  half as far as one natural rebuild moved it, which is the condition §6.4 reads
  as the ensemble being tamer at that cell rather than unlucky.
- **The other three preconditions hold at every cell and at the set.** Precision
  clears its three-standard-error margin everywhere, with 4.025 the narrowest;
  the half-split null stays inside ±τ_cell at every cell and reads 1.001451 on
  the geometric mean; decorrelation and the set-level coverage comparison pass.

Per amendment §6.6 the failing precondition is coverage, not precision and not
the half-split null, so **the K = 256 ladder rung is not the remedy this
document prescribes**: "the ensemble is not a model of the layout variation the
record measures, and no larger K repairs that", and the ensemble's axes become
the subject of a tracked amendment before the next measured run. The two failing
cells and their shortfalls are recorded below.

The amendment's central prediction is nonetheless measured and it holds. Nine of
the eleven cells that tripped τ_cell in the second receipt fall inside the
tolerance once the layout draw is averaged over 128 members, including all five
polynomial cells that lie on paths the cutover never touches. What survives the
averaging is the two `bit_backend/popcount` cells, which have now tripped the
per-cell rule in all three post-cutover sessions.

Per plan §7 and control-arm §4.5 this session is run once and stands as taken.
No predeclared value moves for it: not τ_cell, not τ_set, not the pinned set,
not the schema token, not the ensemble, not the margin of three standard errors.

## Post-cutover state

`select_backend_for_size` at `crates/gf2-core/src/kernels/backend.rs:104` is
`#[inline]` and reads the published threshold through
`crate::tuning::active_simd_min_words()` at `:106`; the definition site of the
conservative default it resolves to is the compiled-in `SIMD_MIN_WORDS_DEFAULT`
at `:83`.

Issue `2a85f728` is the work this session measures. The threshold lives in one
`AtomicUsize`, `ACTIVE_SIMD_MIN_WORDS` at `crates/gf2-core/src/tuning/mod.rs:648`,
statically initialised to `SIMD_MIN_WORDS_DEFAULT`. The inlined reader
`active_simd_min_words` at `:659` is one `Relaxed` load and nothing else, so the
selection boundary carries no resolved flag, no acquire ordering and no cold
path. `active` at `:666` and `install` at `:686` publish the threshold inside
the `OnceLock` initialisation, before the installed profile can become
observable. The whole diff in `crates/gf2-core/src/` between the second
receipt's revision `bba06631533e8316ef3a4e317b2ae2971f9825ad` and this
session's is 58 lines in `tuning/mod.rs`.

Behaviour is unchanged at every pinned size: `--self-check` resolves
`simd_min_words=8` in both checkouts and `--list-cells` reproduces the plan's §2
arms cell for cell, so the comparison's per-cell identity precondition holds and
no cell changes arm.

## Reproducible protocol and provenance

Every value below is observed during this run. The two arms share one host
state, one lock, one affinity mask and one wrapper invocation; the rows that
differ between them are marked.

| Item | Value |
|---|---|
| Harness and checker | `crates/gf2-core/benches/selector_non_regression.rs`; schema `selector-non-regression-v1`; full-worktree `git status --porcelain --untracked-files=all` |
| Procedure | [`selector-non-regression-plan-v1.md`](selector-non-regression-plan-v1.md) as amended by [`layout-attribution-verdict-v1.md`](layout-attribution-verdict-v1.md), executed unmodified |
| Ensemble | K = 128 members per amendment §3.1; `--execution j+1` names member `j`; no member failed to build, so §3.5's replacement rule was not exercised |
| Source revision, candidate arm | `46d21d6dcfd85b24fff938606dde73844f12546e`; every row records `source_dirty=false`; HEAD and the full-worktree status are recorded unchanged at the session's start and end |
| Source revision, reference arm | `0c072d73ca65cf50af98b8c4b61ed876f8218df6`, the revision every row of the baseline receipt records; every row records `source_dirty=false` |
| Harness file SHA-256, candidate checkout | `1d5f4414e3f091bc03784c856d7818163e167ac23ad493c6d957019708a218e7` |
| Harness file SHA-256, reference checkout | `ca605cc35adb2047110010905a502d075c40839d69ad8b065d71c3b3667f7f84` |
| Why the two differ | The reference revision predates the `--layout-audit` mode of amendment §6.5, so the two checkouts' copies of the harness differ, exactly as §4 records. That mode is reached only through its flag and runs no measurement; the recorded rows, the timed windows, the pinned set and the protocol constants are the same on both sides, and what the difference produces is placement, which is the quantity this ensemble samples |
| Reference-arm checkout | `.agents/worktrees/control-0c072d73`, detached at `0c072d73ca65cf50af98b8c4b61ed876f8218df6` and clean. Its `gf2-core` release artifacts are removed with `cargo +1.95.0 clean -p gf2-core --release` before its first build, so every one of its 128 binaries is compiled in this session |
| Bench binaries | 256, one per member per arm, each staged out of its target directory to `/tmp/gf2-ens3/` before the next member overwrote it, each hashed. The full member-by-member table is below |
| Toolchain | `rustc 1.95.0 (59807616e 2026-04-14)`, built and run as `cargo +1.95.0`, both arms |
| Features | `simd`, both arms; no tuning profile installed |
| Host | `fraktaali`; AMD Ryzen 9 5900X 12-Core Processor |
| OS/kernel and governor | `Linux 7.1.8-arch1-3 #1 SMP PREEMPT_DYNAMIC Tue, 11 Aug 2026 09:16:08 +0000 x86_64 GNU/Linux`; `powersave` on CPUs 6--11 |
| Lock and affinity | `dev/scripts/ccx1-bench-flock.sh`, one invocation holding `/tmp/gf2-ccx1.lock` for all 256 executions; the lock is unheld at start and acquired without blocking, and `/proc/locks` inside the wrapper records `FLOCK ADVISORY WRITE` by PID 3407587 on `00:2d:39210`, the lock file's device and inode as `stat` reports them (`45:39210`). CPUs 6--11, observed as `Cpus_allowed_list: 6-11` inside the wrapper |
| Niceness | The wrapper's best-effort `nice -n -5` is denied; `nice` reports `cannot set niceness: Permission denied` and the child runs at niceness 5, the niceness of the invoking session, rather than the 0 the two committed post-cutover receipts record. The lock and the affinity remain in force. Both arms run at the same niceness inside one session, so the verdict ratio carries none of it; the deviation is recorded in Deviations below |
| Host quiescence | No `permanent-campaign-runner` process, matched by inspecting `ps -eo pid,comm,args` rather than by a self-matching commandline pattern. The one-minute load average is read down to 0.06 after the build phase and stands at 0.18 as the wrapper is invoked and 1.11 at its end; the five- and fifteen-minute averages are 0.42 and 1.97 at the start and 1.12 and 1.16 at the end. No `cargo`, `rustc` or `nextest` process runs on the host during the timed phase |
| Timed work | One execution of one repetition per build, `--target-ms 250`, 256 executions in total (amendment §5.1) |
| Build window | 2026-08-20 20:56:12--21:56:50 UTC (60 min 38 s), no lock held, no compilation under the lock |
| Measured duration | 2026-08-20 22:12:19--22:58:39 UTC (46 min 20 s), the wrapper's whole child lifetime, covering both arms |
| Raw file, reference arm | [`2026-08-20-ensemble-reference-arm.csv`](2026-08-20-ensemble-reference-arm.csv), SHA-256 `d610eded43af330056e654d5937e20eeaa985173c3645ba30ff5c7fb11b5a59c` |
| Raw file, candidate arm | [`2026-08-20-ensemble-candidate-arm.csv`](2026-08-20-ensemble-candidate-arm.csv), SHA-256 `b2e06da9e9ccba74863a4de975c79d4e8b7709a893c60fb3595b9244fc250dc0` |

Dates in this receipt are UTC. The session runs on the evening of 2026-08-20
UTC, which is the small hours of 2026-08-21 in the host's local `EEST` zone.

Each arm records 4352 rows: thirty-four cells x 128 builds x one repetition.
Every row of the candidate arm carries `source_dirty=false` and revision
`46d21d6dcfd85b24fff938606dde73844f12546e`; every row of the reference arm
carries `source_dirty=false` and revision
`0c072d73ca65cf50af98b8c4b61ed876f8218df6`. Each arm records exactly 34 rows per
execution and execution indices 1 through 128 with none missing. Every cell's
equivalence probe passes in every one of the 256 executions — a failed probe
aborts the run, and the session driver ran to completion reporting 256
executions.

The raw files are written to
`/tmp/gf2-e8fe47f5-ensemble-reference-arm-0c072d73.csv` and
`/tmp/gf2-e8fe47f5-ensemble-candidate-arm-46d21d6d.csv`, both checked absent
beforehand, and copied byte-for-byte into this directory afterwards; `cmp`
reports each copy identical and each pair of paths hashes to the SHA-256 above.
No in-repository file is created while the run is in flight.

## The ensemble

Member `j` takes `A(j) = ⌊j/16⌋`, `B(j) = ⌊(j mod 16)/4⌋` and
`C(j) = ((j mod 4) + A(j) + B(j)) mod 4` per amendment §3.1, omits any option
whose level is 0, and records `--execution j+1`. The 128 members are distinct as
`(A, B, C)` triples, the level sum has the parity of `j` at every member, and the
parity split is balanced on every axis: each half holds eight members at each
level of A and sixteen at each level of B and of C, as §3.2 states.

**No member failed to build**, in either arm, so §3.5's drop-and-replace rule
was not exercised and both arms carry member indices 0 through 127. Build
wall-clock is 13 s to 15 s per binary across both arms, against the 5 s to 16 s
the axis verification receipt captures; the two members recorded at 0 s were
already present in their target directory from the smoke test that preceded the
build phase.

**The 128 members are not 128 distinct binaries.** The candidate arm's members
produce 125 distinct SHA-256 and the reference arm's 127: candidate members 4
and 14, 103 and 106, and 117 and 127 collide, as do reference members 68 and 78.
Different `(A, B, C)` levels can leave a particular binary byte-identical when
the added option changes no padding this program needs. Nothing predeclared
forbids it — §6.5's audit accepts an arm on its member count and the balance of
its parity halves, both of which hold — and the consequence, a slightly smaller
spread of placements than 128 draws would give, is measured rather than assumed:
it is part of the `s_R` the coverage precondition reads.

Binary sizes span 790,040 to 918,296 bytes in the candidate arm and 722,696 to
841,496 bytes in the reference arm, a spread of 16.2 % and 16.4 % from the
smallest member to the largest, consistent with the +15.61 % the axis receipt
measures at its most perturbing member.

**The ensemble re-places the library under test, not only the harness.** Between
candidate members 0 and 1, 808 of the 819 common text symbols sit at different
addresses, and all nineteen symbols carrying a `gf2_core` path move — for
instance `gf2_core::kernels::ops::scalar_xor_inplace` from `0x603e0` to
`0x602e0`. Most of the crate's pinned code is inlined into the bench binary and
so is counted among the remaining moved symbols rather than under a `gf2_core`
name.

### Member provenance

Every member's `RUSTFLAGS` and the SHA-256 of the binary it produced, for both
arms.

| Member `j` | A | B | C | `RUSTFLAGS` | Candidate binary SHA-256 | Reference binary SHA-256 |
|---:|---:|---:|---:|---|---|---|
| 0 | 0 | 0 | 0 | (none) | `a76eb03e09c5e70b58c58653372a897e2a9b703e05584d0b5e7a67e8c3d2f6f4` | `c7be7a87d7a01d3c2e297919033a99237afe8939f3fce147802ecd86df32c710` |
| 1 | 0 | 0 | 1 | `-C llvm-args=-align-all-blocks=1` | `06c3061156f5847618537410d72fd1ff34623d3b625098d4f501fc3b751fb6ae` | `eb6e3d7e77bf6acc86b9454f5d2321257599ffa179a0cd0fad6512727aeddd5f` |
| 2 | 0 | 0 | 2 | `-C llvm-args=-align-all-blocks=2` | `54af8be512b26190c7590fd90409d98e6890550f531ae891d756c4196dd4d722` | `c0e875cbc8a062d818cfc181a3424a0f50ddddccfc0926d0f91033ba2eadba18` |
| 3 | 0 | 0 | 3 | `-C llvm-args=-align-all-blocks=3` | `ca3948f4e93547920e4a205f389fd1f24dbfb62e05386a985ca4d200fad9e3ed` | `b1f7ef2103945639515d6839e237381ba48976a7585efc791132b1106d38a445` |
| 4 | 0 | 1 | 1 | `-C llvm-args=-align-all-nofallthru-blocks=1 -C llvm-args=-align-all-blocks=1` | `313921f8e36be5f17cceb74dff7cd9639b6ffdd498c6833cd16ad4ea5035570b` | `1447042846e3517c94a6f2aa7d3d66c9865cca16c95bcd2e85c601202a852eb2` |
| 5 | 0 | 1 | 2 | `-C llvm-args=-align-all-nofallthru-blocks=1 -C llvm-args=-align-all-blocks=2` | `b1cb10c196cf70de213274a982ba7cc9309e000b4d4c722e57e7a269298138bd` | `8d6e9fc96f2335a23f89c8b10e714873e9a7bc6b27cbbdcbd351525ffb43f5d7` |
| 6 | 0 | 1 | 3 | `-C llvm-args=-align-all-nofallthru-blocks=1 -C llvm-args=-align-all-blocks=3` | `3eb53a8b3a95cd3b398eb5f6e43fb444c777aaec8b0a1a211c1606d454f9043d` | `8ff86203c8b67e049eb6dd7078e97e4de783778dba474928900c91cf5cc54e96` |
| 7 | 0 | 1 | 0 | `-C llvm-args=-align-all-nofallthru-blocks=1` | `0f2aebb1b7858ac471d0174e9d463cdaf76298bc55d05eef98583a26e7032b8d` | `2f5fef2aa994f9235ebc1be6a8c4dae8d30870241c058208fab238bf946c0033` |
| 8 | 0 | 2 | 2 | `-C llvm-args=-align-all-nofallthru-blocks=2 -C llvm-args=-align-all-blocks=2` | `5d4ca9a317b5a25cd691df4518848196c529659033da624fbe069b31de579d9e` | `0908de604e83e09666f3d3f6e981087f47cfd71fcc08217eb3dbeea9542279a1` |
| 9 | 0 | 2 | 3 | `-C llvm-args=-align-all-nofallthru-blocks=2 -C llvm-args=-align-all-blocks=3` | `6211ee7df4ead85500b9d6040f02b818b4ece13cb1d39c65869453bae94014ef` | `680f74ac96c81e489180d55b56cd52029005fa30951f97ec6be3b60b17646c3d` |
| 10 | 0 | 2 | 0 | `-C llvm-args=-align-all-nofallthru-blocks=2` | `f55e19fbf476952ab389386256ccbe4a4e436e94b7a430b78ad8452cfdd28edb` | `6c4eb2f5bf1d06241951a8cb3db222ba1e57ef243a85b67d5641a12bf08d5885` |
| 11 | 0 | 2 | 1 | `-C llvm-args=-align-all-nofallthru-blocks=2 -C llvm-args=-align-all-blocks=1` | `12b3e30a073dec6bd13a01d9388e9dfbfcfc8df68ae42286a05c1bf80f79b793` | `fa3f58cafebdde5050d76ff1905d15fe14d913a1e43dd6865c5c7890be3138c5` |
| 12 | 0 | 3 | 3 | `-C llvm-args=-align-all-nofallthru-blocks=3 -C llvm-args=-align-all-blocks=3` | `77b409f9b7e6935b9f76b087b4f196f5db813ab32a1a90da4f8d623b07ad761f` | `0f2c6a675fe9e91f9f0b5b022964de4a926d817a335427a661d1f7f3df3fae9b` |
| 13 | 0 | 3 | 0 | `-C llvm-args=-align-all-nofallthru-blocks=3` | `a9e241ffb98e63be2174a1def7e73c2b8b674edd4a40b0413e72466c2e233bc3` | `4ddc8f15576e3734444b9f93f460ea90b730c6eb25e36115cbc36b5a6381e35c` |
| 14 | 0 | 3 | 1 | `-C llvm-args=-align-all-nofallthru-blocks=3 -C llvm-args=-align-all-blocks=1` | `313921f8e36be5f17cceb74dff7cd9639b6ffdd498c6833cd16ad4ea5035570b` | `9e38020f7f758bbad1089a08c9ed48da01aef8a8052c4062119b7e9183a51189` |
| 15 | 0 | 3 | 2 | `-C llvm-args=-align-all-nofallthru-blocks=3 -C llvm-args=-align-all-blocks=2` | `11ba6d4dfae5bad53bd862817c0ab48a35d978cc5c9f8c9f529c93001d8e627a` | `8887a8662a60d7393c9869e8447ccf786ae224a33db22badd4ef28de361d118b` |
| 16 | 1 | 0 | 1 | `-C llvm-args=-align-all-functions=1 -C llvm-args=-align-all-blocks=1` | `2ed39f9670b252d22d9b9583fbcc5b94b3abaeec5743ba8717bd2c2e39bc15ac` | `fddc3c3217cb547fd6e2542a3df3558d1821395be45e46bcdd36765ef4f1137e` |
| 17 | 1 | 0 | 2 | `-C llvm-args=-align-all-functions=1 -C llvm-args=-align-all-blocks=2` | `4f72113ec487ed789218b568a924951a4a5ed41e4ad24e6ee1a2be9883dfb544` | `dfa8c36a071652e8e7c746d010b1604f9330483f579ef8b03dde91a54b12c5f7` |
| 18 | 1 | 0 | 3 | `-C llvm-args=-align-all-functions=1 -C llvm-args=-align-all-blocks=3` | `bbb3dfebfce7975d18eee06a3417e33203bd2d47bf473aa4128cae153ae7ddc2` | `e393e3b519fd0c2d153888ee25277587f5a4fd0474bcb00b944d235331af4d42` |
| 19 | 1 | 0 | 0 | `-C llvm-args=-align-all-functions=1` | `eef12b35cf04715c6c8ce5fe0e8434240e5c0ee4667fe1ea91fab64986ef442b` | `1b11ecd870fcec8f40bcf1ac22f65730538fa29d229989714a116b47fde296d2` |
| 20 | 1 | 1 | 2 | `-C llvm-args=-align-all-functions=1 -C llvm-args=-align-all-nofallthru-blocks=1 -C llvm-args=-align-all-blocks=2` | `015722c1e26e8d783028015dffa1a6f40ed2ad55445768a80557ca453c0b98b7` | `083036f2b36b5dd7db8daa3ac39b8eb3e427226d59ea21e3f771b378ae200fb7` |
| 21 | 1 | 1 | 3 | `-C llvm-args=-align-all-functions=1 -C llvm-args=-align-all-nofallthru-blocks=1 -C llvm-args=-align-all-blocks=3` | `3ed951bb45a9b89f9510180300fe9dd787d84849bc97f86d06b69d11fd22a78a` | `917e96512ffe450fe04bced80123c762f66487f13259012684b548b554b2c852` |
| 22 | 1 | 1 | 0 | `-C llvm-args=-align-all-functions=1 -C llvm-args=-align-all-nofallthru-blocks=1` | `d04626ecf9ac298714675754ac3019b759fd00210663d7d240906a71f6076102` | `e55d6b259dcfab7e4f9a1f5ae9bcba89356211f7e96f16cd8d49d731f1654f41` |
| 23 | 1 | 1 | 1 | `-C llvm-args=-align-all-functions=1 -C llvm-args=-align-all-nofallthru-blocks=1 -C llvm-args=-align-all-blocks=1` | `2ff32121b2b77a846cd3dda534b4699f8afe5aca5965ad7a704d930aeafe978d` | `2c0e9fe170cec72baa1b83dded3b45577f9f3ce2e655e30a0a8b6e5a0d68be2c` |
| 24 | 1 | 2 | 3 | `-C llvm-args=-align-all-functions=1 -C llvm-args=-align-all-nofallthru-blocks=2 -C llvm-args=-align-all-blocks=3` | `beb1eb59796fc88f13077651b36795dc01c9965c83d2d5eda9a164ea2c5b6fdf` | `e1ae6923a20ab10ff5b2eb12ac468b816d7ba09788bb5eeacd7fc575ad4256da` |
| 25 | 1 | 2 | 0 | `-C llvm-args=-align-all-functions=1 -C llvm-args=-align-all-nofallthru-blocks=2` | `e8c4cc7b0a5b8a11c22ab110b4546d7a953a208f367a41f819e1ae8c3835a6c0` | `015b4bb7eee0ba2ef07d792ab8ab91aee9d63885e463a7fbb0eb6917848fe416` |
| 26 | 1 | 2 | 1 | `-C llvm-args=-align-all-functions=1 -C llvm-args=-align-all-nofallthru-blocks=2 -C llvm-args=-align-all-blocks=1` | `5b3ccf044459eaa70efbd7e158c29ab8ae8385a46dabce356e1da859e0342a5a` | `7b299331ac558392fcd39a2139fc4c51743703fd148f5751d84bc4ae8ef6c55b` |
| 27 | 1 | 2 | 2 | `-C llvm-args=-align-all-functions=1 -C llvm-args=-align-all-nofallthru-blocks=2 -C llvm-args=-align-all-blocks=2` | `a6fa5a95db814631db7a335aa4bf5d89bca1eed914a6ae6ec90d58a211e5a3d5` | `4f191b2b0edf6ac1afcaa629c237fa50a33fafbbc449a2c1a786e04552eb3c15` |
| 28 | 1 | 3 | 0 | `-C llvm-args=-align-all-functions=1 -C llvm-args=-align-all-nofallthru-blocks=3` | `da6a2ef6c16eeff68dbe89763485772096f32068d51bf85257f0c59354c4c168` | `020dfbd73c12bfee4f454375e8daae3faaa98cccec1763f8420b56a75690251c` |
| 29 | 1 | 3 | 1 | `-C llvm-args=-align-all-functions=1 -C llvm-args=-align-all-nofallthru-blocks=3 -C llvm-args=-align-all-blocks=1` | `a86837c774220ae054bde32778fff301d3d214f89ae0e7b15da7a0dc55a0f245` | `b99946260eab22afc9e1104db968b6e9e66a6cdb9a25353af20c39119599daf3` |
| 30 | 1 | 3 | 2 | `-C llvm-args=-align-all-functions=1 -C llvm-args=-align-all-nofallthru-blocks=3 -C llvm-args=-align-all-blocks=2` | `521edab045573cd38feed64bd0759d84af55dc92ee10f7a8f2d8a21b1a151f53` | `a91cd46799e33dc65c0f9d1f23bab869e3e1942d69d5b1d40109c97ed2245beb` |
| 31 | 1 | 3 | 3 | `-C llvm-args=-align-all-functions=1 -C llvm-args=-align-all-nofallthru-blocks=3 -C llvm-args=-align-all-blocks=3` | `9b35a5a3442234bd0be5204c22be89a6d297f902c926dce46847b4ac16c11107` | `8cefd16d6c13ff7ac238ab817758f2bbaa76fc9fef646d054900ec557045c29c` |
| 32 | 2 | 0 | 2 | `-C llvm-args=-align-all-functions=2 -C llvm-args=-align-all-blocks=2` | `db6aa05016173bbedfc9f0ecc0ac82b0ab3b9afa834fb2617bdd782662e80449` | `cfe632bef0be85f08b693ecbcc9a801ee3e42fc1d212a254bb8ead84f7493855` |
| 33 | 2 | 0 | 3 | `-C llvm-args=-align-all-functions=2 -C llvm-args=-align-all-blocks=3` | `cc483dc5e649f78cc635d37d601575e0e0fdcb7251b76bf9aa6ef849e36b9c79` | `8d2152388b495b7eb5f417fb2a250748653bfc1b5e3e0c379975036433821d98` |
| 34 | 2 | 0 | 0 | `-C llvm-args=-align-all-functions=2` | `433b6d6d87c165c5a3507768728dcf7ebde39db379b40476383743f64901f314` | `01889042e36289860da33a66272ec6ecdeefb2512a5cda53537e06446e214e11` |
| 35 | 2 | 0 | 1 | `-C llvm-args=-align-all-functions=2 -C llvm-args=-align-all-blocks=1` | `b8d9af92163b80691774ff14b76679f6c6036e63f8ca9c991d8429875bc3a716` | `c6b07d8edcb7ecb36fc2c757f6e6fe28aa569fd9ee291fa95f3c4ef9e0cb4943` |
| 36 | 2 | 1 | 3 | `-C llvm-args=-align-all-functions=2 -C llvm-args=-align-all-nofallthru-blocks=1 -C llvm-args=-align-all-blocks=3` | `be5f25c9da02621c403dbb91520fe65ff5a2ab897a7a34a8445e7f8509dcdede` | `47cf832e463d7d8d318bc9bc016996325825399d4d1cc75ded98cc4ab5bb0ff1` |
| 37 | 2 | 1 | 0 | `-C llvm-args=-align-all-functions=2 -C llvm-args=-align-all-nofallthru-blocks=1` | `63c5f96c4d531c615f2ea0ceb05a30bc03aaed84ee9041a274af58ae778467de` | `faaf51f2a98a85eccc100ae4bd2b662127e9cd58535a4c8d7eea866ad36ce56d` |
| 38 | 2 | 1 | 1 | `-C llvm-args=-align-all-functions=2 -C llvm-args=-align-all-nofallthru-blocks=1 -C llvm-args=-align-all-blocks=1` | `665deebc15c8446518292d52843b85941793ffbccc0f93ba130ae8d46cee146b` | `55ef4702617a5e8ada55e1bfda77f677210ff30356960de3398908acbe488647` |
| 39 | 2 | 1 | 2 | `-C llvm-args=-align-all-functions=2 -C llvm-args=-align-all-nofallthru-blocks=1 -C llvm-args=-align-all-blocks=2` | `7cfd2fa0f2e6def4c0ef684787a3529052de7cb2b7ec0baf850d2fc2978231ac` | `e7e23d0ea81fcdd26aeee23bcd7c2260dc518b26f20be42741ecdbda59db59e8` |
| 40 | 2 | 2 | 0 | `-C llvm-args=-align-all-functions=2 -C llvm-args=-align-all-nofallthru-blocks=2` | `96a5de3de7b7103e6c520f44a661a3b9ea10feb95360de8d8b88250f83ebabf3` | `cd79a1a0734feae95dcecb9606332be3be6738818c70a5e882a340f71e1a81c1` |
| 41 | 2 | 2 | 1 | `-C llvm-args=-align-all-functions=2 -C llvm-args=-align-all-nofallthru-blocks=2 -C llvm-args=-align-all-blocks=1` | `1a225034f6c2526503b9781589d3804b445aebd343747889e517ac2ce8389537` | `6c310ecd821552535a8af51e6031d5828efee7fd193fe4fc035b71380e5c69f3` |
| 42 | 2 | 2 | 2 | `-C llvm-args=-align-all-functions=2 -C llvm-args=-align-all-nofallthru-blocks=2 -C llvm-args=-align-all-blocks=2` | `89f8375ff2e57337a82c49f4a905c389e9f1354e5483487f8db52164fb3173db` | `ef647c363b42a96f72b3ab295f991284a8e8ae00073e40b880e728cf8dee030a` |
| 43 | 2 | 2 | 3 | `-C llvm-args=-align-all-functions=2 -C llvm-args=-align-all-nofallthru-blocks=2 -C llvm-args=-align-all-blocks=3` | `0b1ccac2ef2bb9cf6d4e1148b548f55a20d49384088b67e62c7e51bb9e3b6912` | `db3c90673f62bcb1a5963be6dca52f2779c25b8645a34c7eab85c6e2d5537add` |
| 44 | 2 | 3 | 1 | `-C llvm-args=-align-all-functions=2 -C llvm-args=-align-all-nofallthru-blocks=3 -C llvm-args=-align-all-blocks=1` | `9599a4bb1a91616e953da87a0eeb70653f79d1b9517a74acd5a8a007a1a3e50a` | `e73d128c2c64f28d92d38bce563dd0a0229505bab962bdd1f232406f645a9888` |
| 45 | 2 | 3 | 2 | `-C llvm-args=-align-all-functions=2 -C llvm-args=-align-all-nofallthru-blocks=3 -C llvm-args=-align-all-blocks=2` | `ce99a2d094219399e5dbf97fbb9d2380637b206a472df8c5c8939169321e0882` | `f1a83d0c8c54095052c2d7cf0d0d034cb5498068c40ec74e83cbc40a747c4a35` |
| 46 | 2 | 3 | 3 | `-C llvm-args=-align-all-functions=2 -C llvm-args=-align-all-nofallthru-blocks=3 -C llvm-args=-align-all-blocks=3` | `a9c859c5af57e6b6ff3b06a593f69371e14ca54c2fe5df133ad5e76ae971a673` | `a414740d4f014dae5cf56f5b2dcead18e99a7176fb15798c60a06adec39a6ce6` |
| 47 | 2 | 3 | 0 | `-C llvm-args=-align-all-functions=2 -C llvm-args=-align-all-nofallthru-blocks=3` | `92eed7e8c5ce76806b04ae6358ad258207267cfad1afbfe7dbfb8c5f8686d209` | `52a9f27ac9d4351bab583f212d42072fa4973d082caed15f6516fddda1919e1a` |
| 48 | 3 | 0 | 3 | `-C llvm-args=-align-all-functions=3 -C llvm-args=-align-all-blocks=3` | `6aee820020cea4979ff0d9bc7ce5f3aa7c692da8ead25fd84e334db3db2e7044` | `f2cb78ffdf8b13f7811a0e6df55fbf6568fe082b2eaf7cfa06e748b8177f6416` |
| 49 | 3 | 0 | 0 | `-C llvm-args=-align-all-functions=3` | `9ebc9416c60dc39af3196dc6ee1ac5afc69e79bff1309e7257fc3d3b9021c301` | `69516b7ab45f8ee5594f55e1c69685c2c63cebc210ee52466c2dc6498aea87e7` |
| 50 | 3 | 0 | 1 | `-C llvm-args=-align-all-functions=3 -C llvm-args=-align-all-blocks=1` | `bbdae5f0f3e668bbe74f27df9f8430e4f1a4cda443e23183669e2116bfe29e1d` | `940387c2aab719ec071491d1ecd6b3f3495939ff6e8a79c6742c25fd723b0aee` |
| 51 | 3 | 0 | 2 | `-C llvm-args=-align-all-functions=3 -C llvm-args=-align-all-blocks=2` | `dc8dbb180154ef776a83e9fdc7550f15f4355ccbf0e68f5b41ce2bce5e7fb91c` | `5b412ae38c7a0d6c235a93543b5531630d85c81768041366ba69c3e2841ae968` |
| 52 | 3 | 1 | 0 | `-C llvm-args=-align-all-functions=3 -C llvm-args=-align-all-nofallthru-blocks=1` | `932087b5ca9f011cfcda2f9d9f5d8a896c6cd2cce28581f8cc6a14049b913155` | `5e97886dd210b8631c1621f9f06d06def4fc4314eb8e1c006225d987f8008472` |
| 53 | 3 | 1 | 1 | `-C llvm-args=-align-all-functions=3 -C llvm-args=-align-all-nofallthru-blocks=1 -C llvm-args=-align-all-blocks=1` | `a15e2d3c96826dc1b336ee05c2324c909deefcf8aa91ac963bb0c0b50d503904` | `28e20cd93a3978869a6330847effed65b7b68399fe62583e941f100f5b04d66b` |
| 54 | 3 | 1 | 2 | `-C llvm-args=-align-all-functions=3 -C llvm-args=-align-all-nofallthru-blocks=1 -C llvm-args=-align-all-blocks=2` | `99f2c37b1cc6451794460049d99e3343fd18d967f9087618879b2f85f873430e` | `4b7d4c3871dc388054d7a3305c2d177fd581b3e4c4e5f633ddafba6b2d3a4b61` |
| 55 | 3 | 1 | 3 | `-C llvm-args=-align-all-functions=3 -C llvm-args=-align-all-nofallthru-blocks=1 -C llvm-args=-align-all-blocks=3` | `b3513b3613a578be3252ae55686ea2fdcd8bff75d7ae3b832652b918b1297336` | `6c92f429f5ab76dc63b62f620627139b97c7e5d13bca15173788697ebabbe3c7` |
| 56 | 3 | 2 | 1 | `-C llvm-args=-align-all-functions=3 -C llvm-args=-align-all-nofallthru-blocks=2 -C llvm-args=-align-all-blocks=1` | `e25dca5417867b9d6dc76090962c7852d404d1979c9ef82b11732bcf7cef0354` | `afec13ddb2328ac444be9f9df3aae31af7d3b929954f916c95599a1c652e4227` |
| 57 | 3 | 2 | 2 | `-C llvm-args=-align-all-functions=3 -C llvm-args=-align-all-nofallthru-blocks=2 -C llvm-args=-align-all-blocks=2` | `95206ed14fdb8c45e5e4a795fe894859757486f83ecc9b6ef7091a2b0bb9366c` | `0400b72f9bbdc15cf0184c2ece30be19e15a82684d0605d463e57ad9bc7a82f6` |
| 58 | 3 | 2 | 3 | `-C llvm-args=-align-all-functions=3 -C llvm-args=-align-all-nofallthru-blocks=2 -C llvm-args=-align-all-blocks=3` | `a6cb61ba9146f7d380255abc8b9af1514e95505f72e18b87919bd21a80efba0f` | `97367e9ecffeca67d375b1dc7c4b653e2d6d72b4324ef75e38465f319a8138fa` |
| 59 | 3 | 2 | 0 | `-C llvm-args=-align-all-functions=3 -C llvm-args=-align-all-nofallthru-blocks=2` | `734cb04d3348463ba0bfe62d1ce8da906ea0efc1c9957077a179d20e60c04a45` | `a6912cfbef735d8260fb892e2c0d6dcd7901add043c7a3709bc26a488af48078` |
| 60 | 3 | 3 | 2 | `-C llvm-args=-align-all-functions=3 -C llvm-args=-align-all-nofallthru-blocks=3 -C llvm-args=-align-all-blocks=2` | `e0370f990f421433ff8bff455674de7e65061b707e8b318bd14fd83cb5c1aeda` | `4933c55b8b5ab9ad0fe2c7f7d93e6ed415ffa826cc6c5131a057e609d82ef737` |
| 61 | 3 | 3 | 3 | `-C llvm-args=-align-all-functions=3 -C llvm-args=-align-all-nofallthru-blocks=3 -C llvm-args=-align-all-blocks=3` | `9c908741024a4b93305b4b7275d73128afce203959614118771d0a97ed939f18` | `5ecd9f6df0c75c404eb89fb287f4b61df8172bc993983a882c2fb4ab378d16b4` |
| 62 | 3 | 3 | 0 | `-C llvm-args=-align-all-functions=3 -C llvm-args=-align-all-nofallthru-blocks=3` | `29bcce8a0e8fcb4610188ad62c649823daa25b6d2c08bb775a91ffa1cb556d66` | `30cb01101f8f6b5fb87c0ed9328899b783ae4c8ce437915c42d934c815a6d177` |
| 63 | 3 | 3 | 1 | `-C llvm-args=-align-all-functions=3 -C llvm-args=-align-all-nofallthru-blocks=3 -C llvm-args=-align-all-blocks=1` | `e3021e5e8ce61be7ffcb9fb4ccba3b03f9acda08959541a6d0306551fc18dcaf` | `d1e41436fb192c4c8b7256ec8f162a59581f76d75e392aa8f0d3d328e1004923` |
| 64 | 4 | 0 | 0 | `-C llvm-args=-align-all-functions=4` | `b6541230686fd5f6052e025b6641ac5ae037074eb5f2bbb2ec568d3acaed402c` | `5603cc9fd18f71f4f5233b646031ac3fd3a6780b13970f94efd953cf7c01da94` |
| 65 | 4 | 0 | 1 | `-C llvm-args=-align-all-functions=4 -C llvm-args=-align-all-blocks=1` | `641ede164502caba6c3ff8bb31f6cc0cbbd2e44ef7b4e27430a18fe4faef7c0b` | `7937014a1cf2515a4842ce0c4cd7223c7f32c058a10bb20d5cb0fd985c4f3d5d` |
| 66 | 4 | 0 | 2 | `-C llvm-args=-align-all-functions=4 -C llvm-args=-align-all-blocks=2` | `094353c2a2a333c36ab2601698c3f553b7e0816c3de6d8131f18f060ad81e3e2` | `ce492fcd0ad6cc7b0a2df9241561af9cdd611dcddfed75d3ef6dd221dbbb0bd8` |
| 67 | 4 | 0 | 3 | `-C llvm-args=-align-all-functions=4 -C llvm-args=-align-all-blocks=3` | `98c6c62bb65afbddf64b641630383f3d9dbcef64c2afe462a69dcc009facc8ba` | `69fdaf340e0db6cea8596e968669add59a3774ef5e913b454f3d239afe39987a` |
| 68 | 4 | 1 | 1 | `-C llvm-args=-align-all-functions=4 -C llvm-args=-align-all-nofallthru-blocks=1 -C llvm-args=-align-all-blocks=1` | `813190481a369c6246c90cfc551d0b37eb945fc1d087bc43d414407c194b8a7c` | `ea339d978a0c1826d3d208f64199bbc0d53a75e0b7e143474cde9127a76df62f` |
| 69 | 4 | 1 | 2 | `-C llvm-args=-align-all-functions=4 -C llvm-args=-align-all-nofallthru-blocks=1 -C llvm-args=-align-all-blocks=2` | `efc91aa9e5a5568112d86077a22db375e80d5cbc6abc953978b210f5db9f21b8` | `1ebe3f26c80903d062ad3586331a079f0ba556a94894b59ffdaacb2184b4ff08` |
| 70 | 4 | 1 | 3 | `-C llvm-args=-align-all-functions=4 -C llvm-args=-align-all-nofallthru-blocks=1 -C llvm-args=-align-all-blocks=3` | `c71b41f61340640679c74aa66cd29fac6bbcbfb95d19458e6e0b6dbe41e7bfa2` | `3491939da09298ca42e9e37accb9da2cd529ca9352be641a03aad833b2fccfaf` |
| 71 | 4 | 1 | 0 | `-C llvm-args=-align-all-functions=4 -C llvm-args=-align-all-nofallthru-blocks=1` | `a012ceecaed02c414ed499763256016763ca48fed8cb9921764b374315b17dfe` | `359bd4d5da799c34e1f33eb4eacbcb2d2ab16e40d33d08717622ee20b4fb94c7` |
| 72 | 4 | 2 | 2 | `-C llvm-args=-align-all-functions=4 -C llvm-args=-align-all-nofallthru-blocks=2 -C llvm-args=-align-all-blocks=2` | `c190e79cfd7e422d6f78ee36aa31a53e283f751812813474ce3a0dc26bff4547` | `63214c00314e724f85a6c07777df215d0e4b1254904bfb52ba989f85dbc0df62` |
| 73 | 4 | 2 | 3 | `-C llvm-args=-align-all-functions=4 -C llvm-args=-align-all-nofallthru-blocks=2 -C llvm-args=-align-all-blocks=3` | `f20af2455506c9c73f03a818d5de9f11739e0cdae0b1be8209012248aec6304c` | `6038a605e0e400d9a268f955d6c6365f851b262218cf91802560cc7a1bdd8dc9` |
| 74 | 4 | 2 | 0 | `-C llvm-args=-align-all-functions=4 -C llvm-args=-align-all-nofallthru-blocks=2` | `24ee86b282a0aeb26491bcbd24db712ff748f88318409c3a87e2d0c512fc22a0` | `5c7661327eb17c581060b3a0f1822f4db4d8d8f251e130016300f9017aa7e87e` |
| 75 | 4 | 2 | 1 | `-C llvm-args=-align-all-functions=4 -C llvm-args=-align-all-nofallthru-blocks=2 -C llvm-args=-align-all-blocks=1` | `267f319fe5bc400502d1a0004c1eacea45f09db75e23ec4b468aae46f9de9333` | `b641cae1020ad399800fa91bd9772a56daef15b46dba014b0a24644e3c671b14` |
| 76 | 4 | 3 | 3 | `-C llvm-args=-align-all-functions=4 -C llvm-args=-align-all-nofallthru-blocks=3 -C llvm-args=-align-all-blocks=3` | `1f7476da2c53f279ffa64dc1a907726ddc3cd410ed28a55e63e7af92411c6238` | `f81d6bb89ef4a04684aa6bf29f6f55fe37e2446a568be3934c52c47fc9ac2c5e` |
| 77 | 4 | 3 | 0 | `-C llvm-args=-align-all-functions=4 -C llvm-args=-align-all-nofallthru-blocks=3` | `1425fb6990cc9ec2e47d7de7211cbe46a9b38bd9453720b68ba17e107ea4f837` | `9cbec03f9c687f9c0386924423e17901db7cd37ca3805ceaadbf22560a55f0d0` |
| 78 | 4 | 3 | 1 | `-C llvm-args=-align-all-functions=4 -C llvm-args=-align-all-nofallthru-blocks=3 -C llvm-args=-align-all-blocks=1` | `ac08bc853919bbf59b73693ab6fbdf5e03132473f2bb662166a1759d8799a587` | `ea339d978a0c1826d3d208f64199bbc0d53a75e0b7e143474cde9127a76df62f` |
| 79 | 4 | 3 | 2 | `-C llvm-args=-align-all-functions=4 -C llvm-args=-align-all-nofallthru-blocks=3 -C llvm-args=-align-all-blocks=2` | `1cb9910762919acb38cffd33ad7f8a526fad8edc0057ca3e2e35fe3342a1b0f2` | `c50190e7b4fd99aafe74fa74843e3fd6871a40ae60004ec1f992ed7f7dd24490` |
| 80 | 5 | 0 | 1 | `-C llvm-args=-align-all-functions=5 -C llvm-args=-align-all-blocks=1` | `dd5d401ac87264ef4f928de95bbf082e0e5535e067161fc5d02f8d06416a891c` | `c5135e1efbf8dfd9d7cd4ce4a46a8a6ae7c153a10d83fe0dc0e67f3373087d8f` |
| 81 | 5 | 0 | 2 | `-C llvm-args=-align-all-functions=5 -C llvm-args=-align-all-blocks=2` | `f41f75c9c6f3fcff6c7a8fcdd94ab3f41485749fc5a6bb352753c3d2eb5e8a58` | `3c7d27a057e76996a443c38bf7974f421cfa1bb84a8f1be2ce27b0d89a9e5496` |
| 82 | 5 | 0 | 3 | `-C llvm-args=-align-all-functions=5 -C llvm-args=-align-all-blocks=3` | `da1fdccf10801da72566cb2fabafe9ecbe15f2d551d0fb9443eb911aea9b8cdb` | `0ba7c1b2237ddc3cfa69ea5309b87e869b0c6ed4db710d9f69c8a5670ebbfcb1` |
| 83 | 5 | 0 | 0 | `-C llvm-args=-align-all-functions=5` | `2547954f5e277c6bf37616853777826fcf4aa0f9fee3a071d9ec9271f6aaeccf` | `271daf67dc6d06013f4e2dcd19bbbf761b14c612131cd6b4d068a5ab6a417182` |
| 84 | 5 | 1 | 2 | `-C llvm-args=-align-all-functions=5 -C llvm-args=-align-all-nofallthru-blocks=1 -C llvm-args=-align-all-blocks=2` | `a3532f20d03db08952d4d5cc76806a4af257a389f4e47a51daae0c40ff2c2ea3` | `2609831c83653f3995df667fac0d8fa0ce4ec0cb45e2bdf42a21728b62bb3afd` |
| 85 | 5 | 1 | 3 | `-C llvm-args=-align-all-functions=5 -C llvm-args=-align-all-nofallthru-blocks=1 -C llvm-args=-align-all-blocks=3` | `9216499a712cb49a7c32ca01930124b07b90fbe3d6221c01d2686e884ec8d03c` | `148a58b6507f645d0ef79bdec87352a2d087b1e37be0de1956b5250b731c6b5f` |
| 86 | 5 | 1 | 0 | `-C llvm-args=-align-all-functions=5 -C llvm-args=-align-all-nofallthru-blocks=1` | `dbee27995c3cb5f500b82fe3d87e34dc0368007ca21802c1725d63307c62beaa` | `49c0f2dc71c04112d768bf5c63e386be629c7d219102ce62dd7df7218dfb7f87` |
| 87 | 5 | 1 | 1 | `-C llvm-args=-align-all-functions=5 -C llvm-args=-align-all-nofallthru-blocks=1 -C llvm-args=-align-all-blocks=1` | `c41add0f95be61f1cc29286bf367907056810c8ef56fecf140e6f5a8686609a2` | `e1cbf6de8f55dd3ec651ac3679bcdcf519d0b8c84041acec4ebbc87b2c5881e6` |
| 88 | 5 | 2 | 3 | `-C llvm-args=-align-all-functions=5 -C llvm-args=-align-all-nofallthru-blocks=2 -C llvm-args=-align-all-blocks=3` | `22094bc7b68071960a0bae1eaf46f0906423113a7547f5297016d5cccbb49d36` | `b2ef6aa44bf3536b707df55875f74741c0aaf77fc772ee1e33b447cb4e2f8022` |
| 89 | 5 | 2 | 0 | `-C llvm-args=-align-all-functions=5 -C llvm-args=-align-all-nofallthru-blocks=2` | `05ca0a56c6d612d5b6f0ca7bfbfdf0ea13b84d69ece40bd52e39e467f68c3802` | `83a412527fc5429b5cb303c49fe3c82cde8f5aecd2605d2c11001cdfdb3f381b` |
| 90 | 5 | 2 | 1 | `-C llvm-args=-align-all-functions=5 -C llvm-args=-align-all-nofallthru-blocks=2 -C llvm-args=-align-all-blocks=1` | `19c0221513035b2f2b191ec10b87abe0385f24ad547f54d200ebc73ec947e92f` | `8edb80592b802403ee84264db20f4d041860f606e9f436a51372057fef8856a5` |
| 91 | 5 | 2 | 2 | `-C llvm-args=-align-all-functions=5 -C llvm-args=-align-all-nofallthru-blocks=2 -C llvm-args=-align-all-blocks=2` | `2e6b97d06dd410b6febd5691f63de09bde4ed0ae73ad3225d2963bb0269dca04` | `c3a4d4aaad7ee7f65903796e422653242cfc8ca61e10318fbbc53285ad88dc24` |
| 92 | 5 | 3 | 0 | `-C llvm-args=-align-all-functions=5 -C llvm-args=-align-all-nofallthru-blocks=3` | `1c8d9c77cb15d125666b24b1d622dc64bbc5c00e4e464d3706eb60d5268d0c45` | `bf2a285fd9f15ed6f618688d04231d4eb090b2c1557bd0e928339474798088df` |
| 93 | 5 | 3 | 1 | `-C llvm-args=-align-all-functions=5 -C llvm-args=-align-all-nofallthru-blocks=3 -C llvm-args=-align-all-blocks=1` | `a95d7a0041a4fcdf26112431c405e0171a5926337d38f3ca82073d8546a69fa9` | `8c62a6d1345cbbcf126eb4dfa63b13b05d0420a123d9bc12cf50e03a1cffee5a` |
| 94 | 5 | 3 | 2 | `-C llvm-args=-align-all-functions=5 -C llvm-args=-align-all-nofallthru-blocks=3 -C llvm-args=-align-all-blocks=2` | `c73a58f03575eb1fa2663e128babbffdb914d771eace4467c7fc94f05653855b` | `5478c39439cba2341dc94b189dc9d004e368862c468f26e4403994b925743e52` |
| 95 | 5 | 3 | 3 | `-C llvm-args=-align-all-functions=5 -C llvm-args=-align-all-nofallthru-blocks=3 -C llvm-args=-align-all-blocks=3` | `2aa3172a65d6d6f68089607d64bc6e3806b31edac0876a64c38274cd8a43cf0f` | `d9eb7417bc3c88ae2aef602911c53f3f480c76851852207c3f9f73bbdfe9ac99` |
| 96 | 6 | 0 | 2 | `-C llvm-args=-align-all-functions=6 -C llvm-args=-align-all-blocks=2` | `042c1fc03e62215f35dd2b8349731dab430c83cf2f1513704b11a48d0f13eb7a` | `1fe20c3f06104d3ee3636c27c20320b589362b8803ab1bda367724bc4a29a4c9` |
| 97 | 6 | 0 | 3 | `-C llvm-args=-align-all-functions=6 -C llvm-args=-align-all-blocks=3` | `03fad2295520a9f5c3106fcb766b971bfce26d5e69a2a7f2ded6ed6ed7ce67b3` | `804f063300ed88e29474b6284448dbd7504ede10899772b0a4823045f9ec07a4` |
| 98 | 6 | 0 | 0 | `-C llvm-args=-align-all-functions=6` | `c762fc935cecbf38034013e72016c3242811858634770e6b65667aae86cb0108` | `46075adbbf2ad142c53de8b092e6871b400dfb1a927304e6e5eddea1731773e8` |
| 99 | 6 | 0 | 1 | `-C llvm-args=-align-all-functions=6 -C llvm-args=-align-all-blocks=1` | `310ed3fe3f7d1edd71f35cb475a3568b448ef0e88d7aac76d7e506ab83931ffe` | `33ea01f5e76a8191fbdeb9ffd2ce7937969ec80a4717b5e090169542bf734e07` |
| 100 | 6 | 1 | 3 | `-C llvm-args=-align-all-functions=6 -C llvm-args=-align-all-nofallthru-blocks=1 -C llvm-args=-align-all-blocks=3` | `2a2c8e472e2db3c8c3cd663542a5405354e0cfd3dcc62fc4db3291dff04e62e2` | `56260a3184bbaa860ad21882fa64b99b0cb8dc26fd1dbacbef2ba3a7cc5099b5` |
| 101 | 6 | 1 | 0 | `-C llvm-args=-align-all-functions=6 -C llvm-args=-align-all-nofallthru-blocks=1` | `ec7dfd9037745a4c15acaf80056f159dcc5a23ccf09b6ad341d91e48aa999893` | `b4130756eac4a79efcabd5233d5f4b572bd4dcc100e0a2a5304bd9e7ad5733d6` |
| 102 | 6 | 1 | 1 | `-C llvm-args=-align-all-functions=6 -C llvm-args=-align-all-nofallthru-blocks=1 -C llvm-args=-align-all-blocks=1` | `e6260fcff463f547f5b77092798ab5a68c595bc3d303019a2a93a4ed7e38b83d` | `d882adbeadc18117e3ebc2ea59e63bd0102309f4688eae1a03fd9c7c17446df1` |
| 103 | 6 | 1 | 2 | `-C llvm-args=-align-all-functions=6 -C llvm-args=-align-all-nofallthru-blocks=1 -C llvm-args=-align-all-blocks=2` | `39fd0de76ff471e35333fc92149e144e9f8634f90bfbef998e4a27af784a8c91` | `a0e0c279d6c4c072dcb6432ae9509e85927509819d2eea19e4642d32b38f62a2` |
| 104 | 6 | 2 | 0 | `-C llvm-args=-align-all-functions=6 -C llvm-args=-align-all-nofallthru-blocks=2` | `5cdd9709b77647a62c473790d069c5890fb052d3efdf6bc40dd858eca7a2308e` | `964e140eca1660638d95d69773f27401d28cba742fba46a48a36c7641a2065e3` |
| 105 | 6 | 2 | 1 | `-C llvm-args=-align-all-functions=6 -C llvm-args=-align-all-nofallthru-blocks=2 -C llvm-args=-align-all-blocks=1` | `e25b753813822607f8bcc9c3e3b0d3112b1ac6682f9a245210972851d5e5d9aa` | `db38bc41e297e1fa6410a37d7d2ee9b461921be6df967a07c884f9624beffc44` |
| 106 | 6 | 2 | 2 | `-C llvm-args=-align-all-functions=6 -C llvm-args=-align-all-nofallthru-blocks=2 -C llvm-args=-align-all-blocks=2` | `39fd0de76ff471e35333fc92149e144e9f8634f90bfbef998e4a27af784a8c91` | `d05f1d51e909dc4ee701c5d152aee5eac703edc80331934c8ee3e358aa847d89` |
| 107 | 6 | 2 | 3 | `-C llvm-args=-align-all-functions=6 -C llvm-args=-align-all-nofallthru-blocks=2 -C llvm-args=-align-all-blocks=3` | `d4335a12c1de739aa9ba54317113d467f83e6e54d2e01783deee3fad1819e749` | `a9290908796c3af58872a368bb6d0962182ab143152e8d209d53365a9db8ace2` |
| 108 | 6 | 3 | 1 | `-C llvm-args=-align-all-functions=6 -C llvm-args=-align-all-nofallthru-blocks=3 -C llvm-args=-align-all-blocks=1` | `4bf960322d6ae39b55880b8352695874db8ca7a27a642d61122d82399d751bf8` | `d44752a23799dbbd3254507dbdf05a66d66196d49f4d23c717efba3aa7217238` |
| 109 | 6 | 3 | 2 | `-C llvm-args=-align-all-functions=6 -C llvm-args=-align-all-nofallthru-blocks=3 -C llvm-args=-align-all-blocks=2` | `eb234c97a4beccdf6dded638b5b78e87d712a52c1184f1d637b6542a2fb9b61a` | `3aad7c42f897663d09ebe0b27106a95735a7b5b6ad254bd2029644110e2d97e6` |
| 110 | 6 | 3 | 3 | `-C llvm-args=-align-all-functions=6 -C llvm-args=-align-all-nofallthru-blocks=3 -C llvm-args=-align-all-blocks=3` | `5bf39f3faf7262e004ce506da6d1987a53018325afb0f2c7962086330e3a0996` | `7971eb7939eb33b7bdfd73734da9974a7c69e8f539a0115545432d53bf6d6f65` |
| 111 | 6 | 3 | 0 | `-C llvm-args=-align-all-functions=6 -C llvm-args=-align-all-nofallthru-blocks=3` | `f40b2811e88d71d375c95aeeecf66f01686df2bc075844d15214cc874b52282b` | `83bc2906b18bad0aad00b607c1f72ac437acf0523c921b4c9d61eaa5393f8476` |
| 112 | 7 | 0 | 3 | `-C llvm-args=-align-all-functions=7 -C llvm-args=-align-all-blocks=3` | `a508d36c23ef7f295b6c40893fc01c8672aa36a9b6570d6e3bb4842d44ded18e` | `d745c554cff2db62e5e26d998e7cd721cd8888376579c42d863fba67cf932dfb` |
| 113 | 7 | 0 | 0 | `-C llvm-args=-align-all-functions=7` | `26b431c7996797bbd38b7998d5cc4a7034005e43fbae61a8ced5c8bb404eab0b` | `2af11f21805a171de07f937ff99e2c1758bc65dbeee377b9e9bdfcae8e890c4b` |
| 114 | 7 | 0 | 1 | `-C llvm-args=-align-all-functions=7 -C llvm-args=-align-all-blocks=1` | `7cb2a72b77b05716f623fb1c19c38a92cb071f0c36db47cc616e728800e6bb5d` | `161fded23ff5bd1c3c6c1cb6752b98907b6fbcfcdb11df68dee6f8bea4767381` |
| 115 | 7 | 0 | 2 | `-C llvm-args=-align-all-functions=7 -C llvm-args=-align-all-blocks=2` | `f15e5697305940bd0e6f1c29e26f96cb113f405ae96de38b0323f6b674725e65` | `4aae33ba7341c319b6d392fad490633a061e04d3834d70a677b561739900a3aa` |
| 116 | 7 | 1 | 0 | `-C llvm-args=-align-all-functions=7 -C llvm-args=-align-all-nofallthru-blocks=1` | `a987a23e3e4502bc45fc0f783dd8f49a5c9e7e0b3062a16d1e8f3eccb5f81da8` | `108ed289e7a638c3ab8a930a27f4ba9355b7bf5e7963fc51975bd931d13f6c3e` |
| 117 | 7 | 1 | 1 | `-C llvm-args=-align-all-functions=7 -C llvm-args=-align-all-nofallthru-blocks=1 -C llvm-args=-align-all-blocks=1` | `5e4bd6d724fd96f6e90056de1e37fa0c0ea9a92ec4f36badc407b093dbe071a3` | `27063efeed3aafc5ac0413f1554cc47daf8956ed5469859c1b04d6be01c0f485` |
| 118 | 7 | 1 | 2 | `-C llvm-args=-align-all-functions=7 -C llvm-args=-align-all-nofallthru-blocks=1 -C llvm-args=-align-all-blocks=2` | `d1aac803e8531c844b56d9c1fe7f25ae4727a9184627ebd89265516615482367` | `b7e35f8707f534b2a005940aeb219e2eeb265846eb5b7e7b49036796f710e946` |
| 119 | 7 | 1 | 3 | `-C llvm-args=-align-all-functions=7 -C llvm-args=-align-all-nofallthru-blocks=1 -C llvm-args=-align-all-blocks=3` | `1d53c36c5963c063decd38f08742b373db6dbe12ce15309e51e886bab40a489e` | `fbb5ef67ff30bdd1fe4977eaf0318f16d7f46722745cbd61693c1e5e7bb64d56` |
| 120 | 7 | 2 | 1 | `-C llvm-args=-align-all-functions=7 -C llvm-args=-align-all-nofallthru-blocks=2 -C llvm-args=-align-all-blocks=1` | `0b99f210e667eefc80a0b57e1619a86fcb8f703b4ed02971169309c33a05f6df` | `5ba146dc1deed30bc3c58c89b8d1dfaa7c91db0528f1bc5f66463c88702360af` |
| 121 | 7 | 2 | 2 | `-C llvm-args=-align-all-functions=7 -C llvm-args=-align-all-nofallthru-blocks=2 -C llvm-args=-align-all-blocks=2` | `e100c20e078b2aee5be6567844216f8fbcfc3d5ee757d8de2c4efddaa6d217b1` | `64f0c3ff456dfe5f192f6acaa82c326bb37b19f2b21eec6287c5e3bcc395b4a6` |
| 122 | 7 | 2 | 3 | `-C llvm-args=-align-all-functions=7 -C llvm-args=-align-all-nofallthru-blocks=2 -C llvm-args=-align-all-blocks=3` | `e7fe30854d4de7824f2670265669b282b718d7fd6aefbfb7427969bb731b70f6` | `d7fce7d4f2d1ff4fe9590c6304a727bbcf38f464379284c4e145e1f93dc56aaf` |
| 123 | 7 | 2 | 0 | `-C llvm-args=-align-all-functions=7 -C llvm-args=-align-all-nofallthru-blocks=2` | `9ef9727221bd2e93b8ab3a8126ce61954e5818241ceccc316516f8131e74c22b` | `a00ea13677791470b944f76b4589baee1f282edf2e5d8db2b82ce08d26adbdbc` |
| 124 | 7 | 3 | 2 | `-C llvm-args=-align-all-functions=7 -C llvm-args=-align-all-nofallthru-blocks=3 -C llvm-args=-align-all-blocks=2` | `5d77720fd0e1e40a7dc9d99f9eb19cec082e18617797c4ff3ceadaf31b3eec81` | `419e324c336b0519030c379858249ca013020d50927e28d1127fc8ff330ad55c` |
| 125 | 7 | 3 | 3 | `-C llvm-args=-align-all-functions=7 -C llvm-args=-align-all-nofallthru-blocks=3 -C llvm-args=-align-all-blocks=3` | `9e0625e94f0d4a80dcb62cf91d0189a00d6bcefbf832f226426485e186614e07` | `c6554d0708833d23e7170dbad2323a5cb7244c3dc6a2dfd126a58a08f5ad654c` |
| 126 | 7 | 3 | 0 | `-C llvm-args=-align-all-functions=7 -C llvm-args=-align-all-nofallthru-blocks=3` | `361f286eb6bf4b9af413ee02ab83c59a141edc7dc62f44c1376c27be1d3230d4` | `593900e24e88af2a457c4534bf76df2b52c727dc084a3f37b62557cb6b9401c6` |
| 127 | 7 | 3 | 1 | `-C llvm-args=-align-all-functions=7 -C llvm-args=-align-all-nofallthru-blocks=3 -C llvm-args=-align-all-blocks=1` | `5e4bd6d724fd96f6e90056de1e37fa0c0ea9a92ec4f36badc407b093dbe071a3` | `4f4780c904268e0ce697757c7824cca11e366551938a214d671339749d3f73c8` |

## Arm ordering

Member `j` runs its two arms adjacently, reference first when `j` is even and
candidate first when `j` is odd, per amendment §5.2. The session driver
implements exactly that: over the 256 slots there is no violation of the rule,
each arm holds exactly 64 of the first-of-pair positions and 64 of the second,
and the mean slot position of the two arms is equal at 128.5 — the exact
half of the 256 positions, with none of the one-slot residual that five
executions per arm forced on the two earlier sessions.

| Slot | Member `j` | Arm | Execution |
|---:|---:|---|---:|
| 1 | 0 | reference | 1 |
| 2 | 0 | candidate | 1 |
| 3 | 1 | candidate | 2 |
| 4 | 1 | reference | 2 |
| ... | ... | ... | ... |
| 253 | 126 | reference | 127 |
| 254 | 126 | candidate | 127 |
| 255 | 127 | candidate | 128 |
| 256 | 127 | reference | 128 |

Consecutive executions start 10 s to 11 s apart, mean 10.86 s, against 8.5 s of
timed work per execution; the amendment's §5.3 budget of about 11 s per amended
execution is what the session costs.

## Step 0 — harness against the plan

`--self-check` prints the same line from the staged member-0 binary of each
checkout, run with its working directory at its own checkout:

```
protocol: schema=selector-non-regression-v1 cells=34 repetitions=5 target_ms=250 per_cell_tolerance=0.050000 set_tolerance=0.020000 simd_min_words=8
self-check PASS
```

`per_cell_tolerance=0.050000` and `set_tolerance=0.020000` equal the plan's §4
values, and `cells=34` equals its §2 count. `simd_min_words=8` is the value the
candidate checkout resolves through the published threshold and the value the
reference checkout reads from its compiled-in constant, so the two agree on the
guard the bit-backend cells straddle.

`--list-cells` prints thirty-four rows in each checkout and the two listings are
byte-identical, reproducing the plan's §2 table cell for cell with the arm it
predicts for each:

```
cell	family	arm	size_a	size_b
bit_backend/xor_inplace/words=1	bit_backend	scalar	1	0
bit_backend/xor_inplace/words=4	bit_backend	scalar	4	0
bit_backend/xor_inplace/words=7	bit_backend	scalar	7	0
bit_backend/xor_inplace/words=8	bit_backend	simd	8	0
bit_backend/xor_inplace/words=16	bit_backend	simd	16	0
bit_backend/xor_inplace/words=64	bit_backend	simd	64	0
bit_backend/and_inplace/words=1	bit_backend	scalar	1	0
bit_backend/and_inplace/words=8	bit_backend	simd	8	0
bit_backend/or_inplace/words=1	bit_backend	scalar	1	0
bit_backend/or_inplace/words=8	bit_backend	simd	8	0
bit_backend/not_inplace/words=1	bit_backend	scalar	1	0
bit_backend/not_inplace/words=8	bit_backend	simd	8	0
bit_backend/popcount/words=1	bit_backend	scalar	1	0
bit_backend/popcount/words=8	bit_backend	simd	8	0
polynomial/mul/len=16	polynomial	schoolbook	16	0
polynomial/mul/len=32	polynomial	schoolbook	32	0
polynomial/mul/len=33	polynomial	karatsuba	33	0
polynomial/mul/len=64	polynomial	karatsuba	64	0
polynomial/mul/len=256	polynomial	karatsuba	256	0
polynomial/mul_fast/len=32	polynomial	mul_dispatch	32	0
polynomial/mul_fast/len=64	polynomial	mul_dispatch	64	0
polynomial/mul_fast/len=65	polynomial	ntt	65	0
polynomial/mul_fast/len=128	polynomial	ntt	128	0
polynomial/mul_fast/len=512	polynomial	ntt	512	0
polynomial/div_rem_auto/dividend=4096/divisor=1024	polynomial	schoolbook	4096	1024
polynomial/div_rem_auto/dividend=4096/divisor=2047	polynomial	schoolbook	4096	2047
polynomial/div_rem_auto/dividend=4096/divisor=2048	polynomial	newton	4096	2048
polynomial/div_rem_auto/dividend=8192/divisor=4096	polynomial	newton	8192	4096
polynomial/batch_evaluate/coeffs=2048/points=2048	polynomial	horner	2048	2048
polynomial/batch_evaluate/coeffs=4095/points=4095	polynomial	horner	4095	4095
polynomial/batch_evaluate/coeffs=4096/points=4096	polynomial	subproduct	4096	4096
polynomial/batch_evaluate_auto/coeffs=2048/points=2048	polynomial	horner	2048	2048
polynomial/batch_evaluate_auto/coeffs=4095/points=4095	polynomial	horner	4095	4095
polynomial/batch_evaluate_auto/coeffs=4096/points=4096	polynomial	subproduct	4096	4096
```

## Step 1 — commands

The build phase runs first and holds no lock. Each member is built by putting
its `RUSTFLAGS` in the environment and reading the produced binary from Cargo's
own JSON, because a changed `RUSTFLAGS` changes the artifact's filename hash:

```sh
RUSTFLAGS="<member flags>" cargo +1.95.0 bench -p gf2-core --features simd \
  --bench selector_non_regression --no-run --message-format=json |
  jq -r 'select(.executable != null and .target.name == "selector_non_regression") | .executable'
```

The binary is copied to `/tmp/gf2-ens3/<arm>/member-<j>.bin` and hashed before
the next member is built. Member 0 takes no options at all, so it is built with
`RUSTFLAGS` unset rather than empty.

The timed phase is one wrapper invocation over all 256 executions:

```sh
test ! -e /tmp/gf2-e8fe47f5-ensemble-reference-arm-0c072d73.csv
test ! -e /tmp/gf2-e8fe47f5-ensemble-candidate-arm-46d21d6d.csv
GF2_BENCH=1 ./dev/scripts/ccx1-bench-flock.sh bash -l /tmp/gf2-ens3/run-ensemble.sh \
  /tmp/gf2-e8fe47f5-ensemble-reference-arm-0c072d73.csv \
  /tmp/gf2-e8fe47f5-ensemble-candidate-arm-46d21d6d.csv
```

`run-ensemble.sh` is the session driver. It records the host facts this receipt
cites, then runs the 256 executions in the order §5.2 fixes:

```sh
run_ref() {
  local j="$1"
  cd "$CTRL"
  "$STAGE/ref/member-$j.bin" --execution "$(( j + 1 ))" --repetitions 1 \
    --target-ms 250 --output "$REF_OUT" --append
}

run_cand() {
  local j="$1"
  guard_main "$j"
  cd "$MAIN"
  "$STAGE/cand/member-$j.bin" --execution "$(( j + 1 ))" --repetitions 1 \
    --target-ms 250 --output "$CAND_OUT" --append
}

while read -r j; do
  if [ $(( j % 2 )) -eq 0 ]; then run_ref "$j"; run_cand "$j"
  else run_cand "$j"; run_ref "$j"; fi
done < "$STAGE/members.txt"
```

Each staged binary runs with its working directory at its own checkout, so each
row records that arm's revision, and `--output` takes an absolute `/tmp` path
checked absent beforehand. `guard_main` is this session's one addition to the
driver and is recorded in Deviations below: it re-reads the candidate checkout's
`HEAD` and full-worktree porcelain status before every candidate execution and
aborts the run if either has moved. It took no action; the session log records
`HEAD` and an empty status at the start and at the end.

## Arm rates

Pooled nanoseconds per call per the plan's §5, computed over an arm's whole
ensemble: the sum of `elapsed_ns` over every recorded window of a cell in that
arm divided by the sum of `calls`. Each arm contributes 128 windows per cell.
The ratio column is the verdict statistic ρ(c) = T_C(c) / T_R(c).

| Cell | Arm | Reference arm ns/call | Candidate arm ns/call | ρ(c) |
|---|---|---:|---:|---:|
| `bit_backend/and_inplace/words=1` | scalar | 3.853664 | 3.888949 | 1.009156 |
| `bit_backend/and_inplace/words=8` | simd | 4.775351 | 4.824412 | 1.010274 |
| `bit_backend/not_inplace/words=1` | scalar | 2.587264 | 2.462258 | 0.951684 |
| `bit_backend/not_inplace/words=8` | simd | 4.140258 | 4.333668 | 1.046715 |
| `bit_backend/or_inplace/words=1` | scalar | 3.816618 | 3.907761 | 1.023880 |
| `bit_backend/or_inplace/words=8` | simd | 4.975236 | 5.015781 | 1.008149 |
| `bit_backend/popcount/words=1` | scalar | 1.911839 | 2.306798 | 1.206586 |
| `bit_backend/popcount/words=8` | simd | 3.657607 | 3.867758 | 1.057456 |
| `bit_backend/xor_inplace/words=1` | scalar | 3.013004 | 3.165134 | 1.050491 |
| `bit_backend/xor_inplace/words=16` | simd | 4.349764 | 4.571772 | 1.051039 |
| `bit_backend/xor_inplace/words=4` | scalar | 4.102217 | 4.303261 | 1.049009 |
| `bit_backend/xor_inplace/words=64` | simd | 7.516105 | 7.683223 | 1.022235 |
| `bit_backend/xor_inplace/words=7` | scalar | 4.871424 | 5.004030 | 1.027221 |
| `bit_backend/xor_inplace/words=8` | simd | 3.947340 | 4.101983 | 1.039177 |
| `polynomial/batch_evaluate/coeffs=2048/points=2048` | horner | 14,284,045.118107 | 14,274,121.216452 | 0.999305 |
| `polynomial/batch_evaluate/coeffs=4095/points=4095` | horner | 57,149,373.052734 | 57,165,364.509766 | 1.000280 |
| `polynomial/batch_evaluate/coeffs=4096/points=4096` | subproduct | 26,620,038.078276 | 26,634,477.516300 | 1.000542 |
| `polynomial/batch_evaluate_auto/coeffs=2048/points=2048` | horner | 14,285,767.982537 | 14,269,531.695312 | 0.998863 |
| `polynomial/batch_evaluate_auto/coeffs=4095/points=4095` | horner | 57,121,326.970703 | 57,115,004.353516 | 0.999889 |
| `polynomial/batch_evaluate_auto/coeffs=4096/points=4096` | subproduct | 18,971,746.351897 | 18,972,589.968731 | 1.000044 |
| `polynomial/div_rem_auto/dividend=4096/divisor=1024` | schoolbook | 4,002,629.221705 | 3,964,701.105263 | 0.990524 |
| `polynomial/div_rem_auto/dividend=4096/divisor=2047` | schoolbook | 5,310,073.146092 | 5,311,507.845741 | 1.000270 |
| `polynomial/div_rem_auto/dividend=4096/divisor=2048` | newton | 2,311,051.130397 | 2,310,553.715136 | 0.999785 |
| `polynomial/div_rem_auto/dividend=8192/divisor=4096` | newton | 5,103,241.344511 | 5,105,477.814475 | 1.000438 |
| `polynomial/mul/len=16` | schoolbook | 322.032687 | 332.383538 | 1.032142 |
| `polynomial/mul/len=256` | karatsuba | 38,862.695830 | 39,010.410514 | 1.003801 |
| `polynomial/mul/len=32` | schoolbook | 1,249.975446 | 1,288.139968 | 1.030532 |
| `polynomial/mul/len=33` | karatsuba | 1,201.054504 | 1,201.872899 | 1.000681 |
| `polynomial/mul/len=64` | karatsuba | 4,059.267589 | 4,074.309731 | 1.003706 |
| `polynomial/mul_fast/len=128` | ntt | 11,953.735548 | 11,952.058087 | 0.999860 |
| `polynomial/mul_fast/len=32` | mul_dispatch | 1,258.964442 | 1,266.520887 | 1.006002 |
| `polynomial/mul_fast/len=512` | ntt | 53,776.109894 | 53,764.254336 | 0.999780 |
| `polynomial/mul_fast/len=64` | mul_dispatch | 4,019.595632 | 4,040.630285 | 1.005233 |
| `polynomial/mul_fast/len=65` | ntt | 11,957.815641 | 11,956.089531 | 0.999856 |

## Comparison of the two arms — the verdict

This is the comparison amendment §6.2 defines and the epic gates on: the
reference arm against the candidate arm, on pooled ns/call per cell, judged by
τ_cell = 5 % and τ_set = 2 %, computed by the plan's §5 mode unmodified.

```sh
cargo +1.95.0 bench -p gf2-core --features simd --bench selector_non_regression -- \
  --compare dev/benchmarks/tuning_profiles/2026-08-20-ensemble-reference-arm.csv \
  --against dev/benchmarks/tuning_profiles/2026-08-20-ensemble-candidate-arm.csv
```

The run prints all thirty-four per-cell lines and a geometric mean, so the §5
preconditions hold: both files carry schema `selector-non-regression-v1`, their
cell sets are identical and equal to the pinned set of plan §2, and every cell's
arm, family and operand sizes agree. The failure is therefore a tolerance
failure rather than a `RESULT: FAIL (selector identity mismatch)`.

```
comparison: baseline=dev/benchmarks/tuning_profiles/2026-08-20-ensemble-reference-arm.csv candidate=dev/benchmarks/tuning_profiles/2026-08-20-ensemble-candidate-arm.csv per_cell_tolerance=0.050000 set_tolerance=0.020000
cell arm baseline_ns_per_call candidate_ns_per_call ratio verdict
bit_backend/and_inplace/words=1 scalar 3.853664 3.888949 1.009156 PASS
bit_backend/and_inplace/words=8 simd 4.775351 4.824412 1.010274 PASS
bit_backend/not_inplace/words=1 scalar 2.587264 2.462258 0.951684 PASS
bit_backend/not_inplace/words=8 simd 4.140258 4.333668 1.046715 PASS
bit_backend/or_inplace/words=1 scalar 3.816618 3.907761 1.023880 PASS
bit_backend/or_inplace/words=8 simd 4.975236 5.015781 1.008149 PASS
bit_backend/popcount/words=1 scalar 1.911839 2.306798 1.206586 FAIL
bit_backend/popcount/words=8 simd 3.657607 3.867758 1.057456 FAIL
bit_backend/xor_inplace/words=1 scalar 3.013004 3.165134 1.050491 FAIL
bit_backend/xor_inplace/words=16 simd 4.349764 4.571772 1.051039 FAIL
bit_backend/xor_inplace/words=4 scalar 4.102217 4.303261 1.049009 PASS
bit_backend/xor_inplace/words=64 simd 7.516105 7.683223 1.022235 PASS
bit_backend/xor_inplace/words=7 scalar 4.871424 5.004030 1.027221 PASS
bit_backend/xor_inplace/words=8 simd 3.947340 4.101983 1.039177 PASS
polynomial/batch_evaluate/coeffs=2048/points=2048 horner 14284045.118107 14274121.216452 0.999305 PASS
polynomial/batch_evaluate/coeffs=4095/points=4095 horner 57149373.052734 57165364.509766 1.000280 PASS
polynomial/batch_evaluate/coeffs=4096/points=4096 subproduct 26620038.078276 26634477.516300 1.000542 PASS
polynomial/batch_evaluate_auto/coeffs=2048/points=2048 horner 14285767.982537 14269531.695312 0.998863 PASS
polynomial/batch_evaluate_auto/coeffs=4095/points=4095 horner 57121326.970703 57115004.353516 0.999889 PASS
polynomial/batch_evaluate_auto/coeffs=4096/points=4096 subproduct 18971746.351897 18972589.968731 1.000044 PASS
polynomial/div_rem_auto/dividend=4096/divisor=1024 schoolbook 4002629.221705 3964701.105263 0.990524 PASS
polynomial/div_rem_auto/dividend=4096/divisor=2047 schoolbook 5310073.146092 5311507.845741 1.000270 PASS
polynomial/div_rem_auto/dividend=4096/divisor=2048 newton 2311051.130397 2310553.715136 0.999785 PASS
polynomial/div_rem_auto/dividend=8192/divisor=4096 newton 5103241.344511 5105477.814475 1.000438 PASS
polynomial/mul/len=16 schoolbook 322.032687 332.383538 1.032142 PASS
polynomial/mul/len=256 karatsuba 38862.695830 39010.410514 1.003801 PASS
polynomial/mul/len=32 schoolbook 1249.975446 1288.139968 1.030532 PASS
polynomial/mul/len=33 karatsuba 1201.054504 1201.872899 1.000681 PASS
polynomial/mul/len=64 karatsuba 4059.267589 4074.309731 1.003706 PASS
polynomial/mul_fast/len=128 ntt 11953.735548 11952.058087 0.999860 PASS
polynomial/mul_fast/len=32 mul_dispatch 1258.964442 1266.520887 1.006002 PASS
polynomial/mul_fast/len=512 ntt 53776.109894 53764.254336 0.999780 PASS
polynomial/mul_fast/len=64 mul_dispatch 4019.595632 4040.630285 1.005233 PASS
polynomial/mul_fast/len=65 ntt 11957.815641 11956.089531 0.999856 PASS
geometric_mean 1.017669 PASS
RESULT: FAIL
```

The comparison exits 1.

Four cells exceed τ_cell. The set statistic, 1.017669, is inside τ_set = 1.02;
the rule requires both to hold, so the comparison fails.

| Cell | Arm | Reference ns/call | Candidate ns/call | ρ(c) | Above τ_cell by | ρ(c) in layout standard errors |
|---|---|---:|---:|---:|---:|---:|
| `bit_backend/popcount/words=1` | scalar | 1.911839 | 2.306798 | 1.206586 | +0.156586 | 35.41 |
| `bit_backend/popcount/words=8` | simd | 3.657607 | 3.867758 | 1.057456 | +0.007456 | 8.61 |
| `bit_backend/xor_inplace/words=16` | simd | 4.349764 | 4.571772 | 1.051039 | +0.001039 | 4.11 |
| `bit_backend/xor_inplace/words=1` | scalar | 3.013004 | 3.165134 | 1.050491 | +0.000491 | 9.04 |

The last column is `ln ρ(c) / se(c)`, how far the measured ratio stands from 1
in units of the layout standard error the audit computes for that cell. The two
`popcount` cells stand far outside it; `bit_backend/xor_inplace/words=16` and
`bit_backend/xor_inplace/words=1` stand 0.099 % and 0.047 % above the bar and
sit 4.11 and 9.04 layout standard errors above 1.

## The attribution audit

Amendment §6.5's `--layout-audit` mode computes §6.4's four preconditions. The
receipt pair naming the two committed receipts whose ratio defines `σ̂` is
mandatory, and it is supplied:

```sh
cargo +1.95.0 bench -p gf2-core --features simd --bench selector_non_regression -- \
  --layout-audit dev/benchmarks/tuning_profiles/2026-08-20-ensemble-reference-arm.csv \
  --layout-candidate dev/benchmarks/tuning_profiles/2026-08-20-ensemble-candidate-arm.csv \
  --compare dev/benchmarks/tuning_profiles/2026-08-19-pre-cutover-baseline.csv \
  --against dev/benchmarks/tuning_profiles/2026-08-20-post-cutover-control-arm-2.csv
```

The mode audits rather than refusing, so the two arms describe the predeclared
ensemble of the pinned set: one revision each, no row recording
`source_dirty=true`, no execution index below one, 128 members per arm, halves
balanced on member parity, the same member indices in both arms, and a receipt
pair that is the pinned set.

```
layout-audit: reference=dev/benchmarks/tuning_profiles/2026-08-20-ensemble-reference-arm.csv candidate=dev/benchmarks/tuning_profiles/2026-08-20-ensemble-candidate-arm.csv reference_revision=0c072d73ca65cf50af98b8c4b61ed876f8218df6 candidate_revision=46d21d6dcfd85b24fff938606dde73844f12546e builds=128 per_cell_tolerance=0.050000 set_tolerance=0.020000 attribution_margin=3.000000
cell arm reference_spread reference_log_sd candidate_log_sd paired_log_sd layout_se margin null_ratio natural_sigma verdict
bit_backend/and_inplace/words=1 scalar 1.963603 0.064587 0.040152 0.069160 0.006722 7.258 1.009565 0.040952 PASS
bit_backend/and_inplace/words=8 simd 1.200017 0.042748 0.104135 0.104062 0.009950 4.904 1.010597 0.033324 PASS
bit_backend/not_inplace/words=1 scalar 1.270778 0.066757 0.044562 0.082213 0.007094 6.877 1.005388 0.000222 PASS
bit_backend/not_inplace/words=8 simd 1.426011 0.056975 0.081146 0.092973 0.008764 5.567 1.002326 0.006726 PASS
bit_backend/or_inplace/words=1 scalar 1.184605 0.047448 0.042242 0.047075 0.005615 8.689 0.994693 0.118154 FAIL
bit_backend/or_inplace/words=8 simd 1.181130 0.041864 0.069121 0.070985 0.007143 6.831 0.997913 0.026910 PASS
bit_backend/popcount/words=1 scalar 1.173523 0.041295 0.043538 0.050160 0.005304 9.199 1.001217 0.032932 PASS
bit_backend/popcount/words=8 simd 1.126130 0.041415 0.060588 0.067570 0.006487 7.521 1.000618 0.000442 PASS
bit_backend/xor_inplace/words=1 scalar 1.256847 0.038850 0.047858 0.057400 0.005448 8.955 0.994377 0.099112 FAIL
bit_backend/xor_inplace/words=16 simd 1.710989 0.066487 0.119955 0.144973 0.012122 4.025 1.005229 0.007439 PASS
bit_backend/xor_inplace/words=4 scalar 1.461026 0.044247 0.075244 0.067586 0.007715 6.324 1.004520 0.037917 PASS
bit_backend/xor_inplace/words=64 simd 1.376082 0.048640 0.044744 0.063890 0.005842 8.352 0.998218 0.004897 PASS
bit_backend/xor_inplace/words=7 scalar 1.591662 0.054521 0.047852 0.041910 0.006412 7.609 1.004448 0.015778 PASS
bit_backend/xor_inplace/words=8 simd 1.963445 0.073044 0.115164 0.108166 0.012054 4.048 1.010962 0.007810 PASS
polynomial/batch_evaluate/coeffs=2048/points=2048 horner 1.023952 0.005426 0.003100 0.006222 0.000552 88.333 1.000002 0.001034 PASS
polynomial/batch_evaluate/coeffs=4095/points=4095 horner 1.023126 0.005349 0.003556 0.006336 0.000568 85.935 1.000233 0.002585 PASS
polynomial/batch_evaluate/coeffs=4096/points=4096 subproduct 1.096855 0.022369 0.022282 0.021120 0.002791 17.483 1.001652 0.027133 PASS
polynomial/batch_evaluate_auto/coeffs=2048/points=2048 horner 1.015029 0.003382 0.002753 0.003871 0.000385 126.566 0.999804 0.000433 PASS
polynomial/batch_evaluate_auto/coeffs=4095/points=4095 horner 1.012317 0.003648 0.004958 0.005123 0.000544 89.669 0.999874 0.002449 PASS
polynomial/batch_evaluate_auto/coeffs=4096/points=4096 subproduct 1.103149 0.021858 0.020563 0.021895 0.002653 18.394 1.001027 0.012806 PASS
polynomial/div_rem_auto/dividend=4096/divisor=1024 schoolbook 1.131647 0.050754 0.047989 0.042101 0.006174 7.903 0.999948 0.083921 PASS
polynomial/div_rem_auto/dividend=4096/divisor=2047 schoolbook 1.107397 0.028792 0.029046 0.026618 0.003615 13.497 0.998918 0.002770 PASS
polynomial/div_rem_auto/dividend=4096/divisor=2048 newton 1.032507 0.006406 0.005502 0.006126 0.000746 65.372 1.000198 0.007581 PASS
polynomial/div_rem_auto/dividend=8192/divisor=4096 newton 1.029544 0.004798 0.004382 0.005147 0.000574 84.954 0.999725 0.003686 PASS
polynomial/mul/len=16 schoolbook 1.164081 0.040146 0.041978 0.058867 0.005134 9.503 1.007171 0.012652 PASS
polynomial/mul/len=256 karatsuba 1.171583 0.041648 0.039764 0.040898 0.005090 9.586 0.996527 0.009322 PASS
polynomial/mul/len=32 schoolbook 1.197111 0.049771 0.052373 0.068342 0.006386 7.640 1.009897 0.012820 PASS
polynomial/mul/len=33 karatsuba 1.113463 0.029555 0.027215 0.028129 0.003551 13.739 1.000733 0.001730 PASS
polynomial/mul/len=64 karatsuba 1.162281 0.042744 0.043093 0.044514 0.005365 9.094 0.997912 0.008774 PASS
polynomial/mul_fast/len=128 ntt 1.039749 0.007826 0.008448 0.006724 0.001018 47.933 0.999443 0.008404 PASS
polynomial/mul_fast/len=32 mul_dispatch 1.161366 0.043579 0.042977 0.043536 0.005410 9.019 0.997360 0.001877 PASS
polynomial/mul_fast/len=512 ntt 1.037902 0.007217 0.007232 0.006716 0.000903 54.027 0.999721 0.008442 PASS
polynomial/mul_fast/len=64 mul_dispatch 1.155377 0.040614 0.040906 0.040160 0.005095 9.576 0.999836 0.003848 PASS
polynomial/mul_fast/len=65 ntt 1.044223 0.008073 0.008423 0.006478 0.001031 47.313 0.999592 0.009129 PASS
set layout_se=0.001883 margin=10.516 null_geomean=1.001451 PASS
coverage ensemble_rms=0.040753 natural_rms=0.033878 PASS
decorrelation paired_rms=0.057287 reference_rms=0.040753 PASS
RESULT: FAIL
```

The audit exits 1.

### The four preconditions

| Precondition | What §6.4 requires | Measured | Verdict |
|---|---|---|---|
| **Coverage, per cell** | `s_R(c) ≥ σ̂(c)/2` at every cell | Fails at two cells: `bit_backend/or_inplace/words=1` with `s_R` = 0.047448 against a floor of 0.059077, and `bit_backend/xor_inplace/words=1` with 0.038850 against 0.049556. The other thirty-two clear their floors | **FAIL** |
| **Coverage, whole set** | RMS of `s_R` ≥ RMS of `σ̂` over the pinned set | 0.040753 against 0.033878 | PASS |
| **Decorrelation** | RMS of `p` ≥ RMS of `s_R` | 0.057287 against 0.040753, a ratio of 1.406 against the 1.414 that fully independent arms would give | PASS |
| **Precision, per cell** | `ln(1.05) ≥ 3 · se(c)` at every cell | Every cell clears it; the narrowest margin is 4.025 at `bit_backend/xor_inplace/words=16` and the widest 126.566 at `polynomial/batch_evaluate_auto/coeffs=2048/points=2048` | PASS |
| **Precision, whole set** | `ln(1.02) ≥ 3 ·` set-level `se` | Set `se` = 0.001883, margin 10.516 | PASS |
| **Half-split null, per cell** | Inside ±τ_cell at every cell, read two-sided | Widest is 1.010962 at `bit_backend/xor_inplace/words=8`; every cell inside ±1.1 % | PASS |
| **Half-split null, whole set** | Inside ±τ_set on the geometric mean | 1.001451 | PASS |

The audit is `RESULT: FAIL` because the per-cell coverage precondition fails.

### The two cells that fail coverage

| Cell | `s_R` | Floor `σ̂`/2 | Shortfall | `s_R`/`σ̂` (needs ≥ 0.5) | Fraction of the floor reached |
|---|---:|---:|---:|---:|---:|
| `bit_backend/or_inplace/words=1` | 0.047448 | 0.059077 | 0.011629 | 0.4016 | 80.3 % |
| `bit_backend/xor_inplace/words=1` | 0.038850 | 0.049556 | 0.010706 | 0.3920 | 78.4 % |

These are two of the five cells at which the second receipt's control arm landed
outside ±5 % of the baseline. `σ̂` is derived from that pair: at
`bit_backend/or_inplace/words=1` the control arm stood at 0.846119 of the
baseline, giving `σ̂` = |ln 0.846119|/√2 = 0.118154, and at
`bit_backend/xor_inplace/words=1` it stood at 0.869215, giving 0.099112. Those
are the two largest `σ̂` in the pinned set, and they are the two the ensemble
fails to reach half of. The ensemble's own dispersion at these cells, 4.7 % and
3.9 %, is unremarkable beside its dispersion elsewhere — it is the floor that is
exceptional, not the ensemble.

§6.4 states what the factor of two is for: "one build pair estimates a
dispersion as `|Z|` times it, whose median is 0.674, so an ensemble standing
more than a factor of two below `σ̂` at a cell is tamer there rather than
unlucky." The measurement says these two cells are ones where a natural rebuild
moves the code further than this ensemble's alignment axes move it.

### Which §7 reading applies

The amendment's §7 table declares all four combinations in advance. The verdict
is `FAIL` and one precondition fails, so the fourth row governs:

> | FAIL | one or more fail | No attributable verdict, and the gate stays unmet either way. §6.6 governs. |

`50b47eae` REQ-01 is unmet. No passing comparison is claimed from this session,
and none is available to claim: the comparison fails as well.

§6.6 distinguishes two remedies by which precondition failed. Precision and the
half-split null both pass here, so the K = 256 ladder rung is not this session's
consequence. The failing precondition is coverage, and §6.6 says of it: "The
ensemble is not a model of the layout variation the record measures, and no
larger K repairs that. The run reports which cells fail and by how much, and the
ensemble's axes are the subject of a tracked amendment before the next measured
run." The cells and their shortfalls are the table above; the tracked amendment
belongs to the owner of epic `6dc81018`, not to this receipt.

## §7.1 — the recorded single-build comparison

Amendment §7.1 fixes this reading in advance: the receipt runs the plan's §5
comparison of the committed 2026-08-19 baseline against the candidate arm's
ordinary build alone — member A = B = C = 0, built under empty `RUSTFLAGS`,
binary SHA-256 `a76eb03e09c5e70b58c58653372a897e2a9b703e05584d0b5e7a67e8c3d2f6f4`
— and records its per-cell lines and its `RESULT:`. **It carries no verdict
standing.**

Its rows are the candidate arm's `execution=1` rows, extracted from the
committed arm file to an absolute `/tmp` path so that no in-repository file is
created:

```sh
OUT=/tmp/gf2-ens3/candidate-member0-ordinary.csv
head -1 dev/benchmarks/tuning_profiles/2026-08-20-ensemble-candidate-arm.csv > "$OUT"
awk -F, 'NR>1 && $2==1' dev/benchmarks/tuning_profiles/2026-08-20-ensemble-candidate-arm.csv >> "$OUT"
cargo +1.95.0 bench -p gf2-core --features simd --bench selector_non_regression -- \
  --compare dev/benchmarks/tuning_profiles/2026-08-19-pre-cutover-baseline.csv \
  --against "$OUT"
```

```
comparison: baseline=dev/benchmarks/tuning_profiles/2026-08-19-pre-cutover-baseline.csv candidate=/tmp/gf2-ens3/candidate-member0-ordinary.csv per_cell_tolerance=0.050000 set_tolerance=0.020000
cell arm baseline_ns_per_call candidate_ns_per_call ratio verdict
bit_backend/and_inplace/words=1 scalar 3.519421 3.592874 1.020871 PASS
bit_backend/and_inplace/words=8 simd 4.611779 4.612568 1.000171 PASS
bit_backend/not_inplace/words=1 scalar 2.833877 2.412034 0.851143 PASS
bit_backend/not_inplace/words=8 simd 4.159037 3.948100 0.949282 PASS
bit_backend/or_inplace/words=1 scalar 4.157592 3.624418 0.871759 PASS
bit_backend/or_inplace/words=8 simd 4.651777 4.663921 1.002610 PASS
bit_backend/popcount/words=1 scalar 1.853240 2.185640 1.179361 FAIL
bit_backend/popcount/words=8 simd 3.501248 3.713116 1.060512 FAIL
bit_backend/xor_inplace/words=1 scalar 3.282865 3.287139 1.001302 PASS
bit_backend/xor_inplace/words=16 simd 4.237336 4.411711 1.041152 PASS
bit_backend/xor_inplace/words=4 scalar 3.945581 3.946534 1.000242 PASS
bit_backend/xor_inplace/words=64 simd 7.510168 7.551452 1.005497 PASS
bit_backend/xor_inplace/words=7 scalar 4.512804 4.613147 1.022235 PASS
bit_backend/xor_inplace/words=8 simd 3.785692 3.953342 1.044285 PASS
polynomial/batch_evaluate/coeffs=2048/points=2048 horner 14259449.788235 14255350.705882 0.999713 PASS
polynomial/batch_evaluate/coeffs=4095/points=4095 horner 57113481.640000 57241903.500000 1.002249 PASS
polynomial/batch_evaluate/coeffs=4096/points=4096 subproduct 27445793.431818 26530260.333333 0.966642 PASS
polynomial/batch_evaluate_auto/coeffs=2048/points=2048 horner 14261197.134118 14264586.000000 1.000238 PASS
polynomial/batch_evaluate_auto/coeffs=4095/points=4095 horner 56938298.770000 57066613.500000 1.002254 PASS
polynomial/batch_evaluate_auto/coeffs=4096/points=4096 subproduct 19245019.569231 18682510.923077 0.970771 PASS
polynomial/div_rem_auto/dividend=4096/divisor=1024 schoolbook 4312979.932867 3834121.876923 0.888973 PASS
polynomial/div_rem_auto/dividend=4096/divisor=2047 schoolbook 5290880.676068 5373064.608696 1.015533 PASS
polynomial/div_rem_auto/dividend=4096/divisor=2048 newton 2301365.583704 2299817.611111 0.999327 PASS
polynomial/div_rem_auto/dividend=8192/divisor=4096 newton 5091288.531148 5082423.714286 0.998259 PASS
polynomial/mul/len=16 schoolbook 316.335008 320.159995 1.012092 PASS
polynomial/mul/len=256 karatsuba 36942.861779 39628.135397 1.072687 FAIL
polynomial/mul/len=32 schoolbook 1222.719206 1230.005440 1.005959 PASS
polynomial/mul/len=33 karatsuba 1140.814861 1209.789563 1.060461 FAIL
polynomial/mul/len=64 karatsuba 3807.325330 4217.814989 1.107816 FAIL
polynomial/mul_fast/len=128 ntt 11919.874440 11840.612157 0.993350 PASS
polynomial/mul_fast/len=32 mul_dispatch 1190.350051 1282.723733 1.077602 FAIL
polynomial/mul_fast/len=512 ntt 53574.708467 53270.481679 0.994321 PASS
polynomial/mul_fast/len=64 mul_dispatch 3813.538569 4080.228687 1.069932 FAIL
polynomial/mul_fast/len=65 ntt 11910.678494 11842.426718 0.994270 PASS
geometric_mean 1.006461 PASS
RESULT: FAIL
```

**The two comparisons disagree, at seven of the thirty-four cells.** That is the
case §7.1 exists for, and it directs that every disagreeing cell be recorded
with `s_R` and `σ̂`, "as the measurement of how much one draw moved the old
verdict".

| Cell | Single-build ratio | Single-build | Ensemble ρ(c) | Ensemble | `s_R` | `σ̂` |
|---|---:|---|---:|---|---:|---:|
| `bit_backend/xor_inplace/words=1` | 1.001302 | PASS | 1.050491 | FAIL | 0.038850 | 0.099112 |
| `bit_backend/xor_inplace/words=16` | 1.041152 | PASS | 1.051039 | FAIL | 0.066487 | 0.007439 |
| `polynomial/mul/len=256` | 1.072687 | FAIL | 1.003801 | PASS | 0.041648 | 0.009322 |
| `polynomial/mul/len=33` | 1.060461 | FAIL | 1.000681 | PASS | 0.029555 | 0.001730 |
| `polynomial/mul/len=64` | 1.107816 | FAIL | 1.003706 | PASS | 0.042744 | 0.008774 |
| `polynomial/mul_fast/len=32` | 1.077602 | FAIL | 1.006002 | PASS | 0.043579 | 0.001877 |
| `polynomial/mul_fast/len=64` | 1.069932 | FAIL | 1.005233 | PASS | 0.040614 | 0.003848 |

The disagreement runs in both directions and the direction is informative. Five
of the seven are polynomial cells that the single ordinary build trips and the
ensemble does not, by 6.0 % to 10.8 % — the same five cells the second receipt
records as tripping although no polynomial code changes between the compared
revisions. Their `σ̂` is small, 0.0017 to 0.0093, so one natural rebuild moved
them little; what moved them is the particular pair of builds the single-build
comparison happens to compare, and averaging over 128 layout draws removes it.
The remaining two are bit-backend cells the ensemble trips and the single build
does not, and both carry the largest `σ̂` in the set.

The two cells at which the two comparisons agree on `FAIL` are
`bit_backend/popcount/words=1` and `bit_backend/popcount/words=8`.

## §7.2 — the candidate arm's own across-build dispersion

Amendment §7.2 records this and it enters no verdict: `s_C(c)` materially above
`s_R(c)` at a cell says the cutover made that cell's cost more layout-sensitive.

Five cells stand at or above 1.5 × their reference-arm dispersion, all of them
bit-backend and four of the five on the SIMD arm:

| Cell | Arm | `s_R` | `s_C` | `s_C`/`s_R` |
|---|---|---:|---:|---:|
| `bit_backend/and_inplace/words=8` | simd | 0.042748 | 0.104135 | 2.436 |
| `bit_backend/xor_inplace/words=16` | simd | 0.066487 | 0.119955 | 1.804 |
| `bit_backend/xor_inplace/words=4` | scalar | 0.044247 | 0.075244 | 1.701 |
| `bit_backend/or_inplace/words=8` | simd | 0.041864 | 0.069121 | 1.651 |
| `bit_backend/xor_inplace/words=8` | simd | 0.073044 | 0.115164 | 1.577 |

`bit_backend/xor_inplace/words=16` is one of the four cells the verdict trips,
and it is the cell whose precision margin is narrowest at 4.025; its candidate
arm is the most layout-sensitive cell in the set. The other three tripping cells
do not appear here: `bit_backend/popcount/words=1` reads 1.054,
`bit_backend/popcount/words=8` 1.463 and `bit_backend/xor_inplace/words=1`
1.232. Sixteen cells move the other way, with `s_C` below `s_R`; the widest is
`polynomial/batch_evaluate/coeffs=2048/points=2048` at 0.571, followed by
`bit_backend/and_inplace/words=1` at 0.622.

This is recorded evidence about the change. It enters no verdict.

## §7.3 — cells whose own dispersion exceeds τ_cell

Plan §7 and amendment §7.3 direct that a cell whose own dispersion inside a
receipt exceeds τ_cell is recorded as noise-dominated together with its numbers,
that the tolerance is not widened for it, and that the cell is not dropped.

Under this amendment a build contributes one execution of one repetition, so a
cell's own dispersion inside this receipt is its across-build dispersion — the
quantity the ensemble deliberately injects. Eight cells in the candidate arm and
eight in the reference arm exceed τ_cell = 5 % on the sample coefficient of
variation of their 128 per-build rates, thirteen cells in one arm or the other:

| Cell | Reference arm CV | Candidate arm CV | `s_C`/`s_R` |
|---|---:|---:|---:|
| `bit_backend/and_inplace/words=1` | 7.944% | 3.972% | 0.622 |
| `bit_backend/and_inplace/words=8` | 4.230% | 13.247% | 2.436 |
| `bit_backend/not_inplace/words=1` | 6.781% | 4.481% | 0.668 |
| `bit_backend/not_inplace/words=8` | 5.946% | 9.336% | 1.424 |
| `bit_backend/or_inplace/words=1` | 4.693% | 4.176% | 0.890 |
| `bit_backend/or_inplace/words=8` | 4.180% | 8.694% | 1.651 |
| `bit_backend/popcount/words=1` | 4.089% | 4.387% | 1.054 |
| `bit_backend/popcount/words=8` | 4.173% | 7.398% | 1.463 |
| `bit_backend/xor_inplace/words=1` | 3.936% | 4.809% | 1.232 |
| `bit_backend/xor_inplace/words=16` | 7.754% | 15.119% | 1.804 |
| `bit_backend/xor_inplace/words=4` | 4.590% | 8.086% | 1.701 |
| `bit_backend/xor_inplace/words=64` | 5.276% | 4.859% | 0.920 |
| `bit_backend/xor_inplace/words=7` | 5.799% | 4.723% | 0.878 |
| `bit_backend/xor_inplace/words=8` | 9.282% | 14.794% | 1.577 |
| `polynomial/batch_evaluate/coeffs=2048/points=2048` | 0.545% | 0.310% | 0.571 |
| `polynomial/batch_evaluate/coeffs=4095/points=4095` | 0.537% | 0.355% | 0.665 |
| `polynomial/batch_evaluate/coeffs=4096/points=4096` | 2.261% | 2.258% | 0.996 |
| `polynomial/batch_evaluate_auto/coeffs=2048/points=2048` | 0.338% | 0.275% | 0.814 |
| `polynomial/batch_evaluate_auto/coeffs=4095/points=4095` | 0.365% | 0.497% | 1.359 |
| `polynomial/batch_evaluate_auto/coeffs=4096/points=4096` | 2.209% | 2.072% | 0.941 |
| `polynomial/div_rem_auto/dividend=4096/divisor=1024` | 5.130% | 4.896% | 0.946 |
| `polynomial/div_rem_auto/dividend=4096/divisor=2047` | 2.889% | 2.923% | 1.009 |
| `polynomial/div_rem_auto/dividend=4096/divisor=2048` | 0.641% | 0.551% | 0.859 |
| `polynomial/div_rem_auto/dividend=8192/divisor=4096` | 0.480% | 0.438% | 0.913 |
| `polynomial/mul/len=16` | 3.992% | 4.192% | 1.046 |
| `polynomial/mul/len=256` | 4.181% | 3.980% | 0.955 |
| `polynomial/mul/len=32` | 4.963% | 5.195% | 1.052 |
| `polynomial/mul/len=33` | 2.953% | 2.717% | 0.921 |
| `polynomial/mul/len=64` | 4.278% | 4.309% | 1.008 |
| `polynomial/mul_fast/len=128` | 0.786% | 0.849% | 1.079 |
| `polynomial/mul_fast/len=32` | 4.385% | 4.319% | 0.986 |
| `polynomial/mul_fast/len=512` | 0.723% | 0.725% | 1.002 |
| `polynomial/mul_fast/len=64` | 4.091% | 4.114% | 1.007 |
| `polynomial/mul_fast/len=65` | 0.812% | 0.846% | 1.043 |

The tolerance is not widened for any of them and no cell is dropped.

**What that dispersion does and does not say here.** It is the ensemble working
as designed rather than a defect in the measurement: the verdict statistic pools
128 windows per arm per cell, so the layout error it carries is
`se(c) = √(s_R² + s_C²)/√128`, which the precision precondition clears at every
cell with margins of 4.0 to 126.6. The widest per-build dispersion in the set,
15.1 % at `bit_backend/xor_inplace/words=16`, corresponds to a layout standard
error of 1.2 % on the pooled ratio. Recording the cells is what §7.3 requires;
the pooled statistic is not compromised by them, and the audit measures that
rather than asserting it.

## The eleven cells the second receipt failed

The second receipt records eleven of the thirty-four cells exceeding τ_cell
against the baseline. Their ratios there stand beside their amended verdict
ratios here. The two comparisons are not the same comparison — receipt-2's is
one post-cutover build against the committed baseline build, this one is 128
builds of the candidate revision against 128 builds of the baseline revision
measured in the same session — and the table is the measurement of what changed
between them, not a re-computation of either.

| Cell | Arm | Receipt-2 ratio | Receipt-2 verdict | Ensemble ρ(c) | Ensemble verdict | Change |
|---|---|---:|---|---:|---|---:|
| `bit_backend/popcount/words=1` | scalar | 1.294670 | FAIL | 1.206586 | FAIL | -0.088084 |
| `bit_backend/and_inplace/words=1` | scalar | 1.184311 | FAIL | 1.009156 | PASS | -0.175155 |
| `bit_backend/xor_inplace/words=7` | scalar | 1.148518 | FAIL | 1.027221 | PASS | -0.121297 |
| `bit_backend/or_inplace/words=8` | simd | 1.132977 | FAIL | 1.008149 | PASS | -0.124828 |
| `bit_backend/popcount/words=8` | simd | 1.123171 | FAIL | 1.057456 | FAIL | -0.065715 |
| `bit_backend/xor_inplace/words=64` | simd | 1.093284 | FAIL | 1.022235 | PASS | -0.071049 |
| `polynomial/mul/len=64` | karatsuba | 1.078367 | FAIL | 1.003706 | PASS | -0.074661 |
| `polynomial/mul_fast/len=32` | mul_dispatch | 1.077040 | FAIL | 1.006002 | PASS | -0.071038 |
| `polynomial/mul_fast/len=64` | mul_dispatch | 1.073771 | FAIL | 1.005233 | PASS | -0.068538 |
| `polynomial/mul/len=33` | karatsuba | 1.072686 | FAIL | 1.000681 | PASS | -0.072005 |
| `polynomial/mul/len=256` | karatsuba | 1.065752 | FAIL | 1.003801 | PASS | -0.061951 |

Nine of the eleven fall inside the tolerance. All five polynomial cells do, and
they fall furthest: `polynomial/mul/len=33` from 1.072686 to 1.000681,
`polynomial/mul/len=64` from 1.078367 to 1.003706, `polynomial/mul/len=256` from
1.065752 to 1.003801, `polynomial/mul_fast/len=32` from 1.077040 to 1.006002 and
`polynomial/mul_fast/len=64` from 1.073771 to 1.005233. These are the cells the
second receipt records as moving "although no polynomial code changes between
the compared revisions", and under the ensemble they no longer move.

Two of the eleven remain outside the tolerance: `bit_backend/popcount/words=1`
at 1.206586 and `bit_backend/popcount/words=8` at 1.057456. Both are lower than
in the second receipt and both still exceed τ_cell.

Two cells that passed in the second receipt fail here:
`bit_backend/xor_inplace/words=1`, which read 0.959701 there and reads 1.050491
here, and `bit_backend/xor_inplace/words=16`, which read 1.044342 there and
reads 1.051039 here. Both stand under 0.1 % above the bar — 0.099 % and 0.047 % — and
`bit_backend/xor_inplace/words=1` is one of the two cells at which the coverage
precondition fails, so no attribution is available for it from this session.

## The two cells the first session failed

The first receipt records `bit_backend/popcount/words=1` and
`bit_backend/popcount/words=8` as the two cells that exceeded τ_cell before
`c42720ce`. Both have now exceeded it in all three post-cutover sessions.

| Cell | Arm | First receipt ratio | Second receipt ratio | Ensemble ρ(c) | Ensemble verdict |
|---|---|---:|---:|---:|---|
| `bit_backend/popcount/words=1` | scalar | 1.182311 | 1.294670 | 1.206586 | FAIL |
| `bit_backend/popcount/words=8` | simd | 1.064214 | 1.123171 | 1.057456 | FAIL |

Against τ_cell = 1.05 the two cells stand 0.156586 and 0.007456 above the bar,
which is 14.91 % and 0.71 % beyond it. Both ratios are lower than the second
receipt's — the cost at `bit_backend/popcount/words=1` falls from 1.294670 to
1.206586 and at `bit_backend/popcount/words=8` from 1.123171 to 1.057456 — and
neither reduction reaches the tolerance. Both cells pass every attribution
precondition: coverage clears its floor at 0.041295 against 0.016466 and at
0.041415 against 0.000221, precision reads margins of 9.199 and 7.521, and the
half-split null reads 1.001217 and 1.000618. The movement at these two cells is
therefore attributable at this ensemble even though the session as a whole
establishes no verdict.

## Falsification record

### The session establishes no attributable verdict

Both the verdict and the audit report `RESULT: FAIL`. Per amendment §7's fourth
row this session establishes no attributable verdict and `50b47eae` REQ-01 is
unmet. Per §6.6 the failing precondition can only withhold a verdict; it can
never turn a failing comparison into a passing one, and no passing comparison is
claimed from this session in any case.

### Two cells fail the coverage precondition

`bit_backend/or_inplace/words=1` and `bit_backend/xor_inplace/words=1` record
`s_R` at 80.3 % and 78.4 % of the `σ̂`/2 floor. **What this contradicts.**
Amendment §3.4 sizes K = 128 against precisely these cells: it takes
`bit_backend/or_inplace/words=1` at 0.846119 of the baseline as the widest
across-build dispersion the record measures, estimates that cell's across-build
dispersion as `|ln r|/√2` — 0.118178 as §3.4 states it, 0.118154 as the audit
computes it from the committed rows — and sizes the ensemble so that
`3 · √2 · σ̂ / √K ≤ ln(1.05)`. The sizing argument assumed the ensemble
would reproduce a dispersion of that order at that cell. It does not: the
ensemble's `s_R` there is 0.047448, 40 % of `σ̂` rather than the ~100 % the
sizing presumed, and the same holds at `bit_backend/xor_inplace/words=1`.

The consequence for K is the opposite of a shortfall in precision — a smaller
`s_R` makes `se(c)` smaller and the precision margin wider, which is why
precision passes everywhere with 4.025 as its narrowest margin. What the
shortfall means is that the ensemble does not model, at these two cells, the
layout variation a natural rebuild produces there. §6.6 states the consequence:
no larger K repairs it, and the ensemble's axes are the subject of a tracked
amendment.

### The ensemble removes the movement at cells the cutover never touches

Amendment §6.3 states the assumption the procedure rests on — that the layout
ratio averages to 1 over the members — and §1 states what the record established
before this session: five polynomial cells stood 6.6 % to 7.8 % above the
baseline in the gated comparison "although no polynomial code changes between
the compared revisions". This session measures those five cells at 1.000681,
1.003706, 1.003801, 1.006002 and 1.005233 under the ensemble, and the same
session's single ordinary build reproduces the old effect at 1.060461, 1.107816,
1.072687, 1.077602 and 1.069932. The amendment's mechanism does what it was
predeclared to do at the cells it was predeclared for. This is recorded evidence;
it changes no verdict, and the verdict fails.

### The set rule passes and the per-cell rule fails

The geometric mean of the thirty-four ratios is 1.017669, inside τ_set = 1.02
for the first time in the three post-cutover sessions — the first receipt read
0.998853 on a two-cell failure, the second 1.027550 on an eleven-cell failure.
Plan §4 predicts that a cutover adding a fixed per-call cost at every selection
boundary shows "a small shift shared by all thirty-four cells rather than a large
shift in one", and would be caught by τ_set. What this session records is the
other shape: the set statistic inside its tolerance and four cells outside
theirs, concentrated on the two `popcount` cells. The set rule passing is not
evidence that the per-cell excursions are absent, and per plan §4 both rules must
hold.

### Preserved

Per `@/inv/falsification-preserved` and control-arm §4.2, every failing cell is
recorded above with both pooled rates and its ratio, the two coverage failures
are recorded with their shortfalls, and the disagreements between the amended and
unamended comparisons are recorded cell by cell. Per plan §7 and control-arm
§4.5 this session is not repeated until it agrees. It stands as taken.

## Disposition

The session establishes no attributable verdict, so `50b47eae` REQ-01 is unmet
and the epic's gate stays unmet. Amendment §6.6 governs, on its coverage branch:
the ensemble's axes are the subject of a tracked amendment before the next
measured run, and that amendment belongs to the owner of epic `6dc81018`.

Two questions the session poses and does not settle, both for that owner:

- **The ensemble's axes.** Three alignment axes at the levels §3.1 admits reach
  half of `σ̂` at thirty-two of the thirty-four cells and fall short at the two
  where a natural rebuild moves the code furthest. What axis or level reaches
  those two is a question about the ensemble, not about the cutover.
- **The two `popcount` cells.** They have exceeded τ_cell in all three
  post-cutover sessions, they exceed it here against a same-session ensemble of
  the baseline revision, they pass every attribution precondition at this
  ensemble, and `2a85f728` reduced but did not remove their cost. Whether the
  remaining 0.395 ns/call at `words=1` and 0.210 ns/call at `words=8` is the
  selection boundary's or the layout's is not decided by this session, whose
  audit fails.

## What stands unchanged

- **The tolerance.** τ_cell stays 5 % and τ_set stays 2 %, at their predeclared
  values. Neither is widened to admit this session's excursions.
- **The pinned set.** All thirty-four cells of plan §2 stand. No cell is added,
  dropped or re-bracketed, and every one of the 256 builds measures the same set,
  including the cells this receipt records under §7.3.
- **The ensemble.** K stays 128 and the enumeration of §3.1 stands. The margin of
  three standard errors stands.
- **The schema token.** `selector-non-regression-v1` is not bumped, so the
  baseline and both committed post-cutover receipts stay valid and comparable.
- **The standing receipts.** `2026-08-19-pre-cutover-baseline.md`,
  `2026-08-20-post-cutover-receipt.md` and
  `2026-08-20-post-cutover-receipt-2.md` stand as taken. This receipt does not
  modify, supersede or re-run any of them; the baseline and the second receipt's
  control arm are read here only as the natural pair that defines `σ̂`, which is
  the role amendment §8 assigns them.
- **This run.** It stands as taken.

## Deviations

Every departure from the procedure as written, however small, and what each one
does or does not touch.

- **The candidate revision is `46d21d6d`, not the `c3eb5d01` this session was
  dispatched against.** The main checkout advanced by two commits, `dc8df30c`
  and `46d21d6d`, between the dispatch and the build phase. Both change only
  files under `dev/active/6dc81018-field-capability-dispatch/`:
  `git diff c3eb5d01..46d21d6d -- crates Cargo.toml Cargo.lock rust-toolchain.toml`
  is empty, so the candidate binaries are compiled from the same crate sources
  `c3eb5d01` carries, including `2a85f728`'s selection-boundary fix. What the
  substitution changes is the revision string every candidate row records.
- **The session driver guards the candidate checkout.** Before every candidate
  execution the driver re-reads the main checkout's `HEAD` and its full-worktree
  porcelain status and aborts the run if either has moved, because a commit or a
  stray file mid-session would make the candidate arm span two revisions or
  record `source_dirty=true`, either of which the audit refuses. The guard took
  no action; the session log records `HEAD` at `46d21d6d` and an empty status at
  the start and at the end. It changes nothing about what is measured.
- **The child runs at niceness 5.** The wrapper's best-effort `nice -n -5` is
  denied, as plan §6 anticipates, but the invoking session's own niceness is 5
  rather than 0, so the child inherits 5 where the two committed post-cutover
  receipts record 0. The lock and the affinity remain in force, the host carries
  no competing work during the timed phase, and both arms run at the same
  niceness inside one session, so the verdict ratio carries none of it. The
  cross-session comparison of §7.1 is against a baseline taken at niceness 0.
- **Two members were already built when the build phase started.** Member 0 of
  each arm was built during the smoke test that verified the staged-binary
  recipe, so the build phase recorded 0 s for those two and reused the binaries.
  Their SHA-256 are the ones tabulated; the reference arm's is
  `c7be7a87d7a01d3c2e297919033a99237afe8939f3fce147802ecd86df32c710`, which is
  bit-identical to the control build of both earlier sessions.
- **One unlocked throwaway execution per arm preceded the build phase.** The
  smoke test ran one execution of one repetition from each staged member-0
  binary to `/tmp`, outside the lock, to verify that a staged binary records its
  own checkout's revision and `source_dirty=false`. Those two files are
  throwaways; no row of either belongs to an arm, and neither is copied into
  this directory.
- **`--self-check` and `--list-cells` were run from the staged member-0
  binaries** rather than through `cargo bench`, because the build phase leaves
  each checkout's target directory at the last member's `RUSTFLAGS`. The staged
  binaries are the ones the session measures, so this checks the protocol
  against the binaries that produced the rows.
- **The §7.1 extraction writes to `/tmp`.** The candidate arm's member-0 rows are
  extracted to an absolute `/tmp` path rather than into this directory, so no
  in-repository file is created for it. The extraction is one `head` and one
  `awk` over the committed arm file and is reproducible from it.
