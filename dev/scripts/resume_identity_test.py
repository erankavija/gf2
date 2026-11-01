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
        for name in ("resume_identity.py", "repository_files.py"):
            self.write_script(self.root / "dev/scripts" / name, (HERE / name).read_text())
        subprocess.run(["git", "-C", str(self.root), "add", "dev/scripts"], check=True)
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

    def test_older_identity_form_refuses_naming_a_new_log(self):
        self.write_older_record()
        original = self.log.read_text()
        result = subprocess.run(
            ["bash", str(self.launcher), *self.ARGUMENTS], cwd=self.root, env=self.environment,
            text=True, capture_output=True,
        )
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn("older identity form", result.stderr)
        self.assertIn("new log", result.stderr)
        self.assertEqual(self.log.read_text(), original)

    def test_changed_executable_refuses_resume(self):
        self.record(self.run_launcher())
        for path in self.executables():
            with self.subTest(executable=path.name):
                restore = self.checked((path, self.change_executable))
                self.run_launcher(accepted=False)
                restore()


class ClmulCrossoverProfile(LauncherContract, unittest.TestCase):
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
                ("emitted prose", self.launcher,
                 lambda path: replace(path, 'echo "complete" >&2', 'echo "finished" >&2')),
                ("calibration slope divisor", self.launcher,
                 lambda path: replace(path, "(double - single) / low", "(double - single) / (2 * low)"))]

    def write_older_record(self):
        conservative, native = self.executables()
        self.log.write_text(f"identity profile-arm-conservative={digest(conservative)} "
                            f"profile-arm-native={digest(native)} "
                            f"profile-cases.tsv={digest(self.cases)}\n"
                            f"session start {TIME} load=2.53 3.21 3.81\n")

class ConsumerProfile(LauncherContract, unittest.TestCase):
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
                ("emitted prose", self.launcher,
                 lambda path: replace(path, 'echo "profile written to', 'echo "profile stored in')),
                ("allocation trace session", self.launcher, lambda path: replace(
                    path, 'if [[ "${index}" == 1 ]]; then\n        trace=yes',
                    'if [[ "${index}" == 2 ]]; then\n        trace=yes'))]

    def test_session_heredoc_comment_keeps_identity(self):
        identity = self.run_launcher()
        self.record(identity)
        replace(self.survey / "sweep.sh", "import json, sys\n",
                "import json, sys  # Documentation only\n")
        self.assertEqual(self.run_launcher(), identity)

    def write_older_record(self):
        profile, _ = self.executables()
        self.log.write_text(f"rep-01 start {TIME} consumer-profile={digest(profile)} {LOAD}\n"
                            f"rep-01 done {TIME} {LOAD}\n")

class SteadyStateLdpcProfile(LauncherContract, unittest.TestCase):
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
                ("emitted prose", self.launcher,
                 lambda path: replace(path, 'echo "profile written to', 'echo "profile stored in')),
                ("awk program", self.survey / "profile-session.sh",
                 lambda path: replace(path, 'count[$0]++', 'count[$0] += 2')),
                ("case expansion heredoc", self.launcher,
                 lambda path: replace(path, 'spec["profile_arm"]', 'spec["executable"]')),
                ("series scope", self.survey / "profile-series.sh",
                 lambda path: replace(path, '"./${wrapper}" --full-host', '"./${wrapper}" --pinned'))]

    def test_awk_comment_keeps_identity(self):
        identity = self.run_launcher()
        replace(self.survey / "profile-session.sh", "                    END { for (key in count)",
                "                    # Documentation only\n                    END { for (key in count)")
        self.assertEqual(self.run_launcher(), identity)

    def write_older_record(self):
        profile, census = self.executables()
        fields = " ".join([
            f"ldpc-profile={digest(profile)}", f"ldpc-alloc-census={digest(census)}",
            *(f"{name}={digest(self.survey / name)}"
              for name in ("profile-session.sh", "profile-cases.tsv", "arms.json")),
        ])
        self.log.write_text(f"rep-01 start {TIME} {fields} load=[1.20 1.52 4.66]\n"
                            f"rep-01 done {TIME} load=[10.94 7.38 6.38]\n")

class DvbInterleaverProfile(LauncherContract, unittest.TestCase):
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
                ("sampling frequency", self.launcher,
                 lambda path: replace(path, "SAMPLE_FREQUENCY=4000", "SAMPLE_FREQUENCY=4001")),
                ("emitted prose", self.launcher,
                 lambda path: replace(path, 'echo "profile written to', 'echo "profile stored in')),
                ("counter selection", self.launcher,
                 lambda path: replace(path, "counters=yes", "counters=no"))]

    def write_older_record(self):
        (binary,) = self.executables()
        self.log.write_text(f"rep-01 start {TIME} dvb-profile={digest(binary)} "
                            f"inputs={UNRELATED} {LOAD}\n")


class EmbeddedPrograms(unittest.TestCase):
    PYTHON = "python3 - <<'PY'\nimport sys\nprint(1)\nPY\n"
    ARGUMENT = "python3 -c 'import sys\nprint(1)'\n"
    OTHER = "cat <<'EOF'\n# data\nvalue # note\nEOF\n"
    AWK = """awk -v x="1" '/#/ { n = 4 / 2 # note / more\n print "#", n }\n# whole line\nEND { print n }' file\n"""

    def canonical(self, text):
        return resume_identity.canonical_shell(text.encode())

    def test_python_comment_is_not_code(self):
        for program in (self.PYTHON, self.ARGUMENT):
            with self.subTest(program=program):
                self.assertEqual(
                    self.canonical(program),
                    self.canonical(program.replace("print(1)", "print(1)  # note\n# more")))

    def test_escaped_apostrophe_in_a_comment_is_not_code(self):
        comment = "print(1)  # caller'\\''s note"
        self.assertEqual(self.canonical(self.ARGUMENT),
                         self.canonical(self.ARGUMENT.replace("print(1)", comment)))
        awk = "awk '{ print 1 }' file\n"
        self.assertEqual(self.canonical(awk),
                         self.canonical(awk.replace("print 1 ", "print 1 # caller'\\''s note\n")))

    def test_code_after_an_escaped_apostrophe_is_code(self):
        program = "python3 -c 'print(\"it'\\''s\")\nprint(1)'\n"
        self.assertNotEqual(self.canonical(program),
                            self.canonical(program.replace("print(1)", "print(2)")))

    def test_python_change_is_code(self):
        for program in (self.PYTHON, self.ARGUMENT):
            with self.subTest(program=program):
                self.assertNotEqual(self.canonical(program),
                                    self.canonical(program.replace("print(1)", "print(2)")))

    def test_awk_comment_is_not_code_and_literals_stay(self):
        canonical = self.canonical(self.AWK)
        self.assertEqual(canonical, self.canonical(self.AWK.replace("# note / more", "# other")))
        self.assertEqual(canonical, self.canonical(self.AWK.replace("# whole line\n", "")))
        self.assertIn('/#/', canonical)
        self.assertIn('"#"', canonical)
        self.assertNotIn("note", canonical)
        self.assertNotEqual(canonical, self.canonical(self.AWK.replace("4 / 2", "4 / 3")))

    def test_other_embedded_language_compares_by_bytes(self):
        self.assertNotEqual(self.canonical(self.OTHER),
                            self.canonical(self.OTHER.replace("value # note", "value # other")))


if __name__ == "__main__":
    unittest.main()
