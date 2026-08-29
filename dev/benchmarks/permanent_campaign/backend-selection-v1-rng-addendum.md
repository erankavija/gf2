# RNG provenance addendum to permanent campaign backend selection v1

This addendum supplies the random-number-generator and seed provenance for the
two matched cohorts recorded in
`dev/benchmarks/permanent_campaign/backend-selection-v1.md`, SHA-256
`fe5d37ba7c216a753bf3e546614a222c3c20e563f7f4e1d3c0341af9e1c464fe`. It closes
research-review finding F1, which records that the manifests supporting REQ-03
name neither the RNG implementation nor a committed dependency resolution from
which it can be recovered.

It is evidence about the measurement build, not a measurement. It adds no
timing, changes no rate, and moves no selection. The receipt it annotates, every
`premeasure-v1-*` artifact, and everything under `dev/simulation_results/` keep
their bytes and their digests.

## Cohort RNG identity

| Cohort | Generator drawing the measured matrices | Implementation crate | Version | Construction site |
|---|---|---|---|---|
| premeasure-v1 (60 cells, source revision `1350d5b4`) | ChaCha20 | `rand_chacha` | 0.9.0 | `ChaCha20Rng::from_seed(derive_seed(root, q, n, purpose, index))` |
| 296a41c9 frontier (3 cells, harness revision `414d31f8`) | ChaCha20 | `rand_chacha` | 0.9.0 | `ChaCha20Rng::from_seed(derive_seed(root, q, n, stream))` |

At revision `1350d5b4`, the harness manifest requires `rand_chacha = "0.9"` and
`rand_core = "0.9"`:

```
git show 1350d5b4:dev/research/permanent-sampling-feas/Cargo.toml | sed -n '60,63p'
```

names `rand_chacha` at line 62 and `rand_core` at line 63 under the comment at
line 60 declaring ChaCha20 the campaign RNG. The sampler imports
`rand_chacha::ChaCha20Rng` and `rand_core::{RngCore, SeedableRng}` at
`src/sampler.rs:58-59` and constructs the generator at `src/sampler.rs:295`.
Every measured matrix entry originates in `MatrixSampler::next_entry` /
`next_matrix` at `src/sampler.rs:332` and `src/sampler.rs:368`, which consume
bytes from that `ChaCha20Rng` through the 4 KiB refill buffer declared at
`src/sampler.rs:248-261`.

The exact patch version resolves through
`dev/benchmarks/permanent_campaign/premeasure-v1-harness.Cargo.lock` (below) and
is corroborated inside the measurement executable itself: the surviving binary
whose SHA-256 equals the receipt digest
`1198cce47de06a6793fd1b5d880d559f020acffebc3d237137fa5c1326775606` carries the
panic-location string `rand_chacha-0.9.0/src/guts.rs` and the ChaCha20 core
symbols `rand_chacha::guts::refill_wide` with its `impl_avx`, `impl_sse`, and
`impl_ssse` specializations.

```
strings -a /data/gf2-campaigns/b8206228/premeasure-recovery-20260824/permanent-sampling-feas-hip/release/permanent_sampling_feas \
  | grep -oE '[^ ]*registry/src/[^ ]*(rand|chacha|ppv)[^ ]*'
nm -C <same binary> | grep -oE '\b(rand_chacha|rand_core|rand)::[A-Za-z_:]+' | sort -u
```

The first command returns exactly one registry path,
`rand_chacha-0.9.0/src/guts.rs`. The second returns only `rand_chacha::guts`
symbols.

## The second RNG stack in the dependency graph

The measurement build's dependency graph carries a second, older RNG stack:
`rand` 0.8.7, `rand_chacha` 0.3.1, and `rand_core` 0.6.4. It enters through
`gf2-core`, whose default feature set includes `rand`
(`crates/gf2-core/Cargo.toml:42` at revision `1350d5b4`, with the optional
dependency at line 20 and the feature at line 48), resolved against the
workspace requirement `rand = "0.8"` at the repository root `Cargo.toml:28`.
The harness depends on `gf2-core` without disabling default features
(`dev/research/permanent-sampling-feas/Cargo.toml:48`), so the feature is on.
`rand` 0.8.7 in turn requires `rand_chacha` 0.3.1 and `rand_core` 0.6.4; the
lock records those edges.

