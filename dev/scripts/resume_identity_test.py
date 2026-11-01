#!/usr/bin/env python3
"""Resume behaviour of each profile launcher that shares `resume_identity.py`.

Every launcher exits before its first measurement, in a temporary checkout, over
stub executables; older records mirror the committed logs' shapes.
"""

import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import resume_identity  # noqa: E402

HERE = Path(__file__).resolve().parent
REPOSITORY = Path(subprocess.run(
    ["git", "-C", str(HERE), "rev-parse", "--show-toplevel"],
    capture_output=True, check=True, text=True,
).stdout.strip())
TIME = "2026-09-10T22:43:25Z"
LOAD = "load=[ 01:43:25 up 7 days,  3:12,  9 users,  load average: 2.27, 1.47, 1.27]"
UNRELATED = "a" * 64


def located(suffix):
    return REPOSITORY / subprocess.run(
        ["python3", "-B", str(HERE / "repository_files.py"), "file", suffix],
        cwd=REPOSITORY, capture_output=True, check=True, text=True,
    ).stdout.strip()


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def add_comment(path):
    """Add whole-line and trailing comments, or a description to a JSON document."""
    if path.suffix == ".json":
        document = json.loads(path.read_text())
        document["description"] = "Documentation only"
        path.write_text(json.dumps(document, indent=4))
        return
    lines = path.read_text().splitlines()
    insert_at = 1 if lines and lines[0].startswith("#!") else 0
    lines.insert(insert_at, "# Documentation only")
    if path.suffix == ".sh":
        lines = [line + "  # trailing" if line.startswith("set -") else line for line in lines]
    path.write_text("\n".join(lines) + "\n# Documentation only\n")


def replace(path, old, new):
    text = path.read_text()
    assert text.count(old) >= 1, old
    path.write_text(text.replace(old, new))


class LauncherContract:
    """The resume criteria every launcher meets; subclasses supply its fixture."""

    SUFFIX = ""
    STOP = ""
    STOP_WITH = 'echo "${IDENTITY}"; exit 0\n'
    ARGUMENTS = ("profile",)

    def setUp(self):
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        self.root = Path(scratch.name)
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        self.source = located(self.SUFFIX)
        self.survey = self.root / self.source.parent.relative_to(REPOSITORY)
        self.survey.mkdir(parents=True)
        self.launcher = self.survey / self.source.name
        text = self.source.read_text()
        self.assertEqual(text.count(self.STOP), 1)
        self.launcher.write_text(text.replace(self.STOP, self.STOP_WITH + self.STOP))
        self.write_script(self.root / "dev/scripts/resume_identity.py",
                          (HERE / "resume_identity.py").read_text())
        self.out = self.root / "profile"
        self.out.mkdir()
        self.environment = dict(os.environ, HOME=str(self.root / "home"), GF2_BENCH_WINDOW="1")
        self.build_fixture()
        for path in self.executables():
            self.write_script(path, "#!/bin/sh\nexit 0\n")
        self.after_executables()

    def write_script(self, path, text):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
        path.chmod(0o755)

    def copy(self, name):
        shutil.copyfile(self.source.with_name(name), self.survey / name)

    def after_executables(self):
        pass

    def change_executable(self, path):
        path.write_bytes(path.read_bytes() + b"# Different executable bytes\n")

    def run_launcher(self, accepted=True):
        result = subprocess.run(
            ["bash", str(self.launcher), *self.ARGUMENTS], cwd=self.root, env=self.environment,
            text=True, capture_output=True,
        )
        if accepted:
            self.assertEqual(result.returncode, 0, result.stderr)
            return self.identity(result)
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn("resume refused", result.stderr)
        return None

    def identity(self, result):
        return result.stdout.splitlines()[-1]

    def record(self, identity):
        """Append the launcher's form of a first-session record to its log."""
        with self.log.open("a") as log:
            log.write(f"rep-01 start {TIME} {identity} {LOAD}\n")

    def commit_inputs(self):
        git = ["git", "-C", str(self.root), "-c", "user.name=fixture",
               "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false"]
        subprocess.run([*git, "add", "-A", "--", "dev"], check=True)
        subprocess.run([*git, "commit", "-q", "-m", "fixture"], check=True)

    def checked(self, edit):
        """Apply `edit` to a file and restore its bytes afterwards."""
        path, change = edit
        original = path.read_bytes()
        change(path)
        return lambda: path.write_bytes(original)

    def test_comment_only_edit_keeps_identity(self):
        identity = self.run_launcher()
        self.record(identity)
        for path in self.comment_inputs():
            with self.subTest(input=path.name):
                restore = self.checked((path, add_comment))
                self.assertEqual(self.run_launcher(), identity)
                restore()

    def test_measured_change_refuses_resume(self):
        self.record(self.run_launcher())
        for name, path, change in self.measured_edits():
            with self.subTest(change=name):
                restore = self.checked((path, change))
                self.run_launcher(accepted=False)
                restore()

    def test_changed_executable_refuses_resume(self):
        self.record(self.run_launcher())
        for path in self.executables():
            with self.subTest(executable=path.name):
                restore = self.checked((path, self.change_executable))
                self.run_launcher(accepted=False)
                restore()


