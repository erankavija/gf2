"""Projection of a build record onto the closed identity set of one arm."""


def arm_build(build, name, executable=None):
    """Executable digest; for a workspace arm also compiler options, toolchain, build flags.

    `executable` names another executable of a workspace arm's package in place of the arm's.
    Source and link input hashes, file lists, paths and whole-file digests are outside the set.
    """
    row = build["arms"][name]
    if executable is None:
        executable, digest = row["executable"], row["executable_sha256"]
    else:
        digest = build["executables"][executable]["sha256"]
    identity = {"executable_sha256": digest}
    if row["source"] != "workspace":
        return identity
    return {**identity,
            "compiler_options": build["executables"][executable]["compiler_options"],
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