That stack does not draw measurement matrices, and does not reach the
measurement executable at all:

- No source file of the harness at revision `1350d5b4` references `rand`,
  `gf2_algebra::testutil::random_matrix`, or `gf2_core::rng::Lcg` outside the
  module documentation at `src/sampler.rs:19-30`, which records why the in-tree
  LCG is unfit for a published statistic.
- The linked binary contains no `rand` 0.8.7, `rand_chacha` 0.3.1, or
  `rand_core` 0.6.4 symbol and no `Lcg` symbol. The `nm` scan above returns six
  `rand_chacha` symbols and nothing else from the RNG family.

The 0.8 stack is compiled into the build tree — its rlibs sit beside the
0.9 stack in `release/deps/` — and is dropped at link time because nothing calls
it.

## Rebuild proof

### Recipe

The campaign preparation step builds the harness at
`dev/scripts/permanent-campaign-runner.sh:364` (revision `1350d5b4`):

```
cargo +1.95.0 build --manifest-path "$SAMPLING_MANIFEST" --release --features hip --target-dir "$SAMPLING_TARGET_DIR"
```

with `[profile.release]` at `dev/research/permanent-sampling-feas/Cargo.toml:67-70`
setting `opt-level = 3`, `lto = "thin"`, and `codegen-units = 1`. The host
toolchain reports `rustc 1.95.0 (59807616e 2026-04-14)`, matching the compiler
recorded in the receipt.

### Transcript

```
git worktree add --detach <scratch>/src 1350d5b46cd541093882537a89ea35db05a7afc5
cp dev/research/permanent-sampling-feas/Cargo.lock <scratch>/src/dev/research/permanent-sampling-feas/Cargo.lock
ROCM_PATH=/opt/rocm cargo +1.95.0 build \
    --manifest-path <scratch>/src/dev/research/permanent-sampling-feas/Cargo.toml \
    --release --features hip --target-dir <scratch>/permanent-sampling-feas-hip
sha256sum <scratch>/permanent-sampling-feas-hip/release/permanent_sampling_feas
```

The build succeeds and the lock file is unchanged by it (SHA-256
`70e08e7619268dfea5855dd629d049a76cb49aa00ad39cc317a4bde4086c0555` before and
after), so the resolution it encodes is the one Cargo used.

### Whole-executable digest: not reproduced

| Artifact | SHA-256 | Size |
|---|---|---|
| Receipt digest, retained measurement binary | `1198cce47de06a6793fd1b5d880d559f020acffebc3d237137fa5c1326775606` | 1,550,672 |
| Rebuild from the same revision and lock at a scratch path | `2c1d1d6bb64ff5e0c87f9df5d7fd45ea376a0f6a55e92b6d58ec8e3097a49897` | 1,554,752 |

The difference is embedded build-location text, not compilation input. Cargo
passes absolute paths for out-of-workspace path dependencies and derives each
path dependency's `-C metadata` hash from its package id, which contains that
absolute path. A build rooted anywhere other than `/home/vkaskivuo/Projects/gf2`
therefore differs in:

- panic-location and `file!()` strings under `crates/gf2-*` and
  `dev/research/permanent_wave_gpu`;
- codegen-unit and LLVM anonymous-symbol names such as
  `gf2_algebra.84a9846ced51d9df-cgu.0`;
- the `__hip_cuid_*` and `__hip_gpubin_handle_*` identifiers hipcc assigns per
  translation unit;
- the `OUT_DIR` path that `dev/research/permanent_wave_gpu/build.rs:177-178`
  hands to the crate through `cargo:rustc-env`, which no `--remap-path-prefix`
  can normalize because the crate reads it as an environment value.

Reproducing the digest requires building with the frozen revision checked out
over the live working tree at that exact path, which would destroy uncommitted
state this addendum has no authority to touch. No second build attempt is made,
because no change of recipe can alter a difference that originates in the build
location.