class OlderRecordContract:
    """Launchers whose older records name their script by digest."""

    def test_older_record_resumes_and_stays_in_its_form(self):
        older = self.write_older_record()
        self.assertEqual(self.run_launcher(), older)
        self.assertEqual(self.log.read_text().splitlines()[0].split(" load=")[0].split(" ", 3)[-1],
                         older)

    def test_older_record_resumes_after_comment_edit(self):
        older = self.write_older_record()
        self.commit_inputs()
        for path in self.comment_inputs():
            with self.subTest(input=path.name):
                restore = self.checked((path, add_comment))
                self.assertEqual(self.run_launcher(), older)
                restore()

    def test_older_record_refuses_measured_change(self):
        self.write_older_record()
        self.commit_inputs()
        for name, path, change in self.measured_edits():
            with self.subTest(change=name):
                restore = self.checked((path, change))
                self.run_launcher(accepted=False)
                restore()

    def test_older_record_refuses_changed_executable(self):
        self.write_older_record()
        for path in self.executables():
            with self.subTest(executable=path.name):
                restore = self.checked((path, self.change_executable))
                self.run_launcher(accepted=False)
                restore()

    def test_older_record_binds_settings_after_first_resume(self):
        self.write_older_record()
        self.commit_inputs()
        self.run_launcher()
        self.assertIn("configuration=", self.log.read_text())
        for name, path, change in self.measured_edits():
            with self.subTest(change=name):
                restore = self.checked((path, change))
                self.run_launcher(accepted=False)
                restore()

    def test_older_record_unknown_script_refuses(self):
        self.write_older_record(script_digest=UNRELATED)
        self.commit_inputs()
        self.run_launcher(accepted=False)


class NoScriptDigestContract:
    """Launchers whose older records name no script: their settings cannot be recovered."""

    def test_older_record_refuses(self):
        self.write_older_record()
        self.commit_inputs()
        self.run_launcher(accepted=False)


class ClmulCrossoverProfile(LauncherContract, NoScriptDigestContract, unittest.TestCase):
    SUFFIX = "53c5a8c0/survey/run-profile.sh"
    STOP = 'if [[ "${MODE}" == "row" ]]; then\n    :\n'

    def build_fixture(self):
        self.log = self.out / "profile.log"
        self.cases = self.survey / "profile-cases.tsv"
        self.cases.write_text('# Columns\nrow\tcell\tnative\tgf2\tauto\t{"seed":1}\n')
        (self.survey / "check-profile-cases.py").write_text("")

    def executables(self):
        return [self.root / f"target/53c5a8c0-arms-{variant}/release/profile-arm"
                for variant in ("conservative", "native")]

    def record(self, identity):
        self.assertEqual(self.log.read_text().splitlines(), [f"identity {identity}"])

    def comment_inputs(self):
        return [self.cases, self.launcher]

    def measured_edits(self):
        return [("case", self.cases, lambda path: replace(path, '"seed":1', '"seed":2')),
                ("sampling", self.launcher,
                 lambda path: replace(path, "SAMPLE_FREQUENCY=4000", "SAMPLE_FREQUENCY=4001")),
                ("record event literal", self.launcher,
                 lambda path: replace(path, "perf record -q -e cycles:u", "perf record -q -e instructions:u")),
                ("arm warmup literal", self.launcher,
                 lambda path: replace(path, "--warmup 0", "--warmup 2")),
                ("host scope literal", self.launcher,
                 lambda path: replace(path, '"${FLOCK}" --full-host', '"${FLOCK}" --pinned')),
                ("repetitions", self.launcher,
                 lambda path: replace(path, "REPETITIONS=9", "REPETITIONS=10"))]

    def write_older_record(self, script_digest=None):
        conservative, native = self.executables()
        older = (f"profile-arm-conservative={digest(conservative)} "
                 f"profile-arm-native={digest(native)} profile-cases.tsv={digest(self.cases)}")
        self.log.write_text(f"identity {older}\nsession start {TIME} load=2.53 3.21 3.81\n")
        return older


