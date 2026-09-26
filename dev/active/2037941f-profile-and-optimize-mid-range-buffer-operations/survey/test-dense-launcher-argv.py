#!/usr/bin/env python3
"""Exercise the dense window launcher's log line without starting a window."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


SOURCE = Path(os.environ.get("DENSE_LAUNCHER_SOURCE", Path(__file__).with_name("run-dense-harness.sh")))
SURVEY = Path("dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey")


def fixture_launcher(root):
    """Run the real parser and log block with disposable preflight inputs."""
    script = SOURCE.read_text()
    parser_end = '    [[ -n "${family}" && -n "${addendum}" && -n "${run_id}" ]] || {'
    log_start = '    launch="${stage}.launcher.log"'
    assert script.count(parser_end) == script.count(log_start) == 1
    before, rest = script.split(parser_end, 1)
    _, after = rest.split(log_start, 1)
    script = before + '    stage="${REPO}/target/argv-fixture"\n    plan="${stage}.plan.json"\n' + log_start + after
    stop = '    } >>"${launch}"\n    echo "campaign execution log: ${stage}/execution.log" >&2'
    assert script.count(stop) == 1
    script = script.replace(stop, '    } >>"${launch}"\n    exit 0', 1)

    launcher = root / SURVEY / "run-dense-harness.sh"
    launcher.parent.mkdir(parents=True)
    launcher.write_text(script)
    launcher.chmod(0o755)
    for relative in (
        SURVEY / "dense-producing-inputs.json",
        Path("target/e1f9a78f-arms/release/dense-arm"),
        Path("target/e1f9a78f-scalar-arm/release/dense-arm"),
        Path("target/e1f9a78f-m4ri-arm/release/dense-m4ri-arm"),
        Path("target/argv-fixture.plan.json"),
    ):
        target = root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text("fixture\n")
    campaign = root / "target/e1f9a78f-arms/release/dense-campaign"
    campaign.write_text("#!/bin/sh\nprintf 'fixture-pin\\n'\n")
    campaign.chmod(0o755)
    tools = root / "home/.cargo/bin"
    tools.mkdir(parents=True)
    for name in ("git", "rustc"):
        tool = tools / name
        tool.write_text("#!/bin/sh\nprintf 'fixture-tool\\n'\n")
        tool.chmod(0o755)
    return launcher


class LauncherArgvTest(unittest.TestCase):
    def test_window_log_preserves_original_argument_boundaries(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            launcher = fixture_launcher(root)
            addendum = root / "an addendum with spaces.json"
            addendum.write_text("fixture\n")
            marker = root / "marker"
            environment = os.environ.copy()
            environment["HOME"] = str(root / "home")
            environment.pop("GF2_BENCH_WINDOW", None)
            for run_id in ("", f"run;$(touch {marker})'\\value", "line one\nline two"):
                with self.subTest(run_id=run_id):
                    args = ["window", "--family", "family name", "--addendum",
                            str(addendum), "--run-id", run_id, "--m4ri"]
                    result = subprocess.run(
                        [str(launcher), *args], cwd=root, env=environment,
                        text=True, capture_output=True, check=False,
                    )
                    self.assertEqual(result.returncode, 0, result.stderr)
                    log = (root / "target/argv-fixture.launcher.log").read_text()
                    command = log.splitlines()[0]
                    expected = subprocess.run(
                        ["bash", "-c", 'printf "# command:"; printf " %q" "$@"; printf "\\n"',
                         "bash", str(launcher), *args], cwd=root, env=environment,
                        text=True, capture_output=True, check=True,
                    ).stdout.rstrip("\n")
                    self.assertEqual(command, expected)
                    self.assertFalse(marker.exists())
                    (root / "target/argv-fixture.launcher.log").unlink()


if __name__ == "__main__":
    unittest.main()