### Per-crate digest: reproduced for the entire RNG chain

Comparing every rlib in the retained build tree against the rebuild is a sharper
test, and it isolates the question F1 asks. Registry crates carry no workspace
path, so their compilation is location-independent.

```
cd <retained>/release/deps && sha256sum *.rlib | sort -k2
cd <rebuild>/release/deps  && sha256sum *.rlib | sort -k2
```

31 of 40 rlibs are byte-identical, filename metadata hash included. The whole
RNG chain is among them:

| rlib | SHA-256 (both builds) |
|---|---|
| `librand_chacha-197e7e58a6f96e91.rlib` (0.9.0) | `72e1b614f1e681fb3037593fa9645c402d676e0287ee571a74cb57fe5b12efbb` |
| `librand_core-a6a9a0dbc98099a0.rlib` (0.9.5) | `a1008390d28cbbb17227c3c4caf5828756ef6187292a5585c098321e41f4b7dc` |
| `libppv_lite86-e9d1805f692c721f.rlib` | `69606c401f2cf940d7580bc026b21169ace22073a07ef92c93a77f730356c4d2` |
| `libgetrandom-06cd44683d3de332.rlib` | `2b8764c9cffc0cf9c9c21d316ce22c4c8cce0f648eaab883a3f1f86a6016da02` |
| `librand-a79e85695a014a23.rlib` (0.8.7) | `d7dc8696b7a78f2b11293dbb59ef5dddf57874a69a15ce5a8324ae85135f7c24` |
| `librand_chacha-64d149954d12c373.rlib` (0.3.1) | `ecacabbbd7349fbf5ee1003da66ca370d1106d1b19e82ea7680729a959039e9a` |
| `librand_core-f53120286c8e4486.rlib` (0.6.4) | `6075b60c4c4fc37c7ceb36b1d7b002b637d73210701b07830017ac98a62f7e4c` |

Building the committed lock reproduces, bit for bit, the exact `rand_chacha`
0.9.0 object the measurement executable links. That is the resolution proof F1
asks for.

The nine rlibs that differ are the six path-dependency crates
(`gf2_core`, `gf2_algebra`, `gf2_kernels_simd`, `gf2_kernels_hip`,
`permanent_wave_gpu`, `permanent_sampling_feas`), whose `-C metadata` hashes
change with the source path, and `serde`, `serde_core`, and `serde_json`.
The three serde rlibs keep identical metadata hashes and differ only because
`serde` and `serde_core` include a build-script-generated `private.rs` whose
absolute `OUT_DIR` path is embedded, and `serde_json` records the resulting
metadata hash of its dependency. Apart from that path and the archive member
size it shifts, their string tables agree.

### Retained build-tree evidence

The retained `CARGO_TARGET_DIR` at
`/data/gf2-campaigns/b8206228/premeasure-recovery-20260824/permanent-sampling-feas-hip`
holds the digest-matched executable and 43 dep-info files. Those files name both
RNG stacks — `rand_chacha-0.9.0` and `rand_core-0.9.5` alongside
`rand_chacha-0.3.1`, `rand-0.8.7`, and `rand_core-0.6.4` — confirming from the
build tree what the lock encodes and the linked symbols narrow.

## Lock evidence

| Path | SHA-256 | Lines |
|---|---|---|
| `dev/benchmarks/permanent_campaign/premeasure-v1-harness.Cargo.lock` | `70e08e7619268dfea5855dd629d049a76cb49aa00ad39cc317a4bde4086c0555` | 423 |

It is a byte copy of `dev/research/permanent-sampling-feas/Cargo.lock`, which
`.gitignore:3` and `dev/research/permanent-sampling-feas/.gitignore:2` both
exclude from tracking under the pattern `Cargo.lock`. The copy's basename does
not match that pattern, so this path is trackable;
`git check-ignore dev/benchmarks/permanent_campaign/premeasure-v1-harness.Cargo.lock`
exits 1.

`rand_chacha` 0.9.0 is the only stable 0.9 release in the crates.io index
(`~/.cargo/registry/index/index.crates.io-1949cf8c6b5b557f/.cache/ra/nd/rand_chacha`
lists `0.9.0` plus alpha and beta pre-releases, which a caret requirement does
not select), so `rand_chacha = "0.9"` admits exactly one resolution.

