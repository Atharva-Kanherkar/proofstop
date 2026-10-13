# Bounded answer test contract

## Functional Behavior
- Keep the ordinary naturalsize task, runtime, acceptance and evaluation unchanged.
- Add an explicit one-shot forced-answer mode to the existing exact-token controller. Generate a bounded reasoning segment, close the open thinking envelope on a separate answer prefix, and issue exactly one answer request. Stop after that request, including rejected or failed candidates. Never resume reasoning or repair using evaluation.
- Permit request timeouts up to 1200 seconds for queue cold starts plus a bounded worker job; reject values above 1200. Validate every committed calibration configuration before inference.
- Preserve the default periodic-probe and segmented policies and their recorded identities.
- Require one probe and a reasoning segment strictly below the configured main budget. Record reasoning, answer, replay usage and all failures; truncated answers cannot execute.
- Freeze a 16384-token thinking calibration at seed 43 and a 512-token reasoning plus 2048-token forced answer. If calibration passes, repeat both policies at seeds 42 and 44. No outcome-driven prompt, test or budget changes.
- Use a separately recorded Serverless calibration if needed: pinned worker release v2.29.0 (source b7212acc6d60d5ada997f453783c2e09df8e6e49), vLLM 0.31.0, same Qwen3-8B revision, BF16 and 24576 context. Verify exact token-prefix echo and tokenization. Min workers 0, max 1, 10-second idle release, bounded startup and per-job execution. Record queue/proxy transport and do not claim original upstream HTTP byte spacing or dedicated GPU timing from queue execution time.
- Additional GPU spending must remain under $3. Do not provision a billable Pod without verified automatic teardown independent of the interactive client. A scale-to-zero backend is acceptable only if model identity, exact-prefix protocol and cost bounds can be established; disclose backend changes.

## Unit Tests
- Forced mode rejects zero/multiple probes and reasoning segments that exhaust the main budget.
- Existing option validation and token-prefix invariants remain compatible.

## Integration / Functional Tests
- A failed forced answer never starts another main request; preserve its source and visible outcomes.
- A truncated forced answer stops without execution or another request.
- A passing forced answer freezes its decision before evaluation; evaluation is excluded from model requests.
- Existing periodic-probe and segmented integration checks remain green.

## Smoke Tests
- CLI help exposes the forced-answer option; task inspect and the archived reference control remain valid.

## E2E Tests
- Complete the frozen GPU calibration/comparison when the automatic cost guard is available. Otherwise record the exact infrastructure prerequisite and retain a ready local implementation without renting an unguarded GPU.
- Preserve raw bodies, counters and billing records. Confirm teardown before finishing.

## Manual Tests
- cargo test --locked --all-targets
- cargo test --locked --tests -- --ignored --test-threads=1
- cargo fmt --check
- cargo clippy --locked --all-targets -- -D warnings
