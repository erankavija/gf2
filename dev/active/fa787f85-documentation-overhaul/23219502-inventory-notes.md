# 23219502 inventory notes

- `c077a88b`: `findings.md` is linked by c077a88b, which sits under epics 1a379447 and d77176e5, and by a203a23c, which sits under 1a379447 only. Counted by linked document the epics tie and neither id names the entry; 1a379447 holds two of the three document references, so the entry goes under epic 1a379447. Ownership of this destination is uncertain.
- `f547c394`: linking owners f547c394 (both epics), bdc507a3, 2c487595, f63a2464 and 1a379447 put 23 linked documents under 1a379447 and 17 under d77176e5, so the entry goes under epic 1a379447.
- Each entry has a directory head row that carries its entry-level consumers: directory literals, fixture paths under the entry, and relative paths that resolve to an entry directory. A file row carries the consumers that resolve to that file and its own depth-relative lines (`../..`, `parents[n]`).
- `digest_pinned` marks a file whose SHA-256 a committed JSON record keeps beside its path (receipts, plans, producing manifests, snapshot inputs) or that has a snapshot copy under a `dev/bench_results` receipt's `inputs/`.
