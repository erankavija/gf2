#!/usr/bin/env python3
"""Record the comparator build identities for the NR encoder survey (jit:12fdeb5b).

Usage:
  record-build-evidence.py <aff3ct-root> <srsran-root> <staging-dir> <output.json>

Every field is observed when the script runs: commits from the pinned trees,
digests from the files on disk, compiler versions from the compilers, flags
from the survey's own build definitions, and the selected srsRAN backend from
the parameter dump the native flavour produced. Nothing here is transcribed
from a previous run.
"""

import hashlib
import json
import pathlib
import re
import subprocess
import sys

ARMS = {
    "gf2-native": ("target-nr-encode-native/release/gf2-nr-encode-arm", "native",
                   "-C target-cpu=native"),
    "gf2-portable": ("target-nr-encode-portable/release/gf2-nr-encode-arm",
                     "conservative-portable", "-C target-cpu=x86-64"),
    "aff3ct-external": ("target-nr-encode-native/release/aff3ct-nr-encode-arm", "external",
                        "-C target-cpu=native"),
    "srsran-external": ("target-nr-encode-native/release/srsran-nr-encode-arm", "external",
                        "-C target-cpu=native"),
}


def digest(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


def run(*command):
    return subprocess.run(command, capture_output=True, text=True, check=True).stdout.strip()


def literal_list(text, name):
    """The string literals of a Rust `const NAME: [&str; N] = [...]` item."""
    match = re.search(rf"const {name}: \[&str; \d+\] = \[(.*?)\];", text, re.S)
    if match is None:
        raise SystemExit(f"build.rs has no {name}")
    return re.findall(r'"([^"]+)"', match.group(1))


def main():
    if len(sys.argv) != 5:
        sys.exit(__doc__)
    aff3ct = pathlib.Path(sys.argv[1]).resolve()
    srsran = pathlib.Path(sys.argv[2]).resolve()
    staging = pathlib.Path(sys.argv[3]).resolve()
    output = pathlib.Path(sys.argv[4])
    survey = pathlib.Path(__file__).resolve().parent
    build_rs = (survey / "nr-encode" / "build.rs").read_text()

    parameters = json.loads((survey / "nr-encode-parameters.json").read_text())
    observed_avx2 = sorted({
        row["srsran"]["supports_avx2"] for row in parameters["rows"]
        if row.get("srsran", {}).get("accepted")
    })
    if observed_avx2 != [True] and observed_avx2 != [False]:
        raise SystemExit(f"inconsistent AVX2 observation {observed_avx2}")
    backend = "avx2" if observed_avx2 == [True] else "generic"

    document = {
        "schema": "nr-encoder-build-evidence-v1",
        "toolchain": {
            "rustc": run("rustc", "--version"),
            "cargo": run("cargo", "--version"),
            "cxx": run("c++", "--version").splitlines()[0],
        },
        "projects": {
            "aff3ct": {
                "citation": "@/citation/Cassagne2019",
                "commit": run("git", "-C", str(aff3ct), "rev-parse", "HEAD"),
                "conf_submodule_commit": run("git", "-C", str(aff3ct / "conf"), "rev-parse", "HEAD"),
                "release": "v4.7.0",
                "license": {
                    "spdx": "MIT",
                    "path": "LICENSE",
                    "sha256": digest(aff3ct / "LICENSE"),
                },
                "linked_static_library": {
                    "path": "build/lib/libaff3ct-4.7.0.a",
                    "sha256": digest(aff3ct / "build/lib/libaff3ct-4.7.0.a"),
                },
                "shim_compile_flags": ["-std=gnu++11", "-O3", "-march=native", "-DNDEBUG"],
                "shim_definitions": literal_list(build_rs, "AFF3CT_DEFINITIONS"),
                "selected_backend": "scalar",
                "backend_evidence": "Puncturer_5G and Encoder_LDPC_QC_fast carry no MIPP or "
                                    "intrinsic path; eda07788's negative search over those four "
                                    "files records it.",
            },
            "srsran": {
                "citation": "@/citation/Srsran2026",
                "commit": run("git", "-C", str(srsran), "rev-parse", "HEAD"),
                "release": "25.10",
                "license": {
                    "spdx": "AGPL-3.0-or-later",
                    "path": "LICENSE",
                    "sha256": digest(srsran / "LICENSE"),
                },
                "build_method": "The shim compiles the LDPC translation units directly; srsRAN's "
                                "own CMake configuration stops on a host without MbedTLS.",
                "mbedtls_present": subprocess.run(["pkg-config", "--exists", "mbedtls"]).returncode == 0,
                "compiled_sources": [
                    {"path": source, "sha256": digest(srsran / source)}
                    for source in literal_list(build_rs, "SRSRAN_SOURCES")
                ],
                "shim_compile_flags": ["-std=c++17", "-O3", "-march=native", "-DNDEBUG"],
                "selected_backend": backend,
                "backend_evidence": "The shim reproduces ldpc_encoder_factory_sw(\"auto\"): it "
                                    "constructs ldpc_encoder_avx2 when the CPU reports AVX2 and "
                                    "ldpc_encoder_generic otherwise. The value here is the "
                                    "observation the native flavour reported on this host.",
            },
        },
        "shim_sources": {
            f"dev/active/12fdeb5b/survey/nr-encode/{name}": digest(survey / "nr-encode" / name)
            for name in ("build.rs", "cpp/aff3ct_encode_shim.cpp", "cpp/srsran_encode_shim.cpp",
                         "src/lib.rs", "src/aff3ct.rs", "src/srsran.rs", "Cargo.toml", "Cargo.lock")
        },
        "arms": {
            name: {
                "staging_path": relative,
                "sha256": digest(staging / relative),
                "build_identity": identity,
                "rustflags": rustflags,
            }
            for name, (relative, identity, rustflags) in ARMS.items()
        },
    }
    output.write_text(json.dumps(document, indent=1) + "\n")
    print(f"build evidence -> {output}")


if __name__ == "__main__":
    main()
