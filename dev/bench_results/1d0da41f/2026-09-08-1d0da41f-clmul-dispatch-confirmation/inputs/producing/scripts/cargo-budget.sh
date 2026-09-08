#!/usr/bin/env bash
set -euo pipefail

# Host-wide CPU budget for one cargo invocation.
#
# Usage:
#   scripts/cargo-budget.sh <command> [args...]           # build, lint, format
#   scripts/cargo-budget.sh --test <command> [args...]    # test execution
#
# Two domains, because the two kinds of work have different constraints.
#
# Build, lint, and format work carries no wall-clock assertion, so it never
# queues. Each invocation claims one presence slot for its lifetime and sets
# `--jobs` to nproc divided by the number of claimed slots: a lone invocation
# gets the whole machine, K concurrent ones split it evenly, and total demand
# stays at nproc instead of K×nproc. `flock` releases on process death, so a
# crashed or killed invocation frees its slot with no cleanup.
#
# Test execution carries nextest's per-test wall-clock kill, which turns CPU
# contention into spurious FAILs rather than into slower passes: seven
# gf2-coding tests once timed out at 5.0-5.3 s against the 5 s cap while the
# same tests pass in ~1.0 s on an uncontended host. `--test` adds an exclusive
# lock so the invocation owns every core for its duration.
#
# Per-test thread bounds are not set here. RAYON_NUM_THREADS comes from
# `.cargo/config.toml`, so it reaches every cargo invocation in this repository
# whether or not it goes through this wrapper, and it pairs with
# `threads-required` in `.config/nextest.toml`. CARGO_CI_RAYON_THREADS
# overrides it for one invocation.
#
# Both domains are host-wide, not per-repository: the jit checkout competes for
# the same cores.
#
# The test lock keeps the path and override variable the jit checkout's own
# `scripts/cargo-ci.sh` uses, so the two repositories still exclude each other
# without either being updated. That copy holds the lock for its whole run
# while this one holds it only across test execution; the pairing stays correct
# because exclusion is what matters, not symmetry of the hold. Changing this
# path silently un-serializes the two repositories — jit's benchmark baselines
# pin `/tmp/cargo-ci.lock` and `dev/TESTING.md` documents
# CARGO_CI_BUILD_LOCK — so change both sides together or not at all.
#
# CARGO_CI_NO_LOCK=1 disables both domains (an isolated container that already
# owns the machine).
#
# A cargo command that skips this wrapper is unbudgeted and unserialized: it
# takes every core for its build and runs its tests against whatever else is on
# the machine. See the commands section of AGENTS.md.

TEST_DOMAIN=""
if [ "${1:-}" = "--test" ]; then
  TEST_DOMAIN=1
  shift
fi

if [ "$#" -eq 0 ]; then
  echo "usage: $0 [--test] <command> [args...]" >&2
  exit 2
fi

TEST_LOCK="${CARGO_CI_BUILD_LOCK:-${XDG_RUNTIME_DIR:-/tmp}/cargo-ci.lock}"
LOCK_DIR="${CARGO_CI_LOCK_DIR:-${XDG_RUNTIME_DIR:-/tmp}/cargo-ci-slots}"
MAX_RUNS="${CARGO_CI_MAX_RUNS:-6}"
NPROC=$(nproc)

[ -z "${CARGO_CI_RAYON_THREADS:-}" ] || export RAYON_NUM_THREADS="$CARGO_CI_RAYON_THREADS"

# Compilation cache. sccache caches rustc invocations content-addressed.
#
# Its hit rate is high and its benefit small, because the crates it cannot
# cache are the expensive ones. sccache hashes the whole CARGO_* environment
# namespace, and Cargo sets CARGO_MANIFEST_DIR and CARGO_MANIFEST_PATH per
# crate to absolute paths, so a workspace crate never hits from a different
# worktree while registry crates do.
#
# Measured 2026-08-31 on this host, full CI in a fresh worktree: the first
# build at a given feature set hit 1.7% (9 of 528) in 661 s; a second fresh
# worktree at the same flags hit 84.5% (470 of 556) in 635 s. That is 4% of
# wall clock for an 84.5% hit rate. Plan dispatch cost on the full build; a
# hardlinked target-directory pool is what avoids it, not this.
#
# It lives here rather than in `.cargo/config.toml` because a committed
# `rustc-wrapper` is a hard failure on a host without sccache, and the GitHub
# runners do not install it. An RUSTC_WRAPPER the caller already set is left
# alone; CARGO_CI_NO_SCCACHE=1 disables (the Codex sandbox denies sccache with
# EPERM).
if [ -z "${CARGO_CI_NO_SCCACHE:-}" ] && [ -z "${RUSTC_WRAPPER:-}" ] &&
   command -v sccache >/dev/null 2>&1; then
  export RUSTC_WRAPPER=sccache
fi

# Deprioritize the work so an interactive shell preempts it under contention.
# nice -n 19 = lowest CPU priority; ionice -c2 -n7 = best-effort lowest I/O
# priority (NOT the idle class -c3, which can be starved indefinitely). Both
# are best-effort: a missing binary degrades to running normally.
# CARGO_CI_NO_NICE=1 disables; CARGO_CI_NICE overrides the level.
PREFIX=()
if [ -z "${CARGO_CI_NO_NICE:-}" ]; then
  command -v nice   >/dev/null 2>&1 && PREFIX+=(nice -n "${CARGO_CI_NICE:-19}")
  command -v ionice >/dev/null 2>&1 && PREFIX+=(ionice -c2 -n7)
