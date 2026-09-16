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
./scripts/cargo-budget.sh cargo build --release --locked --manifest-path "${MANIFEST}" --bins
"${TARGET}/release/validate-permutation-equivalence"
"${TARGET}/release/dvb-profile" verify
printf 'xdsopl commit %s\n' "${PIN}"
for binary in dvb-profile-arm dvb-profile validate-permutation-equivalence; do
    sha256sum "${TARGET}/release/${binary}"
done
