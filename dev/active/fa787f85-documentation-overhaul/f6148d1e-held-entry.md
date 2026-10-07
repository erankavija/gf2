# Held entry dev/active/dbd8787d (f6148d1e)

Issue f6148d1e keeps the flat entry `dev/active/dbd8787d/` in place (DEC-02 of
f6148d1e); issue 82ffd7b8 tracks the move. No file of the entry moves or changes.

| Criterion | Evidence | Establishes |
|---|---|---|
| REQ-01 | `jit issue show dbd8787d --json` on main prints state `done`; `git ls-files dev/active/dbd8787d` lists the three files | The owning issue is terminal and the entry is whole |
| REQ-02 | Facts below; manifest rows under `dev/active/dbd8787d/` are `retained-operational`, empty destination, `complete`; 82ffd7b8 holds the move | In-place retention is recorded and tracked |
| REQ-03 to REQ-05 | The commits of f6148d1e change the manifest rows and add the files of this record | No consumer code, script path literal or digest-pinned file changes; no moved entry binds them |
| REQ-06 | Baseline table of [8f61d6de-baseline-findings.md](8f61d6de-baseline-findings.md): `dev/active/dbd8787d/premeasurement-protocol.md:10` is its unresolved link | The link stays baselined; `616e1d7c-preexec.json` pins the file |
| REQ-07 | `python3 dev/active/fa787f85-documentation-overhaul/migration/check.py` exits 0; `CARGO_CI_STEPS='^tuning-campaign-validator$' ./scripts/cargo-ci.sh` passes | Rows complete; tracker references unchanged. The full Rust CI verdict is the gate's |

## Validator facts

Output of `python3 dev/active/fa787f85-documentation-overhaul/f6148d1e-held-facts.py <stage> <repository>`:
[f6148d1e-held-facts.txt](f6148d1e-held-facts.txt). Checks of `dev/scripts/validate-tuning-extent-campaign.py`.

| Fact | Check | Output section |
|---|---|---|
| One byte-distinct declaration per issue across live and archived copies | `locate_declaration` | `distinct`: a path-edited copy yields `2 campaign declarations name issue dbd8787d; exactly one must` |
| The live declaration's digest equals an entry of the stage revision's lifecycle digests | `recorded_declaration`, called by `validate_stage` and `expected_evidence_index` | `digest`: the edited copy fails the `lifecycle manifest` identity check |

The `verdicts` section holds the `--self-test`, `--receipt-check` and `--publication` verdicts of the unmoved tree.
