# Superseded portable arms build record

This directory's build identity, smoke and validation records are superseded by
[`preparation-portable-build-tree/`](../preparation-portable-build-tree/), whose
`build-identity.json` names this directory's record under `supersedes`. The
portable executables carry their build path in their debug information, so the
digests recorded here reproduce only in the removed worktree that built them;
the replacement records the same arms built, smoked and validated in the story's
build tree.
