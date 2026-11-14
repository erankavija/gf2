#!/bin/sh
# RUSTC_WRAPPER for cargo-ci.sh: the sccache daemon a compile may spawn outlives
# the run, so it must not inherit the run's private TMPDIR, which the run deletes.
#
# The daemon starts each rustc without cargo's CARGO_MAKEFLAGS and with a
# jobserver of its own, so rustc's codegen threads would escape cargo's pool.
# When that pool is a named fifo, sccache is handed a generated `rustc` script
# that restores the pool's auth from GF2_CI_JOBSERVER (a variable sccache neither
# strips nor hashes) and execs the real rustc. The script's path is keyed by the
# real rustc's file identity, so each toolchain gets its own script. Any failure
# falls back to plain sccache.
TMPDIR=$CARGO_CI_SYSTEM_TMPDIR
export TMPDIR

plain() {
  [ -n "$1" ] && echo "sccache-system-tmp.sh: $1; rustc keeps sccache's jobserver" >&2
  shift
  exec sccache "$@"
}

case "${CARGO_MAKEFLAGS:-}" in
*--jobserver-auth=fifo:*) ;;
*) plain "" "$@" ;;
esac
[ "${1##*/}" = rustc ] || plain "" "$@"

real=$1
ident=$(stat -L -c '%d %i %s %Y' -- "$real" 2>/dev/null) ||
  plain "cannot stat $real" "$@"
key=$(printf '%s %s\n' "$real" "$ident" | cksum | cut -d' ' -f1)
dir=${CARGO_CI_SYSTEM_TMPDIR:-/tmp}/cargo-ci-rustc-shim-$(id -u)/$key
shim=$dir/rustc
if ! [ -f "$shim" ] || ! [ -x "$shim" ]; then
  mkdir -p -- "$dir" 2>/dev/null || plain "cannot create $dir" "$@"
  tmp=$(mktemp "$dir/.rustc.XXXXXX" 2>/dev/null) || plain "cannot create a file in $dir" "$@"
  quoted=$(printf '%s' "$real" | sed "s/'/'\\\\''/g")
  # shellcheck disable=SC2016 # the shim expands these at run time
  if ! printf '#!/bin/sh\n[ -n "${GF2_CI_JOBSERVER:-}" ] && export CARGO_MAKEFLAGS="$GF2_CI_JOBSERVER"\nexec '\''%s'\'' "$@"\n' \
      "$quoted" >"$tmp" 2>/dev/null ||
     ! chmod 755 "$tmp" 2>/dev/null ||
     ! mv -fT "$tmp" "$shim" 2>/dev/null; then
    rm -f "$tmp" 2>/dev/null
    plain "cannot install $shim" "$@"
  fi
fi
shift
GF2_CI_JOBSERVER=$CARGO_MAKEFLAGS exec sccache "$shim" "$@"
