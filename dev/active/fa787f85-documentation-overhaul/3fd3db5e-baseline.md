# 3fd3db5e renderer test baseline

Output of the BCH receipt renderer's test on its synthetic run, before and
after the renderer and its test locate their committed inputs by content
identity: the result of each test of `RenderReceiptTest`, then the receipt the
renderer produces for the test's default synthetic run directory. The raw
captures are committed beside this record.

Baseline source: `808f03aeb22871bef3bfb3cf33099549e13e8b4e`, the commit whose
renderer and test the baseline run executes. After source:
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

## Raw outputs

| State | Capture | SHA-256 |
| --- | --- | --- |
| Baseline | [3fd3db5e-before.txt](3fd3db5e-before.txt) | `7ccd473fb633fffac8a5975dadd620e652f5e842361dfef7649cdfc0d105e53d` |
| After | [3fd3db5e-after.txt](3fd3db5e-after.txt) | `7ccd473fb633fffac8a5975dadd620e652f5e842361dfef7649cdfc0d105e53d` |

Two runs of the command at the baseline source produce the same bytes.

## Equality

The captures are identical; this command prints nothing:

```
diff "$R/3fd3db5e-before.txt" "$R/3fd3db5e-after.txt"
```

## Committed receipt

At each source state the renderer reproduces the committed fd9d5416 receipt
from its committed window run; this command prints nothing:

```
E=$(dirname "$(git ls-files ':(glob)**/*-fd9d5416-bch-performance-receipt.md')")
python3 -B "$(dirname "$(dirname "$T")")/render_receipt.py" "$E"/*-d1b4f85e-w1w2 | diff - "$E"/*-fd9d5416-bch-performance-receipt.md
```
