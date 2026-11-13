# Body of a git filter-repo --commit-callback: rewords the subjects of commits
# on main that break the commit convention, keyed by their original ID.
SUBJECTS = {
    b"70ee11aba449cf4138651879e58962d148b69c34": b"chore(jit:66fefd9e): merge main",
    b"13ae12fec01f95edaa24493c3bb1c886ef788a69": b"fix(jit:da6200b8): keep glob segments out of the host-local rule",
    b"67e2f7aeef5e1ef7103c5fc974e2dbca4877b919": b"fix(jit:da6200b8): resolve link labels by their targets",
    b"5c7e98299270e739f97d1cc1fcfd1d4ab7c5959b": b"fix(jit:da6200b8): add writer helpers and the bare-name citation pass",
    b"572740e376bc83cf260695edf5a64ea237ea4fbf": b"fix(jit:da6200b8): checker rules C1-C4 with self-tests",
    b"cd9857b55af59506eb38306c77a4d01f7200c1b3": b"chore(jit:45ecf975): file 5f33fb28 and 66fefd9e, rule da6200b8",
    b"004fd17c5a49a5f06fe8ad580c7ae5e0859f5e70": b"docs(jit:45ecf975): reword nonconforming main subjects at the rewrite",
}
subject = SUBJECTS.get(commit.original_id)
if subject is not None:
    commit.message = subject + commit.message[commit.message.find(b"\n"):] if b"\n" in commit.message else subject + b"\n"
