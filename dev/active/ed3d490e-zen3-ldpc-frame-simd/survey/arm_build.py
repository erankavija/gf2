"""Projection of a build record onto the closed identity set of one arm."""


def arm_build(build, name):
    """Executable digest; for a workspace arm also compiler options, toolchain, build flags.

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
                       for command in producing(build["build_commands"], row["package"])]}


def producing(commands, package):
    """The commands that build `package`: those naming it, else those naming no package."""
    def named(command):
        argv = command["argv"]
        return {argv[index + 1] for index, word in enumerate(argv[:-1]) if word == "--package"}

    return ([c for c in commands if package in named(c)]
            or [c for c in commands if not named(c)])
