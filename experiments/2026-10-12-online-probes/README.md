# First online probing comparison

The Rust controller stopped successfully after the first probe on the original
fixture. Direct non-thinking solved the same fixture with less work and lower
latency. All four policies failed on the multi-bug variant under the declared budgets. This experiment
verifies the online mechanics and records a negative result for fixed probing;
it does not demonstrate savings against the strongest tested baseline.

## Procedure

One pinned Qwen3-8B / vLLM 0.10.2 server on an A40 ran four policies once on each
of two related public development fixtures. Seed 42, model/tokenizer revision,
precision, context, and budgets were fixed. Policy order was shuffled with seed
42 and declared before any generation. Prefix caching was disabled for every
policy, so earlier trials could not warm later trials' prompt cache. Compilation
and model loading happened before the trials and remain in rental accounting.

Each thinking policy had a 4096-token main cap and model-recommended sampling.
Non-thinking used its documented sampling profile. Segmented policies used
128-token requests. Periodic probing allowed four answer probes, each capped at
1024 tokens, at the first four checkpoints. Failed probes never fed their output
or test results into the main path. No settings were tuned after trial results.

## All eight outcomes

Prompt counts include every replayed prefix. Output counts include every main
and probe generation, including failed candidates. Times are observed single-run
measurements, not confidence intervals.

| Fixture | Policy | Outcome | Prompt tokens | Output tokens | Client wall s | Engine inference s |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| Original | periodic | passed | 1139 | 254 | 15.807 | 7.917 |
| Multi-bug | thinking | failed | 475 | 4096 | 129.766 | 128.167 |
| Multi-bug | segmented | failed | 78688 | 4096 | 215.118 | 139.176 |
| Multi-bug | periodic | failed | 81880 | 4682 | 248.002 | 158.589 |
| Original | segmented | failed | 79616 | 4096 | 212.841 | 138.880 |
| Multi-bug | nonthinking | failed_checks | 479 | 148 | 6.306 | 4.566 |
| Original | thinking | failed | 504 | 4096 | 128.983 | 127.673 |
| Original | nonthinking | passed | 508 | 126 | 5.411 | 3.909 |

`passed` means acceptance 4/4 and evaluation 8/8. `failed` thinking attempts
exhausted their main budget without an accepted final answer. `failed_checks`
means a complete candidate was returned but failed functional checks. The
multi-bug non-thinking candidate passed 2/4 acceptance and 5/8 evaluation checks.

Engine inference seconds are differences in vLLM's
`request_inference_time_seconds_sum` counter, including every completed main and
probe request. They are engine request wall time, not measured GPU-kernel time or
allocated GPU-seconds. Client wall time includes utility API calls and Docker
verification. The runner leaves its allocated-GPU metric unknown. Counter deltas
for generation tokens match reported usage on all eight trials.

## What the stop decision did

On the original fixture, the first main segment generated 128 tokens. A separate
probe generated a complete 126-token answer. Acceptance passed 4/4; the
controller issued no further generation request. The frozen answer then passed
8/8 evaluation. The total was 254 generated tokens and 1139 prompt tokens across
two generation requests, plus four tokenizer utility requests.

On the multi-bug fixture, four probes returned candidates that each passed only
2/4 acceptance checks. The controller rejected them, preserved the main branch,
and eventually exhausted the main budget. Those probes added 586 output tokens
and 32.884 seconds of client wall time relative to the no-probe control.

The live token records confirm:

- The original probe's 128-token main prefix matches the no-probe control.
- All 4096 main tokens on the multi-bug variant match the no-probe control.
- No HTTP request occurred after acceptance triggered the original stop.

These are checks of this experiment, alongside general controller tests for
branch isolation, unknown usage, evaluation failure, and inconclusive verifiers.

## Evidence

- [Final pre-generation plan and order](plan.json), [original declaration](plan-original.json)
- [All results and exact commands](results.json)
- [Continuation and stop invariants](invariants.json)
- [Environment and allocation](environment.json), [packages](packages.txt), [server arguments](server-args.json)
- [Original passing probe](trials/01-merge-ranges-periodic/summary.json)
- [Multi-bug failed probes](trials/04-merge-ranges-multi-bug-periodic/events.jsonl)
- [Direct-answer control](trials/08-merge-ranges-nonthinking/summary.json)

Each trial directory preserves configuration, task identity, events, outcome,
and any frozen candidates. Single-request trials expose request/response JSON
files directly. Online trials package every exact HTTP request and response byte
in `wire.tar.gz`, with individual and archive SHA256 hashes in
`wire-manifest.json`. Compression changes no captured bytes. Extract into a new
empty directory to inspect:

```sh
proofstop_review_dir="$(mktemp -d)"
# Run from this experiment directory:
tar -xzf trials/01-merge-ranges-periodic/wire.tar.gz \
  -C "$proofstop_review_dir"
```

[Raw metrics snapshots](metrics/) support every engine counter delta. Full server
logs and GPU diagnostic output remain in local `runs/online-session-001`.
Filesystem output arguments in the published command records are expressed
relative to the repository for portability; HTTP bodies are unchanged.

The controller and source were pinned at the commit recorded in `plan.json`.
The final-answer parsing refinement happened before the first trial and did not
change generation settings. [Runbook](../../docs/online-probes.md).

## Limitations and decision

Two variants of one public function, one seed, and one trial per policy cannot
establish general quality or an economic benefit. Prefix replay resets the seed
per segment and does not resume RNG state or fork KV cache. Segmented reasoning
therefore is not guaranteed to match an uninterrupted trajectory. Model-recommended
sampling differs between thinking and non-thinking. Cross-run prefix caching was
disabled; a cache-enabled implementation may have different replay costs.

Direct non-thinking and periodic probing each solved one fixture out of two.
Direct answering was cheaper on both. Fixed early probing can waste work when
candidate quality does not improve. The next experiment should start with a
verified cheap candidate and test when to pay for further reasoning, on a broader
frozen development set with a separate held-out evaluation set. Changes based on
these public fixtures must not be reported as held-out improvements.

## Cost and teardown

The Pod read quoted $0.59 per GPU-hour. From allocation to confirmed cleanup,
the session lasted at most about 25.5 minutes, corresponding to approximately
$0.251 of compute. Disk charges and applicable taxes are additional. Final billed
cost remains pending because the billing read returned no records.
The experiment Pod was terminated and a fresh account Pod list was empty. No
network volume was created. The $25 authorized ceiling was not approached.
