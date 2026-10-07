# Leftover directories under dev/active (9451498f)

Git tracks files, so the checks run in a checkout; the removal changes no
tracked file.

| Criterion | Check, from the repository root | Result |
|---|---|---|
| REQ-01 | `find dev/active -type d -empty`; `find dev/active -type d -name __pycache__` | each prints nothing in the primary checkout |
| REQ-02 | `git show --stat` of the commit that adds this record | this record alone |
| REQ-03 | `python3 dev/active/fa787f85-documentation-overhaul/migration/check.py` | exit 0; the pending `deletion` rows are the checker and its self-tests, which @/issue/af84619e removes |