## Seed provenance, premeasure-v1

### Generator construction

At revision `1350d5b4`, `src/sampler.rs:232-247` assembles a 32-byte ChaCha20
seed from four little-endian `u64` words:

```
seed[ 0.. 8] = root                            campaign root seed
seed[ 8..16] = q                               field order, as a stream label
seed[16..24] = n                               matrix dimension
seed[24..32] = (purpose tag << 48) | index     stream address
```

The stream address packs a 16-bit purpose tag over a 48-bit index
(`src/sampler.rs:62-68`, `src/sampler.rs:199-221`). `MeasurementPurpose` assigns
fixed tags at `src/sampler.rs:120-134`; timed grid repetitions use `GridTimed`,
tag 4. The module documentation at `src/sampler.rs:44-47` states the bound this
encoding proves: distinct `(root, q, n, purpose, index)` tuples have distinct
seed addresses, without claiming the ChaCha20 output sequences are disjoint.

### Root seed and index allocation

`SEED_ROOT` is `0xB488_F02C_0000_0001` (`src/main.rs:41`). Each cell reserves
`INDICES_PER_CELL = 100_000` indices within each purpose (`src/main.rs:54`).
`execution_index_base` at `src/main.rs:447-473` gives a process's first index as

```
execution_id * active_grid_size * 100_000 + order_index * 100_000 + 1
```

and `src/main.rs:675-681` assigns it after the seeded Fisher-Yates shuffle and
the stable sort by ascending `n`. `active_grid_size` is the size of the
unfiltered grid for the requested orders, computed before `--only` narrows the
process to one cell (`src/main.rs:386-394`), so filtering cannot make two
execution ids collide. Timed repetitions open one stream per repetition at
`src/protocol.rs:766-772`; untimed warm-up repetitions use the separate
`GridWarmup` purpose at `src/protocol.rs:751-757`, so a wall-clock-dependent
warm-up count cannot shift the timed sample's addresses.

### Recovery from committed evidence

Each premeasure process runs with `--orders <n>`, which expands to 45 grid
specifications, and `--only q=…,n=…,backend=…`, which leaves one. The retained
scratch preambles record that width directly as
`active_grid_specs_per_execution: 45`. Every measured process's timed stream is
therefore

```
(0xB488F02C00000001, q, n, GridTimed, execution_id * 4_500_000 + 1)
```

Every term is recoverable from committed evidence:

- `execution_id` appears verbatim in `premeasure-v1-ledger.csv` columns
  `receipt_command` and `observed_invocation` as `--execution-id N`. Across all
  1,440 ledger rows it equals `schedule_position`, with no exception.
- `q` and `n` appear as `plan_q`/`plan_n` and `receipt_q`/`receipt_n`, which
  agree on all 1,440 rows.
- The root seed, the purpose tag, the per-cell reservation, and the index
  formula are source constants at the pinned revision `1350d5b4`, itself
  recorded per row in `session_source_revision` and `observed_git_sha`.

The formula is checked against the retained raw scratch CSVs, which record
`seed_root`, `timed_purpose`, `seed_index_first`, and `order_index` per
`src/protocol.rs:290-296`. All 1,439 retained processes that produced a data row
carry `seed_root = 0xb488f02c00000001`, `timed_purpose = grid_timed`,
`order_index = 0`, and a `seed_index_first` matching the formula exactly, with
1,439 distinct execution ids over `0..=1439` and no address collision. Schedule
position 539 is the signal-censored process; its committed receipt at
`premeasure-v1-censored/process-0539-3-26-A/receipt.txt` records
`--execution-id 539`, so its stream address is recoverable although it produced
no data row.

Row identity beyond the seed address is preserved by the ledger's
`process_index`, `schedule_position`, `config_id`, `config_code`,
`receipt_session_id`, `raw_file`, `receipt_file`, and `scratch_file` columns.

## Seed provenance and RNG identity, 296a41c9 frontier

