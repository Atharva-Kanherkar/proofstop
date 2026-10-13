# Larger-budget calibration on the ordinary file-size task

The predeclared seed-43 baseline completed naturally after 12,891 output tokens
within its 16,384-token allowance. Its final replacement file is byte-for-byte
identical to the unfixed source. It passes two of four acceptance checks and six
of twelve separate evaluation checks. The original defect remains.

This separates two problems: the previous ceiling prevented this seed's final
answer from finishing, but finishing that answer did not produce a correct fix.
More reasoning alone is not a successful calibration.

The baseline-success gate failed. No forced-answer performance attempt, additional
seed or larger budget was submitted. The one-shot controller is locally tested;
its live quality and exact-prefix compatibility on this backend remain unverified.
Pause this model/task/prompt pairing before making compute-efficiency claims.

## Recorded environment and transport

The task, source snapshot, four visible cases and twelve evaluation cases are
unchanged. Both candidate generation and actual `humanize.naturalsize` execution
use the existing recorded interfaces. Evaluation was not part of the request.

Model: Qwen/Qwen3-8B at `b968826d9c46dd6066d109eabc6255188de91218`.
The separately recorded Serverless backend uses official worker v2.29.0, source
`b7212acc6d60d5ada997f453783c2e09df8e6e49`, vLLM 0.31.0, BF16 and A40.
Observed image digest:
`sha256:a218acc3f5032d2bc1b9104b613b5ed0bf34bcd6b7c561396d17a60aa4b7962c`.
This backend differs from the previous vLLM 0.10.2 Pod and is not an isolated
output-budget comparison against that earlier run.

The private queue bridge preserves the submitted HTTP body, complete provider
job envelope and returned JSON separately. The official worker parses and
serializes JSON; original worker HTTP byte spacing is not observable here.
Provider usage agrees with the locally captured response and summary.

## Time and cost

The job reports 369.053 seconds of execution and 79.531 seconds of queue delay.
At the catalog rate of $1.22/hour, execution alone corresponds to approximately
$0.125. This excludes initialization, setup checks, idle periods and any worker
overlap. It is neither a complete bill nor allocated GPU time.

The billing read returned no rows yet. Complete charges remain unknown, not zero.
Both experiment endpoints were deleted, and a fresh endpoint list was empty.
The Hub template lookup also returns 404. The additional spending limit was $3;
a finalized billing read is still needed to verify total expenditure.

## Setup and limitations

The first setup used the Hub image directly. Its system log reported a registry
authentication problem, so it was deleted before performance requests. The
corrected deployment used the official Hub release. Version checks returned
0.31.0; system logs record automatic stop and removal of worker containers after
requests. Worker inventory still retained idle entries, so it is not used as proof
of empty hardware inventory or a clean single-worker allocation comparison.

A local configuration preflight rejected an unsupported client timeout before
any inference request. Its log was retained locally, the bound was explicitly
updated and all six profiles were tested before model generation.

Raw bodies and execution outcomes are in [trials](trials/), with queue envelopes
in [queue](queue/). [plan.json](plan.json) records the calibration gate and serving
settings. [report.json](report.json) records the failed outcome, identity check,
usage and timing limits. [evidence-manifest.json](evidence-manifest.json) hashes
all captured evidence preceding this explanatory README.

## Recheck captured evidence

```sh
python3 scripts/summarize-bounded.py --experiment experiments/2026-10-13-bounded-calibration
```

The live queue broker used the authenticated MCP connector. To replay the
transport with another authorized broker, submit each `queue/*-pending.json`
input and policy as one Serverless job; retain its entire output and return the
single JSON result to the private bridge. Do not replay into existing capture
folders or interpret a replay as new online savings evidence.