fi

if [ -n "${CARGO_CI_NO_LOCK:-}" ] || ! command -v flock >/dev/null 2>&1; then
  export CARGO_BUILD_JOBS="$NPROC" RUST_TEST_THREADS="$NPROC"
  exec "${PREFIX[@]}" "$@"
fi

mkdir -p "$LOCK_DIR"

# Compose with the benchmark mutex. `dev/scripts/ccx1-bench-flock.sh` takes this
# file exclusively; budget work takes it SHARED, so any number of builds run
# together while a `--full-host` preregistered measurement run excludes all of
# them for its duration. Without this a budget build starts underneath an
# evidence run and contaminates it, which no amount of documentation prevents.
#
# A run that already holds the exclusive side MUST set CARGO_CI_NO_LOCK=1 for
# its own cargo work: flock conflicts across open file descriptions regardless
# of process, so taking the shared side underneath your own exclusive side
# deadlocks against yourself. That is why the early-exit above precedes this.
#
# Bounded and announced for the same reason the test lock is, and it exits 75
# so `cargo-ci.sh` reports a queue timeout rather than a test failure.
CCX1_LOCK="${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}"
CCX1_TIMEOUT="${CARGO_CI_CCX1_TIMEOUT:-1800}"
exec {CCX1_FD}>"$CCX1_LOCK"
if ! flock -s -n "$CCX1_FD"; then
  echo "cargo-budget: a --full-host bench run holds $CCX1_LOCK; waiting up to ${CCX1_TIMEOUT}s" >&2
  if ! flock -s -w "$CCX1_TIMEOUT" "$CCX1_FD"; then
    echo "ERROR: cargo-budget: timed out after ${CCX1_TIMEOUT}s waiting for $CCX1_LOCK" >&2
    exit 75
  fi
fi

# Claims and counts both work by trying to lock each slot, so they are
# serialized against each other: an unserialized count momentarily holds every
# free slot it probes, and a concurrent count reads those as live invocations.
# Measured without this: three runs reported live counts of 4 and 6.
#
# Claim first, then count, inside one critical section — our own slot is held
# by then, so the count includes this invocation. An invocation that finds
# every slot taken holds none and is not counted; MAX_RUNS then floors the
# share. fd 9 is closed before the child starts.
exec 9>"$LOCK_DIR/probe.lock"
flock -x 9

SLOT_FD=""
for slot in $(seq 1 "$MAX_RUNS"); do
  exec {claim}>"$LOCK_DIR/slot.$slot"
  if flock -n "$claim"; then
    SLOT_FD=$claim
    break
  fi
  exec {claim}>&-
done

live=0
for slot in $(seq 1 "$MAX_RUNS"); do
  flock -n "$LOCK_DIR/slot.$slot" true 2>/dev/null || live=$((live + 1))
done
exec 9>&-

[ "$live" -gt 0 ] || live=1
jobs=$((NPROC / live))
[ "$jobs" -gt 0 ] || jobs=1
export CARGO_BUILD_JOBS="$jobs" RUST_TEST_THREADS="$jobs"
echo "cargo-budget: $jobs jobs ($live live, $NPROC cpus)" >&2

if [ -n "$TEST_DOMAIN" ]; then
  # `flock -o` is load-bearing: it closes the lock descriptor in the child
  # before exec. Without it every descendant inherits the descriptor, and a
  # descendant that daemonises keeps holding the lock after this invocation
  # exits — sccache double-forks to PPID 1 and does exactly that — so the next
  # caller blocks forever against work that finished long ago. Because the lock
  # is host-wide, a daemon leaked by either repository wedges the other one.
  #
  # The wait is bounded and announced: an unbounded silent block is
  # indistinguishable from a hung process, and `-E 75` separates "could not
  # acquire the lock" from the wrapped command's own exit status. Keep the
  # default below the cargo-ci gate's `timeout_seconds` so a run that loses the
  # queue reports a queue timeout instead of being killed as a gate failure.
  TEST_LOCK_TIMEOUT="${CARGO_CI_LOCK_TIMEOUT:-600}"
  if ! flock -n -o "$TEST_LOCK" true 2>/dev/null; then
    echo "cargo-budget: another test run holds the lock; waiting up to ${TEST_LOCK_TIMEOUT}s" >&2
  fi
  PREFIX=(flock -o -w "$TEST_LOCK_TIMEOUT" -E 75 "$TEST_LOCK" "${PREFIX[@]}")
fi

# The slot and CCX1 descriptors stay open in this process for the child's
# lifetime and are closed in the child, for the same daemon-inheritance reason
# as `flock -o`: a descendant that daemonises would otherwise hold them forever.
rc=0
if [ -n "$SLOT_FD" ]; then
  "${PREFIX[@]}" "$@" {SLOT_FD}>&- {CCX1_FD}>&- || rc=$?
else
  "${PREFIX[@]}" "$@" {CCX1_FD}>&- || rc=$?
fi
exit "$rc"