class ConsumerProfile(LauncherContract, OlderRecordContract, unittest.TestCase):
    SUFFIX = "04b85d10/survey/run-profile.sh"
    STOP = "# ------------------------------------------------------------- provenance\n"
    TABLES = ("counter-cases.txt", "report-cases.txt", "alloc-trace-cases.txt")

    def build_fixture(self):
        self.log = self.out / "repetitions.log"
        self.write_script(self.root / "scripts/cargo-budget.sh", "#!/bin/sh\nexit 0\n")
        self.copy("sweep.sh")
        for name in self.TABLES:
            (self.survey / name).write_text(f"# Fields\nrow|{name}|path|warm|1000\n")
        self.generator = self.survey / "profile-cases.py"
        self.generator.write_text('print(\'{"family":"fixture"}\')\n')

    def executables(self):
        return [self.root / f"target/consumer-profile-04b85d10/release/consumer-{name}"
                for name in ("profile", "verify")]

    def comment_inputs(self):
        return [self.survey / "sweep.sh", *(self.survey / name for name in self.TABLES),
                self.generator, self.launcher]

    def measured_edits(self):
        return [*((name, self.survey / name, lambda path: replace(path, "row|", "other|"))
                  for name in self.TABLES),
                ("session script", self.survey / "sweep.sh",
                 lambda path: replace(path, "SAMPLE_PERIOD=4000037", "SAMPLE_PERIOD=4000039")),
                ("session heredoc code", self.survey / "sweep.sh",
                 lambda path: replace(path, 'str(entry["target_ms"])', 'str(entry["target_ms"] + 1)')),
                ("ladder", self.generator, lambda path: replace(path, "fixture", "another")),
                ("host scope literal", self.launcher,
                 lambda path: replace(path, '"${FLOCK}" --full-host', '"${FLOCK}" --pinned')),
                ("allocation trace session", self.launcher,
                 lambda path: replace(path, "trace=yes", "trace=no")),
                ("repetitions", self.launcher,
                 lambda path: replace(path, "REPETITIONS=9", "REPETITIONS=10"))]

    def test_session_heredoc_comment_keeps_identity(self):
        identity = self.run_launcher()
        self.record(identity)
        replace(self.survey / "sweep.sh", "import json, sys\n",
                "import json, sys  # Documentation only\n")
        self.assertEqual(self.run_launcher(), identity)

    def write_older_record(self, script_digest=None):
        profile, verify = self.executables()
        older = f"consumer-profile={digest(profile)}"
        self.log.write_text(f"rep-01 start {TIME} {older} {LOAD}\nrep-01 done {TIME} {LOAD}\n")
        sources = "\n".join(f"{digest(self.survey / name)}  dev/active/04b85d10/survey/{name}"
                            for name in ("sweep.sh", *self.TABLES))
        (self.out / "host.txt").write_text(
            f"## harness executables\n{UNRELATED} consumer-arm\n{digest(profile)} consumer-profile\n"
            f"{digest(verify)} consumer-verify\n\n## harness sources\n{sources}\n"
            f"{UNRELATED}  dev/active/04b85d10/survey/profile-cases.py\n"
            f"{script_digest or digest(self.launcher)}  dev/active/04b85d10/survey/run-profile.sh\n"
        )
        with (self.out / "cases.jsonl").open("w") as ladder:
            subprocess.run(["python3", str(self.generator)], check=True, stdout=ladder)
        return older


