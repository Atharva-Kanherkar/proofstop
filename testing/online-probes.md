# First online probes: acceptance contract

## Functional behavior

- Add an explicit optional thinking-mode switch to recorded chat requests. Label
  non-thinking runs as their own baseline. Preserve legacy request defaults.
- Add a pinned multi-bug merge-ranges development fixture, still using the same
  isolated worker and function contract. Keep evaluation out of every prompt.
- Implement a bounded segmented no-probe control and periodic answer probes for
  pinned vLLM 0.10.2 and Qwen3-8B. Use server chat tokenization, integer-token
  completion prompts, returned token IDs, and server detokenization. Never
  approximate continuation by re-tokenizing generated text.
- At each configured checkpoint, branch from the exact main token prefix. If
  reasoning is still open, append tokenized `</think>` on the probe branch only.
  A complete stopped probe answer may be verified. Probe output and test
  feedback never enter the main reasoning path.
- Stop future generation only when visible acceptance passes. Freeze that
  candidate and run separate evaluation exactly once. A later evaluation
  failure does not resume generation or become feedback.
- Failed or malformed probes continue the main path. Sandbox-inconclusive
  results abort as inconclusive. Transport/protocol failures abort and record
  failure. Never treat a truncated probe as executable.
- Bound main generated tokens, segment size, probe output tokens, and number of
  probes. Count every inference request, replayed prompt, and probe token.
- Record exact requests/responses, main/probe token-prefix identities, candidate
  hashes, verification results, failures, and unknown usage. Refuse existing run
  directories. Keep GPU-seconds unknown unless measured.
- Do not claim exact RNG-state or KV-cache continuation. Seed resets and prefix
  replay are explicit limitations; compare with the segmented no-probe control.

## Unit tests

- Thinking switch serialization and baseline policy labels.
- Token response validation: one choice, integral nonnegative IDs, bounded ID
  count, supported stop/length finish reasons, and matching echoed prompt IDs.
- Probe branch preserves exact main prefix without mutating it.
- Missing or inconsistent tokenization and malformed responses fail closed.

## Integration and functional tests

- Local protocol server plus real Docker: first passing probe stops future
  requests; failed probe leaves main continuation unchanged; evaluation failure
  stops rather than feeding back; budget exhaustion leaves recorded failure.
- Unknown usage stays unknown across multiple requests. HTTP errors preserve
  request evidence and do not create false success.
- Existing baseline and Docker isolation tests remain green.
- Reference solution passes both fixtures; multi-bug source fails visible tests.

## Smoke tests

- CLI inspect, explicit non-thinking baseline, segmented, and periodic commands.
- Live pinned GPU `/tokenize`, `/v1/completions` with token IDs, and `/detokenize`.

## End-to-end tests

Run full-thinking, non-thinking, segmented-no-probe, and periodic-probe policies
on the two public development fixtures. Keep failed attempts. Record environment,
wall time, every token usage, metrics, and cleanup. This demonstrates online
mechanics, not held-out quality or GPU-efficiency gains.

## Manual checks

Run formatting, Clippy, Rust and real Docker tests. Review traces to confirm
probe isolation and no request after acceptance. Publish raw synthetic artifacts,
limitations, and a plain-language explanation. Terminate only this experiment's
Pod after copying evidence and confirm absence.

## Final-answer parsing refinement

Strip a leading, closed Qwen `<think>...</think>` envelope before extracting
executable code. Reject unclosed reasoning even if it contains a complete code
fence. In token-prefix runs, restore the known opening thinking marker for
parsing only; never insert it into generation or change recorded token IDs.
Test that draft reasoning code is excluded and only the final block executes.
