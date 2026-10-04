# 3fd3db5e renderer test baseline

Output of the BCH receipt renderer's test on its synthetic run: the result of
each test of `RenderReceiptTest`, then the receipt the renderer produces for
the test's default synthetic run directory. The raw capture is committed
beside this record.

Baseline source: `808f03aeb22871bef3bfb3cf33099549e13e8b4e`, the commit whose
renderer and test the baseline run executes.

## Command

Run from the repository root:

```
R=$(dirname "$(git ls-files ':(glob)**/3fd3db5e-baseline.md')")
T=$(git ls-files ':(glob)**/fd9d5416/tests/test_render_receipt.py')
{ python3 -B "$T" -v RenderReceiptTest 2>&1 | sed -E 's/ in [0-9.]+s$/ in <t>s/'
  python3 -B "$R/3fd3db5e-synthetic-receipt.py" "$T"; } > <capture-file>
```

`<t>` replaces the wall time of the test run. The header of
[3fd3db5e-synthetic-receipt.py](3fd3db5e-synthetic-receipt.py) states how it
renders the synthetic run and the one value it replaces.

## Raw output

| State | Capture | SHA-256 |
| --- | --- | --- |
| Baseline | [3fd3db5e-before.txt](3fd3db5e-before.txt) | `7ccd473fb633fffac8a5975dadd620e652f5e842361dfef7649cdfc0d105e53d` |

Two runs of the command at the baseline source produce the same bytes.