class SteadyStateLdpcProfile(LauncherContract, NoScriptDigestContract, unittest.TestCase):
    SUFFIX = "3be770d5/survey/run-profile.sh"
    STOP = "# Summaries read files and time nothing, so they run after the mutex.\n"
    STOP_WITH = "exit 0\n"

    def build_fixture(self):
        self.log = self.out / "repetitions.log"
        for name in ("profile-series.sh", "profile-session.sh", "arms.json"):
            self.copy(name)
        self.cases = self.survey / "profile-cases.tsv"
        self.cases.write_text("# Columns\nstat\trow\tgf2-nms-f32\tdvb-t2-r12\tcore\t1\t2\n")
        shutil.copyfile(HERE / "repository_files.py", self.root / "dev/scripts/repository_files.py")
        self.write_script(self.root / "dev/scripts/ccx1-bench-flock.sh",
                          '#!/bin/sh\nmkdir -p "$4/stat"\necho 0 >"$4/stat/case.status"\n')
        self.build_identity = self.survey.parent / "preparation/build-identity.json"
        self.build_identity.parent.mkdir()
        (self.out / "host.txt").write_text("# Fixture host record\n")

    def executables(self):
        return [self.root / "target/ldpc-throughput/release" / name
                for name in ("ldpc-profile", "ldpc-alloc-census")]

    def pin_executables(self):
        self.build_identity.write_text(json.dumps({
            "executables": {path.name: digest(path) for path in self.executables()},
            "prepared_quality": ["target/quality/fixture.json"],
        }))

    def after_executables(self):
        self.pin_executables()
        subprocess.run(["git", "-C", str(self.root), "add", "-A", "--", "dev"], check=True)

    def change_executable(self, path):
        super().change_executable(path)
        self.pin_executables()

    def checked(self, edit):
        restore = super().checked(edit)

        def restored():
            restore()
            self.pin_executables()
        return restored

    def identity(self, result):
        starts = [re.match(r"rep-\d+ start \S+ (.*) load=", line).group(1)
                  for line in self.log.read_text().splitlines() if " start " in line]
        self.assertEqual(len(set(starts)), 1)
        return starts[0]

    def record(self, identity):
        self.assertIn(identity, self.log.read_text())

    def comment_inputs(self):
        return [self.survey / "profile-session.sh", self.cases, self.survey / "arms.json",
                self.launcher]

    def measured_edits(self):
        return [("case", self.cases, lambda path: replace(path, "core\t1", "core\t3")),
                ("session script", self.survey / "profile-session.sh",
                 lambda path: replace(path, "SAMPLE_PERIOD=4000037", "SAMPLE_PERIOD=4000039")),
                ("arm environment", self.survey / "arms.json",
                 lambda path: replace(path, '"GF2_AFF3CT_IMPLEM": "NMS"', '"GF2_AFF3CT_IMPLEM": "MS"')),
                ("host scope literal", self.survey / "profile-series.sh",
                 lambda path: replace(path, '"./${wrapper}" --full-host', '"./${wrapper}" --pinned')),
                ("case expansion heredoc", self.launcher,
                 lambda path: replace(path, 'spec["profile_arm"]', 'spec["executable"]')),
                ("repetitions", self.launcher,
                 lambda path: replace(path, "REPETITIONS=9", "REPETITIONS=10"))]

    def write_older_record(self, script_digest=None):
        profile, census = self.executables()
        older = " ".join([
            f"ldpc-profile={digest(profile)}", f"ldpc-alloc-census={digest(census)}",
            *(f"{name}={digest(self.survey / name)}"
              for name in ("profile-session.sh", "profile-cases.tsv", "arms.json")),
        ])
        self.log.write_text(f"rep-01 start {TIME} {older} load=[1.20 1.52 4.66]\n"
                            f"rep-01 done {TIME} load=[10.94 7.38 6.38]\n")
        return older


