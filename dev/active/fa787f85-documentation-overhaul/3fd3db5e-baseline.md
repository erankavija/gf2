# 3fd3db5e renderer test baseline

Output of the BCH receipt renderer's test on its synthetic run, before and
after the renderer and its test locate their committed inputs by content
identity: the result of each test of `RenderReceiptTest`, then the receipt the
renderer produces for the test's default synthetic run directory. The raw
capture is committed beside this record.

Baseline source: `fde2218e31f8d7f6c40072aabab4aba645f0b644`, the commit that adds this record and the
receipt printer; its renderer and test are those of its parent
`808f03aeb22871bef3bfb3cf33099549e13e8b4e` (`git diff` of the two commits
lists three added record files). After source:
`b42dc76c0d8dd89d58057deb61a9f38259feb845`.

## Command

Run from the repository root at each source state:

```
R=$(dirname "$(git ls-files ':(glob)**/3fd3db5e-baseline.md')")
T=$(git ls-files ':(glob)**/fd9d5416/tests/test_render_receipt.py')
{ python3 -B "$T" -v RenderReceiptTest 2>&1 | sed -E 's/ in [0-9.]+s$/ in <t>s/'
  python3 -B "$R/3fd3db5e-synthetic-receipt.py" "$T"; } > <capture-file>
```

`<t>` replaces the wall time of the test run. The header of
[3fd3db5e-synthetic-receipt.py](3fd3db5e-synthetic-receipt.py) states how it
renders the synthetic run and the one value it replaces.

## Output

| State | Capture | SHA-256 |
| --- | --- | --- |
| Baseline | [3fd3db5e-before.txt](3fd3db5e-before.txt) | `7ccd473fb633fffac8a5975dadd620e652f5e842361dfef7649cdfc0d105e53d` |
| After | the same bytes | `7ccd473fb633fffac8a5975dadd620e652f5e842361dfef7649cdfc0d105e53d` |

Two runs of the command at the baseline source produce the same bytes. At the
after source the command reproduces the committed capture; this prints nothing:

```
{ python3 -B "$T" -v RenderReceiptTest 2>&1 | sed -E 's/ in [0-9.]+s$/ in <t>s/'
  python3 -B "$R/3fd3db5e-synthetic-receipt.py" "$T"; } | diff - "$R/3fd3db5e-before.txt"
```

## Committed receipt

At each source state the renderer reproduces the committed fd9d5416 receipt
from its committed window run; this command prints nothing:

```
E=$(dirname "$(git ls-files ':(glob)**/*-fd9d5416-bch-performance-receipt.md')")
python3 -B "$(dirname "$(dirname "$T")")/render_receipt.py" "$E"/*-d1b4f85e-w1w2 | diff - "$E"/*-fd9d5416-bch-performance-receipt.md
```

## Repeat with the entries under the general BCH epic directory

Source: `3720a3304dd7be8cbefea5f617ada6230be3fd9e`, the
commit that moves the renderer, its test and the two smoke-run entries; the
renderer and the test are byte-identical renames.

| Part of the capture | Result against [3fd3db5e-before.txt](3fd3db5e-before.txt) |
| --- | --- |
| Test results of `RenderReceiptTest` | identical |
| Synthetic receipt | identical apart from three paths in lines 14 and 44 |
| Capture SHA-256 | `c0bd5c71fcd36267d35fa482d862dce87f63c68121937f91a5ff186cb7cece00` |

The renderer prints its own path, its protocol's path and the located mapping
document's path as it observes them at run time: line 14 holds the first two
and line 44 the third. The three paths differ from the baseline by the
directory level alone: with the directory above each entry removed from them,
this prints nothing:

```
{ python3 -B "$T" -v RenderReceiptTest 2>&1 | sed -E 's/ in [0-9.]+s$/ in <t>s/'
  python3 -B "$R/3fd3db5e-synthetic-receipt.py" "$T"; } |
  sed -E 's#(dev/active/)[^/`]+/(fd9d5416|591a1c5e)/#\1\2/#g' | diff - "$R/3fd3db5e-before.txt"
```

The command under *Committed receipt* prints the same two differences at lines
3 and 33 of the committed fd9d5416 receipt. The receipt keeps its bytes: the
two lines hold the paths the renderer observed at its run.
