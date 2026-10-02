# 5c0b9635 inventory judgements

- `dev/active/1a379447-zen3-cpu-performance`: owners f547c394, c077a88b and 3be770d5 sit under epics 1a379447 and d77176e5; 1a379447 owns most linked documents and names the entry, so its rows stay in place under epic 1a379447.
- `dev/active/3be770d5`: its only linking owner, 3be770d5, sits under both epics, so the linked-document count ties and neither epic id names the entry; epic 1a379447 holds the destination because the entry's harness crate is a Cargo path dependency of zen3 entries 07ca8585 and f63a2464 and depends on zen3 entry c077a88b.
- Each flat entry has a directory head row carrying its entry-level consumers: directory literals and relative constructs that resolve outside the entry (`parents[4]`, `../../../..`, Cargo path rewrites).
