# 41e1d47d criteria proposal

Overhaul scope changes that carry the requirements of the A items in
`41e1d47d-disposition.md` not already present in an overhaul item's criteria.
Each change needs owner approval before `41e1d47d-apply.sh` runs.

## `fca578b1` Audit the permanent surface against the docs invariants

Append to `## Success Criteria`:

- [hard] REQ-05: Root `AGENTS.md` states `@/issue/<short-id>`, the form the mechanical documentation check resolves, as the way prose and Rustdoc reference a JIT issue, and every issue reference on the permanent surface uses that form.

## `ffc35b8c` Workspace source-comment tersification sweep

Append to `## Success Criteria`:

- [hard] REQ-28: Every Rust file under `dev/tools/` meets REQ-21, REQ-22 and REQ-23 through comment, Rustdoc and whitespace edits only.

No sweep unit covers `dev/tools/`, so REQ-28 needs a crediting unit. The unit
below follows the shape of the existing units. It depends on `62f0d0e6`, and
`030496bd` depends on it.

- Title: Tersify comments in the dev/tools crates
- Labels: `type:task`, `epic:documentation-overhaul`, `component:docs`,
  `story:source-tersification-sweep`, `satisfies:ffc35b8c/REQ-28`
- Gates: `cargo-ci`, `code-review`, `doc-review`, `docs-mechanical`
- Description:

~~~markdown
Bring the comments and Rustdoc in the `dev/tools/` crates to the workspace comment contract: current state only, non-obvious content only, shortest form.

## Background

Scope: Rust files under `dev/tools/`, about 1,800 comment lines on 2026-10-02. These sources are digest-pinned producing inputs of committed receipts, which verify through their snapshot copies. Comments citing an external standard, paper or committed receipt are evidence pointers and stay in shortened form.

## Success Criteria

- [hard] REQ-01: Each comment and Rustdoc block in scope meets the root `AGENTS.md` comment rule: nothing restates adjacent code, and each module-level doc is at most one short orientation paragraph plus the contract statements its public API needs.
- [hard] REQ-02: No comment in scope narrates history or process, or states planned, deferred, excluded or out-of-scope work; a current limitation is restated in present tense, and each removed planned-work statement is matched to or filed as a tracked issue named in the commit message.
- [hard] REQ-03: Each line in scope matching the sweep pattern below is removed or listed with a one-line justification in the commit message.

  ```text
  non-goal|out of scope|not in scope|future work|deferred to|follow-on|phase [a-e]\b|wave [a-z0-9]|(task|issue|story) `?[0-9a-f]{8}|previously|no longer|legacy|migrat
  ```

- [hard] REQ-04: Public API Rustdoc still states purpose, panics, safety conditions and non-obvious complexity; Rustdoc builds without new warnings and the Rust CI gate passes.
- [hard] REQ-05: The diff contains only comment, Rustdoc and whitespace edits, verified by an ignore-whitespace review of every hunk.
- [hard] REQ-06: The commit message states each `dev/tools/` crate's comment-line share before and after the change.
~~~
