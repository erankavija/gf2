# RNG provenance addendum to the three campaign manifests (2026-08-17)

The three preregistered campaign manifests record the measurement revision,
toolchain, host inventory, harness binary SHA-256, exact commands, and campaign
seed root, and they do not record the RNG algorithm or its version. This
addendum records that identity from the sources committed at the revisions those
campaigns pin. It introduces no measurement, and it edits none of the manifests
or receipts it supplements: they stand as published, and this document is the
committed record carrying what they omit.

## 1. What this supplements

| Manifest | Provenance block |
| --- | --- |
| [`../047b62ed/receipts.md`](../047b62ed/receipts.md) | §1, $\mathbb{F}_3$ campaign |
| [`../91605d4d/receipts.md`](../91605d4d/receipts.md) | §1, $\mathbb{F}_5$ campaign |
| [`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) | §1, $\mathbb{F}_7$ campaign |

## 2. The revisions each campaign pins

All three campaigns pin the same pair, each stating it in its own §1:

- **Measurement revision** — `292320d5e7d1fefd6b94262d0960e40c8ea35ba1`, the
  revision the measure run executes at.
- **Binary source revision** — `f9224650a780ea8ed98bcdff6e662cdb0d7b94f4`, the
  revision the four hash-pinned binaries are built at, one commit behind, with
  `tracked_worktree_dirty=false` at that moment. The intervening commit touches
  tracker state, two gate review prompts, and a package manifest, and no file
  under `crates/` or `dev/research/`
  ([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §1).

The generator sources named below are byte-identical at both revisions and in
the current worktree, so the identity is the same whichever of the two a reader
resolves against. `dev/research/permanent-sampling-feas/src/sampler.rs` is blob
`22299cbab4ee9062727a436020c709890e1a0698` and
`dev/research/permanent-sampling-feas/Cargo.toml` is blob
`236cd390143d268b0085000dffa3bda0beefbecc` at `f9224650`, at `292320d5`, and at
`HEAD` as this addendum is written.

## 3. The generator, at those revisions

**ChaCha20, as implemented by `rand_chacha` 0.9, through its `ChaCha20Rng`
type.** Four committed source facts fix it:

1. The sampler's own module documentation names it: "*The generator is ChaCha20
   as implemented by `rand_chacha` 0.9 (`ChaCha20Rng`)*"
   (`dev/research/permanent-sampling-feas/src/sampler.rs:33-34`).
2. That type is the one imported (`sampler.rs:58`) and the one every sampler
   instance is constructed from —
   `rng: ChaCha20Rng::from_seed(derive_seed(root, q, n, purpose, index))`
   (`sampler.rs:295`) — so no second generator reaches a campaign draw.
3. The dependency is declared with an explicit version requirement, under a
   comment naming it the campaign RNG: "*ChaCha20 is the named, versioned
   campaign RNG*", `rand_chacha = "0.9"`
   (`dev/research/permanent-sampling-feas/Cargo.toml:60-62`).
4. The seed is a 32-byte block assembled from
   $(\text{root}, q, n, \text{purpose}, \text{index})$ as four little-endian
   `u64` words (`sampler.rs:232-245`), which is the derivation the manifests'
   seed root and per-cell stream-index blocks feed.

## 4. What committed evidence does not fix: the exact patch version

The version requirement committed at both revisions is the Cargo semver
requirement `"0.9"`, which admits any `0.9.z`. **The resolved patch version is
not recoverable from committed evidence**, and this addendum does not assert
one:

- `dev/research/permanent-sampling-feas/Cargo.lock` is untracked by design —
  `dev/research/permanent-sampling-feas/.gitignore:2` ignores it — and
  `git ls-tree` at `f9224650` and at `292320d5` carries only `.gitignore`,
  `Cargo.toml`, and `src/` under that directory. No lockfile exists in the tree
  at either pinned revision.
- The lockfile present in a current worktree resolves `rand_chacha` to `0.9.0`
  with checksum
  `d3022b5f1df60f26e1ffddd6c66e8aa15de382ae63b3a0c1bfc0e4d3e3f325cb`. That file
  is an uncommitted local artifact produced after the fact, not a record of what
  the pinned build resolved, so it is reported here as what a reader will
  observe locally and is not evidence of the measurement's resolution.

The algorithm, the implementation crate, the type, the construction site, and
the seed derivation are therefore pinned by committed sources; the patch version
within `0.9.z` is not. Since ChaCha20 is a specified stream cipher and
`rand_chacha` holds its output stream stable across patch releases by its own
compatibility policy, a patch difference is not expected to change a draw — but
that is a property of the crate's policy rather than something this record
establishes, and it is stated as the open edge it is.

## 5. Why this is an addendum rather than an edit

The manifests are published artifacts of completed campaigns and are not
rewritten to add provenance discovered afterwards
(`@/inv/falsification-preserved`). The gap they carry is real and is stated
where it bites: a reader holding a manifest alone cannot name the generator, and
must reach the harness source at the pinned revision to do so. This addendum
closes that indirection for the three committed campaigns.

It does not discharge the forward requirement. A campaign run against the
production design of [`findings.md`](findings.md) §6.7 records the RNG algorithm
and version in the manifest itself, at the source that publishes the numbers,
rather than leaving a reader to reconstruct it — which is what
`@/inv/claims-trace-to-artifacts` asks of a committed artifact recording seeds,
revision, hardware, and toolchain. Committing the harness lockfile is the
smallest change that would make the patch version of §4 recoverable for a future
run.
