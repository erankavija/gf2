#!/usr/bin/env bash
# Records the content identity of every arm this survey times (jit:26465e6c
# REQ-02).
#
# The benchmark runner pins its own producing inputs through
# `dev/active/f547c394/producing-inputs.json` and digests each arm executable at
# run time, but that manifest is the protocol tool's, not this survey's: it does
# not name the arm sources or the pinned external checkouts. This script closes
# that gap. It writes `arm-provenance.json`, which pins
#
#   - the upstream revisions the external arms are built from, verified against
#     the checkouts rather than quoted from a comment;
#   - the SHA-256 of every arm source, vendored upstream file and lock file;
#   - the SHA-256 and exact build command of every arm executable;
#   - the compiler and toolchain identities that produced them.
#
# A receipt's `arms[*].executable_sha256` must appear here; `verify` checks
# exactly that, so the chain runs from a committed receipt through this file to
# a pinned upstream commit.
#
# Usage:
#   dev/active/26465e6c/survey/record-provenance.sh            # write the file
#   dev/active/26465e6c/survey/record-provenance.sh verify DIR # check a receipt
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Paths recorded in the file are relative to the checkout this script runs in,
# which is a `git worktree` checkout for agent work and the main checkout
# otherwise. `.agents/ext` is untracked and lives only beside the main checkout,
# so it resolves through the shared git directory instead.
WORKTREE="$(git -C "${HERE}" rev-parse --show-toplevel)"
REPO="$(dirname "$(git -C "${HERE}" rev-parse --path-format=absolute --git-common-dir)")"
EXT="${GF2_SURVEY_EXT:-${REPO}/.agents/ext/26465e6c}"
OUT="${HERE}/arm-provenance.json"

if [[ "${1:-}" == "verify" ]]; then
  receipt_dir="${2:?usage: record-provenance.sh verify <receipt-directory>}"
  python3 - "${OUT}" "${receipt_dir}/receipt.json" <<'PY_VERIFY'
import json, sys
provenance_path, receipt_path = sys.argv[1:]
provenance = json.load(open(provenance_path))
known = {arm["executable_sha256"]: name for name, arm in provenance["arms"].items()}
receipt = json.load(open(receipt_path))
missing = []
for name, arm in receipt["arms"].items():
    digest = arm["executable_sha256"]
    if digest not in known:
        missing.append(f"{name} {digest}")
    else:
        print(f"{name}: {digest} -> {known[digest]}")
if missing:
    print("arm executables absent from arm-provenance.json: " + ", ".join(missing), file=sys.stderr)
    raise SystemExit(1)
print("every receipt arm executable is pinned in arm-provenance.json")
PY_VERIFY
  exit 0
fi

digest() { sha256sum "$1" | cut -d' ' -f1; }
relative() { python3 -c 'import os,sys; print(os.path.relpath(sys.argv[1], sys.argv[2]))' "$1" "${WORKTREE}"; }

libpopcnt_commit=$(git -C "${EXT}/libpopcnt" rev-parse HEAD)
libpopcnt_describe=$(git -C "${EXT}/libpopcnt" describe --tags --always)
mula_commit=$(git -C "${EXT}/mula-sse-popcount" rev-parse HEAD)

