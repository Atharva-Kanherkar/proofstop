# Non-security repository repair and GPU comparison

## Functional Behavior
- Use only an ordinary file-size formatting defect in python-humanize/humanize. No security workloads are part of this branch or live comparison.
- Pin pre-fix commit 7574e0cc377d80db0d2756fc2c5e0b8799a6f8b4 and replace humanize/filesize.py. Execute the actual humanize.naturalsize API in read-only Docker with unchanged runtime dependencies.
- Freeze independently authored visible acceptance and separate evaluation cases before inspecting the upstream repaired target. Keep reference code and evaluation out of generation and feedback.
- Preserve English formatting, decimal/binary/GNU modes, signs, custom precision and suffix limits. Rounded mantissas reaching their unit base must advance when a larger suffix exists.
- Support four-backtick Python fences so literal triple-backtick examples inside the real source file do not invalidate a complete candidate. Keep old three-backtick behavior and ambiguity rejection covered.
- Validate all source hashes, fixed repository identity, safe paths and dependency pinning. Capture exact repository worker stdout without passing it to repair feedback.
- Freeze three policies across seeds 42, 43, 44. Direct budget 4,096, full thinking 12,288, bounded repair 4,096 plus at most 8,192. Three seeds are attempts on one public task, not three distinct tasks or held-out evidence.
- Tokenize the exact prompt before GPU allocation; verify its count on the private server before generation. Context 24,576, pinned Qwen3-8B/vLLM 0.10.2/BF16, prefix caching off. No retries, tuning or task changes after generation starts.
- Read GPU stock/rate first, remain within authorized $25 total, enforce a two-hour rental deadline, download evidence, delete only the experiment Pod and verify absence.

## Unit Tests
- Repository pinning, prompt separation, four-backtick nested examples, missing closure and ambiguous blocks.
- Driver rejects previous sessions and source/task/config drift before generation.

## Integration / Functional Tests
- Actual old module fails rollover cases; archived fixed target passes all independent cases on the same unchanged runtime.
- Local HTTP capture plus Docker runs the replacement target through baseline and bounded repair.
- Preserve all existing synthetic-task tests and report checks.

## Smoke Tests
- inspect validates the repository fixture. CLI verify shows old failure and fixed success.
- Server version/model/tokenizer preflight succeeds through SSH with matching token IDs.

## E2E Tests
- Complete all nine frozen real-model trials, or retain an explicit incomplete report on an infrastructure failure.
- Preserve model failures and raw counters, recompute totals, verify bounded repairs and evaluation exclusion, and confirm teardown before ending work.

## Manual Checks
- cargo test --locked --all-targets
- cargo test --locked --tests -- --ignored --test-threads=1
- cargo fmt --check
- cargo clippy --locked --all-targets -- -D warnings
- python3 -B -m unittest discover -s tests -p 'test_screen_driver.py'
