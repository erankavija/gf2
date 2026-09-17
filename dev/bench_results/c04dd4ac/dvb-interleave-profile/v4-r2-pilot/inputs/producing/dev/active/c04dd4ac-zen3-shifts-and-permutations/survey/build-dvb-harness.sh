#!/usr/bin/env bash
# Build and non-timed validation of the DVB profile harness (jit:9fb40c83).
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${HERE}/../../../.." && pwd)"
MANIFEST="${REPO}/dev/active/eda07788/survey/gf2-side/Cargo.toml"
EXT="${GF2_9FB40C83_EXT:-${REPO}/.agents/ext/9fb40c83}"
PIN=32357d8ad55a6a302c34e093759f0454e45cca56
if [[ -n "${GF2_9FB40C83_XDSOPL_SOURCE:-}" ]]; then
    SOURCE="${GF2_9FB40C83_XDSOPL_SOURCE}"
else
    COMMON="$(git -C "${REPO}" rev-parse --path-format=absolute --git-common-dir)"
    PRIMARY="$(dirname "${COMMON}")"
    SOURCE=
    for candidate in "${PRIMARY}"/.agents/ext/*/xdsopl-ldpc; do
        if [[ -d "${candidate}/.git" ]] \
            && git -C "${candidate}" cat-file -e "${PIN}^{commit}" 2>/dev/null; then
            SOURCE="${candidate}"
            break
        fi
    done
    SOURCE="${SOURCE:-https://github.com/xdsopl/LDPC.git}"
fi
TARGET="${REPO}/target/9fb40c83-arms-native"

mkdir -p "${EXT}"
if [[ ! -d "${EXT}/xdsopl-ldpc/.git" ]]; then
    git clone "${SOURCE}" "${EXT}/xdsopl-ldpc"
fi
git -C "${EXT}/xdsopl-ldpc" checkout --detach "${PIN}"
[[ "$(git -C "${EXT}/xdsopl-ldpc" rev-parse HEAD)" == "${PIN}" ]]

export PATH="${HOME}/.cargo/bin:${PATH}"
export RUSTUP_TOOLCHAIN=1.95
export CARGO_CI_NO_SCCACHE=1
export CARGO_TARGET_DIR="${TARGET}"
export RUSTFLAGS="-C target-cpu=native"
export GF2_XDSOPL_INCLUDE="${EXT}/xdsopl-ldpc"
export GF2_XDSOPL_CXX_ARCH=native

cd "${REPO}"
RECORD=dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/harness-validation.txt

./scripts/cargo-budget.sh cargo build --release --locked --manifest-path "${MANIFEST}" --bins

# The survey harness is its own workspace, so the repository CI contract never
# reaches it and this command is the only thing that runs its contract checks:
# the frozen addendum, the producing closure, the projected plan, the fail-closed
# guards and the arm's mirror of the runner request wire.
CONTRACT="$(./scripts/cargo-budget.sh --test cargo test --release --locked \
    --manifest-path "${MANIFEST}" --tests 2>&1)"
echo "${CONTRACT}"
CONTRACT_PASSED="$(printf '%s\n' "${CONTRACT}" |
    awk '/^test result: ok\./ { total += $4 } END { print total + 0 }')"

VERSION="$(rustc --version | sed 's/^rustc //')"
HOST="$(rustc --version --verbose | sed -n 's/^host: //p')"
LLVM="$(rustc --version --verbose | sed -n 's/^LLVM version: //p')"

# The record is written from this run alone: the validators' own PASS lines, the
# toolchain and comparator this build used, and the executables it produced. It
# carries no clock reading, so a rerun on the same sources reproduces it byte
# for byte.
{
    echo '# DVB-T2 interleaver profile non-timed harness validation (jit:9fb40c83)'
    echo "# command: ${HERE#"${REPO}/"}/build-dvb-harness.sh"
    echo '# timing: none; the command builds release binaries and runs semantic validators only'
    echo "# rustc: ${VERSION}; LLVM ${LLVM}; ${HOST}"
    echo "# xdsopl: ${PIN}"
    echo "PASS survey harness contract: ${CONTRACT_PASSED} tests pass"
    "${TARGET}/release/validate-permutation-equivalence"
    "${TARGET}/release/dvb-profile" verify
    for binary in dvb-profile-arm dvb-profile validate-permutation-equivalence; do
        printf '# %s sha256: %s\n' \
            "${binary}" "$(sha256sum "${TARGET}/release/${binary}" | cut -d' ' -f1)"
    done
} >"${RECORD}"
cat "${RECORD}"
echo "harness validation record: ${RECORD}" >&2
