#!/usr/bin/env python3
"""Regenerate the committed steady-state profile series from their committed
chains and compare them with their committed summaries (jit:70a63a20).

Each series is the directory of a committed `attribution-chains.json` beside
the summarizer's `preparation` directory. It is copied to its own repository
path inside a scratch repository that holds the working-tree summarizer and a
link to that `preparation` directory, so the summary records the series path
the committed one records. The pinned executable is absent there, so the
summarizer reuses the committed chains.

`comparison.tsv` beside this script receives one line per record case,
`series<TAB>case<TAB>categories identical|different`, followed by one line
per remaining byte difference, `series<TAB>file<TAB>location`: a JSON path of
`profile-summary.json`, or a committed line number of a text file. The script
exits non-zero when a case's categories differ or the summarizer fails.
Usage: reproduce-steady-summaries.py
"""

import difflib
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile

HERE = pathlib.Path(__file__).resolve().parent
ROOT = pathlib.Path(subprocess.check_output(
    ["git", "-C", str(HERE), "rev-parse", "--show-toplevel"], text=True).strip())
OUTPUTS = ("profile-summary.json", "profile.md", "attribution.tsv", "attribution-chains.json")


def git(*args):
    return subprocess.check_output(["git", "-C", str(ROOT), *args], text=True).split("\n")[:-1]


def locate(suffix):
    locator = git("ls-files", "--", ":(glob)**/repository_files.py", ":(exclude,glob)**/inputs/**")[0]
    return pathlib.PurePosixPath(subprocess.check_output(
        [sys.executable, "-B", str(ROOT / locator), "file", suffix], cwd=ROOT, text=True).strip())


def json_differences(left, right, path=""):
    if isinstance(left, dict) and isinstance(right, dict):
        out = []
        for key in sorted(set(left) | set(right), key=str):
            if key not in left or key not in right:
                out.append(f"{path}/{key}")
            else:
                out += json_differences(left[key], right[key], f"{path}/{key}")
        return out
    if isinstance(left, list) and isinstance(right, list) and len(left) == len(right):
        return [d for index, (a, b) in enumerate(zip(left, right))
                for d in json_differences(a, b, f"{path}/{index}")]
    return [] if left == right else [path or "/"]


def line_differences(committed, regenerated):
    matcher = difflib.SequenceMatcher(None, committed, regenerated, autojunk=False)
    out = []
    for tag, first, last, other_first, other_last in matcher.get_opcodes():
        if tag == "equal":
            continue
        if last == first:
            out.append(f"after line {first}, {other_last - other_first} regenerated lines")
        else:
            out.append(f"line {first + 1}" if last == first + 1 else f"lines {first + 1}-{last}")
    return out


def regenerate(summarizer, preparation, series, scratch):
    subprocess.run(["git", "init", "-q", str(scratch)], check=True)
    survey = scratch / "survey"
    survey.mkdir()
    for name in ("summarize-profile.py", "intervals.py"):
        shutil.copy2(ROOT / summarizer.parent / name, survey / name)
    (scratch / preparation).parent.mkdir(parents=True)
    (scratch / preparation).symlink_to(ROOT / preparation)
    shutil.copytree(ROOT / series, scratch / series)
    target = scratch / series
    return subprocess.run([sys.executable, "-B", str(survey / "summarize-profile.py"), str(target),
                           "--json", str(target / "profile-summary.json"),
                           "--markdown", str(target / "profile.md")],
                          stdout=subprocess.DEVNULL).returncode == 0


def main():
    summarizer = locate("3be770d5/survey/summarize-profile.py")
    preparation = locate("3be770d5/preparation/build-identity.json").parent
    series_list = sorted(pathlib.PurePosixPath(chains).parent for chains in git("ls-files", "*/attribution-chains.json")
                         if pathlib.PurePosixPath(chains).parent.parent == preparation.parent)
    lines, status = [], 0
    for series in series_list:
        with tempfile.TemporaryDirectory() as directory:
            scratch = pathlib.Path(directory)
            if not regenerate(summarizer, preparation, series, scratch):
                sys.exit(f"{series}: the summarizer failed")
            committed = {name: (ROOT / series / name).read_bytes() for name in OUTPUTS}
            regenerated = {name: (scratch / series / name).read_bytes() for name in OUTPUTS}
        old, new = (json.loads(files["profile-summary.json"]) for files in (committed, regenerated))
        for case in old["records"]:
            same = all(old["records"][case].get(key) == new["records"].get(case, {}).get(key)
                       for key in ("total_samples", "categories"))
            status |= not same
            lines.append(f"{series}\t{case}\tcategories {'identical' if same else 'different'}")
        for name in OUTPUTS:
            if committed[name] == regenerated[name]:
                continue
            if name.endswith(".json"):
                locations = json_differences(json.loads(committed[name]), json.loads(regenerated[name]))
            else:
                locations = line_differences(committed[name].decode().splitlines(),
                                             regenerated[name].decode().splitlines())
            lines += [f"{series}\t{name}\t{location}" for location in locations or ["bytes"]]
    output = HERE / "comparison.tsv"
    with tempfile.NamedTemporaryFile("w", dir=HERE, delete=False, encoding="utf-8") as handle:
        handle.write("".join(line + "\n" for line in lines))
    os.replace(handle.name, output)
    sys.exit(status)


if __name__ == "__main__":
    main()
