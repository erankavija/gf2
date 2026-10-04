# DVB-T2 parity structure amendment record

[table_interpretation.md](table_interpretation.md) describes how the
`@/citation/Etsi2015` tables expand into a parity-check matrix. This record
supersedes the sub-diagonal wrap in its section "Dual-Diagonal Parity
Structure":

- "Parity bit p connects to check equation (p-1) mod m"
- "The sub-diagonal wraps: parity bit 0 connects to check equation (m-1)."
- the "Visual Representation" diagram, whose row $c_0$ carries a 1 in column
  $p_{m-1}$
- "Each parity check involves exactly 2 parity bits (sparse)"

Benchmark receipts pin the note by digest, so the note keeps its bytes and
this record carries the structure.

## Structure

With $k$ information bits and $m$ parity bits, column $k + j$ of $H$ is parity
bit $j$. The parity part $P \in \mathbb{F}_2^{m \times m}$ of
$H = [\,A \mid P\,]$ is

$$
P_{i,j} =
\begin{cases}
1 & \text{if } j = i \text{ or } j = i - 1, \\
0 & \text{otherwise,}
\end{cases}
\qquad 0 \le i, j < m .
$$

## Enforcement

- `build_dvb_edges` in [builder.rs](builder.rs) adds edge $(p, k + p)$ for
  every $p$ and edge $(p, k + p - 1)$ for $p > 0$.
- `LdpcEncoder::has_dual_diagonal_parity` in [core.rs](../core.rs) accepts
  only a matrix whose parity part equals $P$, and `LdpcEncoder::is_ira`
  reports that acceptance.
- `test_ira_encoder_selected_for_all_short_rates` and
  `test_ira_encoder_selected_for_all_normal_rates` in
  [ira_encoder_correctness.rs](../../../tests/ira_encoder_correctness.rs)
  assert `is_ira` for `LdpcCode::dvb_t2_short` and `LdpcCode::dvb_t2_normal`
  at each rate of `ALL_RATES`.
