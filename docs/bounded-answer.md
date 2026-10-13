# Bounded reasoning and a forced answer

The previous ordinary repository comparison produced no successful fixes. One
thinking attempt began a final answer but ran out of tokens. This calibration
asks whether a larger natural-answer baseline succeeds, and whether closing
reasoning early can still produce the same verified fix.

`online --force-answer --max-probes 1` generates one bounded reasoning segment,
then creates a separate answer prefix with the thinking envelope closed. It
returns after that answer, including a failed visible check, malformed answer or
truncation. It never resumes reasoning after a failed forced answer. Evaluation
remains excluded from generation and stopping decisions.

```sh
cargo run --locked -- online --task tasks/repository/naturalsize-rollover \
  --config configs/bounded-answer/forced-seed-43.json \
  --model Qwen/Qwen3-8B --segment-tokens 512 --probe-tokens 2048 \
  --max-probes 1 --force-answer --output runs/forced-43
```

The frozen calibration baseline uses 16,384 output tokens at seed 43. The forced
policy uses at most 512 initial tokens plus 2,048 answer tokens. If calibration
succeeds, both policies repeat at seeds 42 and 44, within the additional $3 cap.
The task, visible cases, separate evaluation, sampling profile and model revision
remain unchanged. This is one public task, not held-out quality evidence.

A Serverless backend may replace the old rented Pod to provide native scale to
zero: no minimum workers, at most one worker and a 10-second idle timeout.
Bounded startup and job execution are recorded before any generation. The
backend change is explicit: official worker release v2.29.0 uses vLLM 0.31.0.
Calibration and forced answers must share this backend. Job execution time is
not allocated GPU time, and queue delay is not billed GPU work. Record provider
billing separately; do not claim GPU efficiency from token counts alone.

The complete test contract is [testing/feat-bounded-answer.md](../testing/feat-bounded-answer.md).
The [live calibration](../experiments/2026-10-13-bounded-calibration/README.md)
completed naturally after 12,891 output tokens but returned the original buggy
file unchanged. It failed the baseline-success gate, so forced-answer performance
and live exact-prefix validation were skipped. Both endpoints were deleted.
The new controller has local protocol and real Docker coverage; its benefit is
unproven.