{
  echo '{'
  echo '  "schema": "gf2-survey-arm-provenance-v1",'
  echo '  "issue": "26465e6c",'
  printf '  "recorded_utc": "%s",\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo '  "host": {'
  printf '    "uname": "%s",\n' "$(uname -srm)"
  printf '    "cpu": "%s"\n' "$(awk -F': ' '/^model name/ { print $2; exit }' /proc/cpuinfo)"
  echo '  },'
  echo '  "toolchain": {'
  printf '    "rustc": "%s",\n' "$(rustc --version)"
  printf '    "cargo": "%s",\n' "$(cargo --version)"
  printf '    "cc": "%s",\n' "$(${CC:-gcc} --version | head -1)"
  printf '    "cxx": "%s"\n' "$(${CXX:-g++} --version | head -1)"
  echo '  },'
  echo '  "external_pins": {'
  echo '    "libpopcnt": {'
  echo '      "upstream": "https://github.com/kimwalisch/libpopcnt.git",'
  printf '      "commit": "%s",\n' "${libpopcnt_commit}"
  printf '      "describe": "%s",\n' "${libpopcnt_describe}"
  printf '      "header_sha256": "%s",\n' "$(digest "${EXT}/libpopcnt/libpopcnt.h")"
  echo '      "license": "BSD 2-Clause License",'
  echo '      "selected_backend": "libpopcnt internal CPUID dispatch; this Zen 3 host selects its AVX2 vpshufb path and never reaches the AVX-512 entry point (see objdump-evidence.txt section 7)"'
  echo '    },'
  echo '    "mula_sse_popcount": {'
  echo '      "upstream": "https://github.com/WojciechMula/sse-popcount.git",'
  printf '      "commit": "%s",\n' "${mula_commit}"
  echo '      "license": "BSD 2-Clause License",'
  echo '      "selected_backend": "popcnt_AVX2_harley_seal from popcnt-avx2-harley-seal.cpp; the 16-vector carry-save network engages at 512-byte blocks and the loads are vmovdqa (see objdump-evidence.txt section 8)",'
  echo '      "vendored": {'
  first=1
  for file in vendor/mula/popcnt-avx2-harley-seal.cpp vendor/mula/popcnt-lookup.cpp vendor/mula/LICENSE; do
    upstream="${EXT}/mula-sse-popcount/$(basename "${file}")"
    local_digest=$(digest "${HERE}/${file}")
    upstream_digest=$(digest "${upstream}")
    if [[ "${local_digest}" != "${upstream_digest}" ]]; then
      echo "vendored ${file} differs from the pinned upstream copy" >&2
      exit 1
    fi
    [[ ${first} -eq 1 ]] || echo ','
    first=0
    printf '        "%s": "%s"' "$(relative "${HERE}/${file}")" "${local_digest}"
  done
  echo
  echo '      }'
  echo '    }'
  echo '  },'
  echo '  "sources": {'
  first=1
  for file in wire_common.h splitmix64.h libpopcnt_arm.c mula_arm.cpp Makefile fetch-build.sh cross-check.sh observe-instructions.sh record-provenance.sh run-campaign.sh freeze-confirmatory.py render-findings-results.sh recount-flagged-windows.sh probe-dispatch.sh gf2-side/Cargo.toml gf2-side/Cargo.lock gf2-side/src/main.rs gf2-side/tests/correctness.rs; do
    [[ ${first} -eq 1 ]] || echo ','
    first=0
    printf '    "%s": "%s"' "$(relative "${HERE}/${file}")" "$(digest "${HERE}/${file}")"
  done
  echo
  echo '  },'
  echo '  "arms": {'
  echo '    "gf2-side": {'
  printf '      "executable": "%s",\n' "$(relative "${HERE}/gf2-side/target/release/popcount-gf2-side")"
  printf '      "executable_sha256": "%s",\n' "$(digest "${HERE}/gf2-side/target/release/popcount-gf2-side")"
  echo '      "build_command": "./scripts/cargo-budget.sh cargo build --release --manifest-path dev/active/26465e6c/survey/gf2-side/Cargo.toml",'
  echo '      "build_identity": "release profile with lto = true, codegen-units = 1, opt-level = 3; no RUSTFLAGS and no target-cpu, so the binary is baseline x86-64 and every vector path is reached through runtime detection",'
  echo '      "serves_arms": ["production-dispatch", "nibble-lut", "scalar-popcnt", "compiler-count-ones", "and-popcnt-fused", "and-popcnt-scalar-control", "and-popcnt-two-pass-consumer"]'
  echo '    },'
  echo '    "libpopcnt-arm": {'
  printf '      "executable": "%s",\n' "$(relative "${HERE}/libpopcnt-arm")"
  printf '      "executable_sha256": "%s",\n' "$(digest "${HERE}/libpopcnt-arm")"
  echo '      "build_command": "cc -O3 -march=native -Wall -Wextra -std=c11 -I<libpopcnt> -o libpopcnt-arm libpopcnt_arm.c",'
  echo '      "build_identity": "external: -march=native, and libpopcnt performs its own CPUID dispatch at run time",'
  echo '      "serves_arms": ["libpopcnt"]'
  echo '    },'
  echo '    "mula-avx2-harleyseal-arm": {'
  printf '      "executable": "%s",\n' "$(relative "${HERE}/mula-avx2-harleyseal-arm")"
  printf '      "executable_sha256": "%s",\n' "$(digest "${HERE}/mula-avx2-harleyseal-arm")"
  echo '      "build_command": "g++ -O3 -march=native -Wall -Wextra -std=c++17 -include cstdint -include immintrin.h -include vendor/mula/lookup-decl.h -I<mula> -Ivendor/mula -o mula-avx2-harleyseal-arm mula_arm.cpp vendor/mula/popcnt-lookup.cpp vendor/mula/popcnt-avx2-harley-seal.cpp",'
  echo '      "build_identity": "external: -march=native over the unmodified vendored reference implementation",'
  echo '      "serves_arms": ["mula-avx2-harleyseal"]'
  echo '    }'
  echo '  }'
  echo '}'
} >"${OUT}.tmp"
python3 -c 'import json,sys; json.load(open(sys.argv[1]))' "${OUT}.tmp"
mv "${OUT}.tmp" "${OUT}"
echo "wrote $(relative "${OUT}")" >&2
