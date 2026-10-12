"""Projection of a build record onto the closed identity set of one arm."""


def arm_build(build, name):
    """Executable digest, and for a workspace arm its compiler options, toolchain and flags.

    Source and link input hashes, file lists, paths and whole-file digests are outside the set.
    """
    row = build["arms"][name]
    identity = {"executable_sha256": row["executable_sha256"]}
    if row["source"] != "workspace":
        return identity
    return {**identity,
            "compiler_options": build["executables"][row["executable"]]["compiler_options"],
            "rustc": build["rustc"], "cargo": build["cargo"],
            "release_profile": build["release_profile"],
            "builds": [{"rustflags": command["environment"].get("RUSTFLAGS"),
                        "release": "--release" in command["argv"]}
                       for command in build["build_commands"]]}