class DvbInterleaverProfile(LauncherContract, OlderRecordContract, unittest.TestCase):
    SUFFIX = "c04dd4ac-zen3-shifts-and-permutations/survey/run-profile.sh"
    STOP = 'if [[ ! -e "${OUT}/host.txt" ]]; then\n'

    def build_fixture(self):
        self.log = self.out / "repetitions.log"
        self.cases = self.survey / "profile-cases.txt"
        self.cases.write_text("# Cases\nqam16-direct\n")
        self.build_record = self.survey / "harness-validation.txt"
        self.manifest = self.survey / "dvb-producing-inputs.json"
        self.manifest.write_text('{"behavior_sources": ["source.rs"], "lifecycle_sources": []}\n')
        self.behavior_source = self.survey / "source.rs"
        self.behavior_source.write_text("fn behavior() {}\n")

    def executables(self):
        return [self.root / "target/9fb40c83-arms-native/release/dvb-profile"]

    def pin_executable(self):
        (binary,) = self.executables()
        self.build_record.write_text(f"# Build record\n# dvb-profile sha256: {digest(binary)}\n")

    def after_executables(self):
        self.pin_executable()

    def change_executable(self, path):
        super().change_executable(path)
        self.pin_executable()

    def checked(self, edit):
        restore = super().checked(edit)

        def restored():
            restore()
            self.pin_executable()
        return restored

    def comment_inputs(self):
        return [self.cases, self.build_record, self.manifest, self.behavior_source, self.launcher]

    def measured_edits(self):
        return [("case", self.cases, lambda path: replace(path, "direct", "packed")),
                ("record event literal", self.launcher,
                 lambda path: replace(path, "perf record -q -e cycles:u", "perf record -q -e instructions:u")),
                ("no-buildid literal", self.launcher,
                 lambda path: replace(path, "--no-buildid-cache", "--buildid-all")),
                ("session command", self.launcher,
                 lambda path: replace(path, '"${BIN}" session', '"${BIN}" counters')),
                ("counter selection", self.launcher,
                 lambda path: replace(path, "counters=yes", "counters=no")),
                ("counter events", self.launcher,
                 lambda path: replace(path, "COUNTER_EVENTS=cycles:u,", "COUNTER_EVENTS=")),
                ("sampling", self.launcher,
                 lambda path: replace(path, "SAMPLE_FREQUENCY=4000", "SAMPLE_FREQUENCY=4001")),
                ("repetitions", self.launcher,
                 lambda path: replace(path, "REPETITIONS=9", "REPETITIONS=10"))]

    def write_older_host(self):
        (binary,) = self.executables()
        (self.out / "host.txt").write_text(
            f"## executable\n{digest(binary)}  {binary}\n"
            f"## profile case declaration\n{digest(self.cases)}  {self.cases}\n"
        )

    def write_older_record(self, script_digest=None):
        (binary,) = self.executables()
        (self.out / "profile-inputs.sha256").write_text(
            f"{script_digest or digest(self.launcher)}  survey/run-profile.sh\n")
        older = f"dvb-profile={digest(binary)} inputs={UNRELATED}"
        self.log.write_text(f"rep-01 start {TIME} {older} {LOAD}\n"
                            f"rep-01 inner argv: run-profile.sh --session out rep-01 yes\n")
        self.write_older_host()
        return older

    def test_record_naming_no_script_refuses(self):
        (binary,) = self.executables()
        self.log.write_text(f"rep-01 start {TIME} dvb-profile={digest(binary)} {LOAD}\n")
        self.write_older_host()
        self.run_launcher(accepted=False)


class EmbeddedPrograms(unittest.TestCase):
    PYTHON = "python3 - <<'PY'\nimport sys\nprint(1)\nPY\n"
    ARGUMENT = "python3 -c 'import sys\nprint(1)'\n"
    OTHER = "cat <<'EOF'\n# data\nvalue # note\nEOF\n"

    def canonical(self, text):
        return resume_identity.canonical_shell(text.encode())

    def test_python_comment_is_not_code(self):
        for program in (self.PYTHON, self.ARGUMENT):
            with self.subTest(program=program):
                self.assertEqual(
                    self.canonical(program),
                    self.canonical(program.replace("print(1)", "print(1)  # note\n# more")))

    def test_python_change_is_code(self):
        for program in (self.PYTHON, self.ARGUMENT):
            with self.subTest(program=program):
                self.assertNotEqual(self.canonical(program),
                                    self.canonical(program.replace("print(1)", "print(2)")))

    def test_other_embedded_language_compares_by_bytes(self):
        self.assertNotEqual(self.canonical(self.OTHER),
                            self.canonical(self.OTHER.replace("value # note", "value # other")))


if __name__ == "__main__":
    unittest.main()