The frontier receipt `backend-ordering.csv` records the seed provenance in full:
`seed_root`, `seed_stream_first`, `timed_stream_first`, `timed_stream_last`,
`reserved_stream_first`, `reserved_stream_last`,
`matrix_address_first`, and `matrix_address_last`. All 48 execution rows carry
`seed_root = 0xb488f02c00000001`, and each `matrix_address_*` value is the
4-tuple `(seed_root, q, n, stream_index)`. Each execution reserves 12,000,000
indices, `seed_stream_first` equals `execution_id * 120 * 100_000 + 1` on all 48
rows, and the timed block starts one index later. That matches the source at
revision `414d31f8`, where `STREAMS_PER_CELL` is `100_000`
(`src/main.rs:45`) and `GRID_SPECS_PER_EXECUTION` is
`QS.len() * NS.len() * (Backend::ALL.len() + 1)` (`src/main.rs:50`), which
evaluates to 120.

The gap F1 names for this cohort is the RNG implementation, and the two pinned
revisions supply it. `git cat-file -t` resolves both as commits.
`harness_source_sha` `414d31f8184a398deee946f151134511522dfca3` is the last
commit touching `dev/research/permanent-sampling-feas` as of the recorded
`git_sha`, which is the same commit; `deps_source_sha`
`d950bbb883845429d378aa2708ae7406b06fa6bc` is the last commit touching `crates/`
as of that revision. The premeasure ledger carries the same pair of identities
in `observed_harness_source_sha` (`03db1bbbd8ab1ca9f5bd38960796d12f25beeed5`) and
`observed_deps_source_sha` (`4a6006825e078d4e9babfe628aa8225887f10e7d`).

At revision `414d31f8` the harness is the same crate,
`dev/research/permanent-sampling-feas`. Its manifest requires
`rand_chacha = "0.9"` and `rand_core = "0.9"` at `Cargo.toml:49-50`; its
`[features]` block declares `hip = ["gf2-algebra/hip"]` at that revision,
matching the receipt's `build_features` value `hip`. The sampler imports
`rand_chacha::ChaCha20Rng` at `src/sampler.rs:56` and constructs it at
`src/sampler.rs:147` as `ChaCha20Rng::from_seed(derive_seed(root, q, n, stream))`.
`SEED_ROOT` is `0xB488_F02C_0000_0001` at `src/main.rs:32`, the value the
receipt records. The seed layout at this revision is the same four little-endian
words with a bare stream index in the fourth, which is why the receipt's
addresses are 4-tuples rather than the 5-tuples the premeasure scratch preamble
records.

The 0.8 RNG stack reaches this build too, by the same route:
`crates/gf2-core/Cargo.toml:41` at revision `d950bbb8` has
`default = ["rand", "io"]` against the workspace requirement `rand = "0.8"`.

Recovery for this cohort is by pinned-revision inspection. No Cargo.lock is
tracked at `414d31f8` — `git ls-tree 414d31f8 dev/research/permanent-sampling-feas/`
lists `.gitignore`, `Cargo.toml`, and `src` only — and no build tree for the
executable `6e24533cfbac987a0cec20af02f9dfb0a7bd9ce12c9e80cbdc00cad72150ccad`
is found under `/data`, so no rebuild proof is offered. The `rand_chacha`
version is nonetheless determined: `"0.9"` admits only 0.9.0. The `rand_core` patch
version is not determined by committed evidence, which is immaterial to the
generator identity, since `rand_core` supplies the `RngCore` and `SeedableRng`
traits while `rand_chacha` 0.9.0 produces every ChaCha20 output byte.

## Limits

- The premeasure executable's SHA-256 is not reproduced by rebuild, for the
  build-location reasons set out above. The dependency resolution the finding
  asks about is proved instead at per-crate granularity, where the retained and
  rebuilt `rand_chacha` 0.9.0 objects are byte-identical.
- The 48 frontier scratch raws named by `backend-ordering.csv` are not
  committed, so their bytes cannot be rehashed here. Their digests and full
  stream addresses remain recorded in that CSV.
- The frontier cohort's `rand_core` patch version is not recoverable from
  committed evidence.
