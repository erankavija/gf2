# LDPC decoder numerical contract

This page states the floating-point rules of the flooding min-sum LDPC decoder
of `gf2-coding` and the clauses that bind a decoder carrying several frames at
once. The canonical decoder is `gf2_coding::ldpc::LdpcDecoder`, defined in
[`../../crates/gf2-coding/src/ldpc/core.rs`](../../crates/gf2-coding/src/ldpc/core.rs), over the canonical edge
indexing `gf2_coding::ldpc::EdgeLayout`. A decoder, kernel or entry point of
the min-sum family conforms when its observable results equal the canonical
decoder's under these rules.

## Scope

- **Precision.** The supported LLR precision is `f32`: `gf2_coding::llr::Llr`
  wraps one `f32`, and every message, belief and posterior is an `Llr`.
- **Update rules.** The contract covers `DecoderAlgorithm::MinSum`,
  `DecoderAlgorithm::NormalizedMinSum` and `DecoderAlgorithm::OffsetMinSum`.
- **Sum-product.** A frame decoded under `DecoderAlgorithm::SumProduct` takes
  the single-frame decoder, whose check update is the `tanh`/`atanh` box-plus
  of `Llr::boxplus_n`. The check-update rules below do not describe it.
- **Schedule.** The schedule is flooding: one iteration updates every check
  from the variable-to-check messages of the preceding iteration, then every
  variable from the resulting check-to-variable messages.
