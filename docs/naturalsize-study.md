# File-size formatting repair

This workload repairs an ordinary formatting defect in python-humanize/humanize:
rounding near a unit boundary can produce `1000.0 kB` instead of `1.0 MB`.
The [upstream fix](https://github.com/python-humanize/humanize/pull/329) is already
merged. This is a public historical development task, not a held-out benchmark.

The model replaces `humanize/filesize.py` at pre-fix commit
`7574e0cc377d80db0d2756fc2c5e0b8799a6f8b4`. The pinned Docker worker calls the real
`humanize.naturalsize` API. All tracked Python modules and `py.typed` are copied
byte-for-byte. The upstream MIT license is preserved. Default English needs no
translation catalogs, and a documented `_version.py` shim provides the
build-generated snapshot version metadata absent from Git.

Four visible cases cover decimal/binary rollover and ordinary formatting.
Twelve independently authored evaluation cases cover signs, GNU style, later
unit boundaries, precision, string input, small values and the largest suffix.
The suites were written before inspecting the repaired target. A large numeric
input uses the API's supported string form to preserve its exact input bytes
across host JSON parsing. Expected answers stay outside Docker.

The real source contains triple-backtick examples inside its docstring. Use a
four-backtick Python block for a complete replacement file. Candidate parsing
supports that delimiter while still rejecting ambiguous external blocks.

```bash
cargo run --locked -- inspect --task tasks/repository/naturalsize-rollover --show-prompt
cargo run --locked -- verify --task tasks/repository/naturalsize-rollover \
  --candidate tests/fixtures/repository/naturalsize-fixed.py
```

The archived target fails the rollover checks; the exact upstream repaired file
passes all sixteen checks on the same runtime. Tests exercise a local captured
model response and the real Docker execution path. Those are harness checks,
not model-performance evidence.

## Frozen live comparison

Three policies use seeds 42, 43 and 44, for nine attempts on one task. The order
is shuffled once before generation, with no retries or post-result tuning:

| Policy | Output ceiling |
| --- | --- |
| Direct non-thinking | 4,096 |
| Full thinking | 12,288 |
| Direct plus one repair | 4,096 initial + at most 8,192 repair |

Thinking and the two-request policy share the same maximum total output ceiling.
Mode-specific sampling uses the existing recommended profiles. Model/tokenizer
revision is `b968826d9c46dd6066d109eabc6255188de91218`, with vLLM 0.10.2, BF16,
24,576-token context, one sequence and prefix caching disabled.

The exact prompt is tokenized locally before renting and must match token IDs
from the live server before generation. Conservative headroom includes the
initial answer, added repair instructions and the repair budget. The driver
refuses prior sessions, source/task/config drift and tokenizer mismatch. Utility
requests and every trial's metrics are captured separately from generation.

`scripts/repository-screen.py --session runs/new-session` consumes a frozen plan,
prompt, tokenizer preflight and allocation deadline. It does not rent hardware.
Only a previously failed visible acceptance check can authorize one repair;
evaluation cannot. Candidate-format and capped-generation failures remain in the
comparison. Infrastructure failures stop it without concealing or retrying them.

Report every attempt, total tokens and exclusive trial allocation time including
verification waits. Engine request timing is a separate counter, not CUDA kernel
time. Complete rental duration includes setup and gaps. One task across three
seeds cannot establish general quality or savings.