- **Outside the contract.** Quantized message alphabets and layered schedules
  are outside the contract. The layered quantized decoder has its own rules,
  stated in [Layered quantized decoder](#layered-quantized-decoder).
- **Iteration cap.** The rules hold for every iteration cap. At a cap of zero
  no iteration runs: the posterior equals the frame's channel LLRs, the
  decoded codeword is their hard decision, `iterations` is 0, and
  `syndrome_check_passed` and `converged` both equal the syndrome test of that
  word.

## Notation

Edge $e$ joins check $c$ and variable $v$. $L_v$ is the channel LLR of
variable $v$, $\mu_e$ the variable-to-check message and $\lambda_e$ the
check-to-variable message on $e$, and $B_v$ the belief of $v$. Positive values
favour bit 0. A check's edges are ordered by `EdgeLayout::check_range` and a
variable's by `EdgeLayout::var_range`; both orders list a node's neighbours by
ascending index.

## Check-node update

The canonical decoder writes each check's outgoing messages through
`gf2_coding::ldpc::min_sum_check_row`
([`../../crates/gf2-coding/src/ldpc/min_sum.rs`](../../crates/gf2-coding/src/ldpc/min_sum.rs)). For the output
on edge $e$ of check $c$, the *other inputs* are the messages $\mu_{e'}$ on
the check's edges $e' \neq e$.

### Sign rule

The sign of an input is taken by the comparison $\mu \geq 0$.

- A positive-zero and a negative-zero input both count as positive.
- A NaN input counts as negative.

The output sign $s_e \in \{+1, -1\}$ is the product of the signs of the other
inputs.

### Magnitude and tie rule

The magnitude $m_e$ of an output is the minimum of $|\mu_{e'}|$ over the other
inputs.

- A candidate replaces the running smallest, and the running second smallest,
  only under a strict comparison.
- Among equal smallest magnitudes the first position in the check's edge order
  is the smallest position, and the equal value is the second smallest.
- The output at the smallest position takes the second smallest magnitude;
  every other output takes the smallest. Under a tie both are the same value.
- A NaN magnitude never satisfies the comparison and is skipped. An output
  whose other inputs are all NaN has magnitude $+\infty$.

### Scaling of each rule

| `MinSumRule` | Output $\lambda_e$ | Parameter domain |
|---|---|---|
| `Plain` | $s_e \cdot m_e$ | none |
| `Normalized(alpha)` | $\alpha \cdot (s_e \cdot m_e)$ | $\alpha$ finite, $0 < \alpha \leq 1$ |
| `Offset(beta)` | $s_e \cdot \max(m_e - \beta,\ 0)$ | $\beta$ finite, $\beta \geq 0$ |

The operations are evaluated in the written order in `f32`. The parameter
domains are enforced by `DecoderConfig::new`. An output can be negative zero;
the sign rule reads it as positive at the next check update.

### Low-degree checks

The single output of a degree-one check is the zero LLR. A degree-zero check
writes nothing.

### Independence from SIMD dispatch

`min_sum_check_row` is scalar and performs no runtime dispatch, so the rules
hold for every setting of the `simd` cargo feature and every host capability.
The AVX2 kernel of `Llr::boxplus_minsum_n`, which the decoder does not reach,
follows the same sign and magnitude rules in its vector lanes and its
remainder.

The explicitly constructed `QcDecoder`
([`../../crates/gf2-coding/src/ldpc/qc.rs`](../../crates/gf2-coding/src/ldpc/qc.rs)) applies these same rules
with lanes representing lifted checks within one frame. The outputs of the
check rule do not depend on the order of a check's edges, since tied magnitudes
are equal values; a masked circulant entry carries a $+\infty$ input, which the
sign and magnitude rules skip. A check row forms each input as the previous
iteration's belief minus the edge's previous check-to-variable message, the
`f32` subtraction that defines the variable-to-check message, so no message
plane of that direction is stored. Each variable accumulates in its canonical
order: a column block's lanes split into segments that share one order of
parity checks. [`ldpc_qc_contract`](../../crates/gf2-coding/tests/ldpc_qc_contract.rs)
compares posterior bits, words and outcomes with the canonical decoder on both
paths over both 5G NR base graphs at lifting sizes below eight, with a
remainder modulo eight, at multiples of eight and at the largest, every DVB-T2
code (`@/citation/Etsi2015`) and
hand-built codes with masked entries, blocks of weight two and permuted orders,
including zero caps, reuse, special values and both stopping modes.

## Channel scaling and saturation

- **No channel scaling.** Before the first iteration every $\lambda_e$ is the
  zero LLR and every $\mu_e$ is $L_v$ unchanged. The decoder multiplies no LLR
  by a constant of its own; `alpha` and `beta` are the only constants of the
  update.
- **No saturation.** The decoder clamps no channel LLR, message or belief.
  `Llr::saturate` is a caller-invoked operation, and the floor at zero of the
  offset rule is the only clipping in the update.
- **Non-finite values.** An infinite or NaN channel LLR is accepted and
  propagates under the rules of this page.

## Variable-node update

### Accumulation order

The belief of variable $v$ with edges $e_1, e_2, \ldots, e_d$ in
`EdgeLayout::var_range` order is the sequential `f32` sum that starts from the
channel LLR:

$$
B_v = (\cdots((L_v + \lambda_{e_1}) + \lambda_{e_2}) + \cdots) + \lambda_{e_d}.
$$

The sum is not reassociated.

### Outgoing messages

Each outgoing message is the accumulated belief minus the incoming message of
its own edge, $\mu_{e_i} = B_v - \lambda_{e_i}$, so every outgoing message
inherits the rounding of $B_v$.

## Hard decision

The decided bit of variable $v$ is 1 when $B_v < 0$ and 0 otherwise
(`Llr::hard_decision`). A positive-zero, negative-zero or NaN belief decides
bit 0.

## Per-frame termination

The parity of a check is the exclusive or of the decided bits on its edges,
and the syndrome passes when every check has parity 0.

- **With early termination** (`DecoderConfig::early_termination`, the
  default), the syndrome is tested after the variable-node update of each
  iteration. The frame stops at the first iteration whose test passes, with
  the `converged` flag set. A frame that reaches the iteration cap without a
  passing test stops there with `converged` clear.
- **Without early termination**, the frame runs to the iteration cap and the
  syndrome is tested once, after the last iteration. `converged` equals that
  test.
- **Iteration count.** `iterations` counts the iterations performed, the
  terminating one included; it is 0 at a cap of zero.
- **Syndrome flag.** `syndrome_check_passed` is the syndrome test of the
  terminal beliefs.
- **Posterior.** `LdpcDecoder::posterior_llrs` returns the beliefs of the last
  iteration performed (the channel LLRs when none is performed), and the decoded codeword is their hard decision,
  whether or not the syndrome passes.

The fields are those of `gf2_coding::ldpc::DecodeOutcome` and
`gf2_coding::DecoderResult`.

## Punctured, truncated and filler positions

The decoder decodes the mother code over all $n$ positions and has no notion
of rate matching. Punctured, truncated and filler positions reach it as
ordinary mother-code channel LLRs prepared by the rate-matching layer,
`Nr5gRateMatchedCode::prepare_llrs` in
[`crates/gf2-coding/src/ldpc/nr_5g/mod.rs`](../../crates/gf2-coding/src/ldpc/nr_5g/mod.rs), for the
rate-matched codes of 3GPP TS 38.212 (`@/citation/ThreeGpp2017`):

| Mother-code position | Channel LLR the decoder receives |
|---|---|
| transmitted | the received LLR |
| untransmitted: punctured systematic or truncated parity | the zero LLR |
| filler | the finite filler magnitude `gf2_coding::ldpc::nr_5g::FILLER_LLR`, positive, favouring the known zero bit |

Every rule above applies to these positions unchanged. The positions each
class occupies are stated in
[Standards conformance](standards-conformance.md#5g-nr-ldpc).

## Batch clauses

A batch decoder carries several frames through one pass of the update. A
*lane* is the slot of one frame in that pass. A lane is *active* while its
frame iterates, *terminated* once its frame has stopped under the
[per-frame termination](#per-frame-termination) rule, and *idle* when the
batch assigns it no frame.

- **One frame per lane.** A lane carries one frame. No reduction inside a
  check and no accumulation inside a variable combines values of different
  lanes, so the check-node and variable-node rules above apply to each lane
  with their order of operations intact.
- **Per-frame equality.** For each batch size and each lane position, a
  frame's results equal the single-frame decoder's on the same channel LLRs,
  configuration and iteration cap in all of:
  - the bit pattern of every posterior LLR,
  - every hard decision,
  - the iteration count,
  - the `converged` and `syndrome_check_passed` flags.
- **Terminated and idle lanes.** A terminated or idle lane is not written
  again: its beliefs, outgoing messages and hard decisions keep the values
  they hold when it leaves the active set. It changes no other lane.
- **Idle lanes.** An idle lane holds zero LLRs.
- **Worker context.** Configured decode concurrency changes route eligibility
  through the coding selectors. The equality clauses hold across explicit
  worker counts, worker scheduling and decoder reuse under another pool.
- **Lane backends.** The lane kernels of `gf2-kernels-simd` have a portable
  backend and an AVX2 backend that writes the same bit patterns, so each
  clause holds on either. `LdpcBatchDecoder` runs the AVX2 backend when
  `gf2-coding` is built with its `simd` cargo feature and AVX2 is detected at
  run time, and the portable backend otherwise; `batch_route` names the
  backend of a lane-route wave.
- **QC backends.** The QC kernels of `gf2-kernels-simd` have the same two
  backends under the same detection, each clause holds on either, and
  `batch_route` names the backend of a QC-route wave.

## Layered quantized decoder

`gf2_coding::ldpc::LayeredDecoder`
([`../../crates/gf2-coding/src/ldpc/layered.rs`](../../crates/gf2-coding/src/ldpc/layered.rs)) is constructed
explicitly; no route selects it. Its results are not the canonical decoder's
frame for frame, and frame error rate is its quality measure. Its kernel,
`gf2_kernels_simd::ldpc_layered`
([`../../crates/gf2-kernels-simd/src/ldpc_layered.rs`](../../crates/gf2-kernels-simd/src/ldpc_layered.rs)),
has a portable and an AVX2 backend, selected as for the QC kernels; both write
the same bit patterns, so every rule below holds on either.

- **Channel quantization.** With channel limit $C \le 127$ and the frame's
  quantizer scale $s_f$, channel LLR $L$ becomes
  $q = \operatorname{clamp}(\operatorname{round}(s_f L), -C, C)$, rounding
  half away from zero. NaN quantizes to 0 and $\pm\infty$ to $\pm C$. A
  punctured or truncated position's zero LLR quantizes to 0, and a filler at
  `FILLER_LLR` saturates to $C$ whenever $s_f \ge C$ / `FILLER_LLR`.
- **Frame scale.** The arithmetic is not scale-invariant: LLRs multiplied by
  $r$ decode as the original LLRs would at scale $r s_f$. `LayeredConfig`
  holds a scale $s$ chosen for natural-log LLRs ($2y/\sigma^2$ for BPSK over
  AWGN, as gf2's demappers produce) and an LLR scale $r$, 1 unless
  `with_llr_scale` states that the inputs are $r$ times natural-log, as the
  $y/\sigma^2$ input bundles of the AFF3CT comparison are with $r = 1/2$.
  The base scale is $b = s/r$. Under `with_fixed_scale`, $s_f = b$.
  Otherwise a cap guard applies: with $m$ the mean magnitude of the frame's
  finite nonzero LLRs,
  $s_f = \max(\min(b, C/(2m)), \min(b, C / \texttt{FILLER\_LLR}))$, and
  $s_f = b$ when no LLR is finite and nonzero. $m$ is the `f32` sum of those
  magnitudes, accumulated into 16 partial sums by index modulo 16 and then
  added in partial-sum order, divided by their count, so $s_f$ is one
  function of the frame on every backend and worker count. The guard keeps
  the channel limit at least twice the mean magnitude: at the waterfall of
  high-rate codes the mean approaches or exceeds the cap $C/b$, and a fixed
  scale saturates a large share of the inputs. The multiple 2 is an observation from the DVB-T2 rate 3/4 to 5/6
  waterfalls, not a derived value; the floor keeps fillers saturated.
- **State.** Each variable holds an `i8` belief $A_v$, initially $q_v$; each
  edge holds an `i8` check-to-variable message $\lambda_e$, initially 0.
- **Layers.** A layer is a set of checks sharing no variable, updated at
  once. `LayeredDecoder::new` takes a code with retained QC structure and
  layers it by base row block in ascending order; a row block whose
  circulants share a column block is split, each check, in lifted-row order,
  joining the first of the block's layers it shares no variable with.
  `LayeredDecoder::with_layers` takes an explicit partition in the given order.
  Checks of degree at most one send only zero messages.
- **Check update.** For check $c$ with message bound $M \le 126$, each edge
  forms $\mu_e = \operatorname{clamp}(A_v - \lambda_e, -(127 - M), 127 - M)$. The
  sign of $\mu \ge 0$ is positive. The excluded minimum magnitude $m_e$ over
  the other edges follows the [tie rule](#magnitude-and-tie-rule) with an
  initial value of 127. `Normalized { eighths: k }` scales it to
  $\lfloor k\, m_e / 8 \rfloor$ and `Offset { beta }` to
  $\max(m_e - \beta, 0)$; the result is bounded by $M$ and signed by the
  product of the other signs. A degree-one check writes 0. Each belief becomes
  $A_v = \mu_e + \lambda_e$, exact in `i8` because of the bound on $\mu_e$.
- **Iteration.** One iteration updates every layer in order; a layer reads
  the beliefs its predecessors wrote.
- **Hard decision.** Variable $v$ decides 1 when $A_v < 0$.
- **Early termination.** A sweep is *settled* when, at each check's update,
  the decisions of its variables have even parity and no update changes a
  decision, and every check of degree one decides 0. A settled sweep leaves
  the decisions it started from, all of which satisfy every check, so its
  syndrome passes. With early termination the frame stops after the first
  settled iteration; this can be one iteration after the first whose
  decisions pass the syndrome.
- **Termination fields.** `iterations` counts the iterations performed;
  `syndrome_check_passed` and `converged` both equal the syndrome test of the
  terminal decisions, which a settled last iteration establishes without a
  separate test. Without early termination the frame runs to the cap. At cap
  zero the posterior is $q$ and the fields are those of
  [per-frame termination](#per-frame-termination).

[`ldpc_layered_contract`](../../crates/gf2-coding/tests/ldpc_layered_contract.rs)
holds both backends to identical posteriors, words, iteration counts and
flags over 5G NR codes of both base graphs at lifting sizes below, at and
above the vector width, DVB-T2 short codes through their QC structure, and a
hand-built code with rows of degree zero, one and two, at caps zero to 25 and
under reuse; it also asserts quantization, termination and rate-matched
decoding, and decodes noiseless and noisy random codewords of every DVB-T2
code and of every NR lifting size off the 32-lane grid, with and without
early termination, under the recommended settings. It holds the frame scale
to its definition and decodes seeded high-rate DVB-T2 frames that the fixed
scale loses. The unit tests of the kernel hold both backends to the update
definition at saturation boundaries.
[`ldpc_layered_fer`](../../crates/gf2-coding/tests/ldpc_layered_fer.rs)
compares the Eb/N0 at frame error rate $10^{-3}$, counted on the first $k$
positions, with the canonical decoder on 5G NR BG1 $Z = 384$ and DVB-T2
normal rate 1/2 over BPSK AWGN, under `LayeredConfig::nr_5g` and
`LayeredConfig::dvb_t2`.

## Evidence

[`ldpc_check_update_contract`](../../crates/gf2-coding/tests/ldpc_check_update_contract.rs)
asserts the check-node rules against a scalar reference on ordinary inputs,
ties, signed zero, extrema and non-finite values, and holds the codeword,
iteration count and flags of `LdpcDecoder` to a per-edge reference decoder for
every algorithm and both early-termination settings.

The unit test `test_zero_cap_results_depend_only_on_the_frame` in
[`crates/gf2-coding/src/ldpc/core.rs`](../../crates/gf2-coding/src/ldpc/core.rs) asserts the result at
an iteration cap of zero against the frame's own channel LLRs and parity checks.

The unit tests of [`crates/gf2-coding/src/ldpc/batch.rs`](../../crates/gf2-coding/src/ldpc/batch.rs)
assert the batch clauses for `LdpcBatchDecoder` on the lane route of each lane
backend of the build and on the single-frame route:
`frames_equal_the_single_frame_decoder_to_the_posterior_bit` holds each frame's
posterior bit patterns, hard decisions, iteration count and flags to
`LdpcDecoder` for each min-sum rule and both early-termination settings,
`a_terminated_lane_keeps_its_posterior_while_its_wave_continues` asserts the
terminated-lane clause, and the two `*_satisfy_the_batch_contract_on_each_route`
tests run the shared batch contract over full, partial and empty waves.

[`ldpc_batch_contract`](../../crates/gf2-coding/tests/ldpc_batch_contract.rs)
asserts per-frame equality on each route and lane backend of
`LdpcBatchDecoder` over a DVB-T2
short code (`@/citation/Etsi2015`), 5G NR mother codes of both base graphs
(`@/citation/ThreeGpp2017`) and hand-built codes with checks of degree one, two
and above eight. Its frames are noisy codewords whose waves mix early, late and
non-converging frames, frames of signed zeros, saturated, extreme finite and
non-finite LLRs, and rate-matched 5G NR frames; each set runs at the batch
sizes around the wave boundaries, under a permutation of its frames, and with
the other lanes of a wave holding non-converging frames. A build with `simd`
runs the AVX2 backend, requires AVX2 of the host and asserts that the lane
route reports that backend; a build without `simd` asserts that it reports the
portable backend. The same assertions run on the QC route of each backend over
5G NR mother and rate-matched codes, where the portable backend is the forced
fallback of a host without the kernel.
[`tuning_profile_committed`](../../crates/gf2-coding/tests/tuning_profile_committed.rs)
installs the committed coding envelope in a fresh process and asserts that NR
BG1 Z=384 decodes on the route its selectors and the structural QC bounds give
each call, on each backend, with each frame's word and outcome equal to
`LdpcDecoder`'s.

[`dvb_t2_concat_batch_decoder`](../../crates/gf2-coding/tests/dvb_t2_concat_batch_decoder.rs)
runs the shared batch contract on `DvbT2ConcatBatchDecoder`, whose LDPC inner
code decodes on `LdpcBatchDecoder`, against `DvbT2Concat::decode_soft_counted`
on each route and lane backend: a frame's BBFRAME, LDPC iteration count and flags equal the
single-frame codec's, for converging and non-converging frames.

The unit tests of
[`crates/gf2-coding/src/ldpc/nr_5g/batch.rs`](../../crates/gf2-coding/src/ldpc/nr_5g/batch.rs) hold
`Nr5gRateMatchedBatchDecoder`, which prepares each rate-matched frame as above
and decodes the batch through `LdpcBatchDecoder`, to
`Nr5gRateMatchedDecoder::decode_iterative` frame by frame: the shared batch
contract runs on each route and lane backend over codes of both base graphs with punctured
systematic columns, truncated parity and filler bits, for each algorithm and
at an iteration cap of zero.
[`nr5g_external_vectors`](../../crates/gf2-coding/tests/nr5g_external_vectors.rs)
decodes the rate-matched codewords of `@/citation/Sionna2026` to their messages
through the same decoder on each route and through
`Nr5gRateMatchedDecoder::decode_iterative`; the vectors are described in
[Standards conformance](standards-conformance.md#5g-nr-ldpc).
