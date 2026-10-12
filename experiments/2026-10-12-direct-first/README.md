# Direct first, one repair: live development screen

A pinned Qwen3-8B / vLLM 0.10.2 / A40 comparison completed all 30 declared trials.
The direct-first controller repaired both cheap-answer failures and passed 10/10
development tasks. Direct answering passed 8/10. Full thinking passed 6/10 under
its per-request 4,096-token cap. These are small public synthetic fixtures, not
held-out research evidence.

## Aggregate results

| Policy | Passed | Output tokens | Engine request seconds | Allocated trial seconds | Trial seconds per success |
| --- | ---: | ---: | ---: | ---: | ---: |
| nonthinking | 8/10 | 806 | 25.37 | 79.23 | 9.90 |
| thinking | 6/10 | 26,144 | 819.59 | 869.54 | 144.92 |
| repair | 10/10 | 4,116 | 129.19 | 181.11 | 18.11 |

Direct-first made 12 requests: ten initial answers and two repairs. All ten initial
answers exactly match their direct-control text. The two repairs fixed binary
search duplicate handling and excessive recursive flattening. No acceptance false
positives were observed in these finite evaluation suites.

| Task | Direct | Thinking | Direct plus one repair |
| --- | --- | --- | --- |
| binary-search | tests failed | pass | pass (one repair) |
| chunk-list | pass | tests failed | pass |
| flatten-once | tests failed | pass | pass (one repair) |
| run-length | pass | pass | pass |
| moving-average | pass | cap reached | pass |
| stable-difference | pass | pass | pass |
| rotate-list | pass | cap reached | pass |
| balanced-brackets | pass | pass | pass |
| dependency-order | pass | cap reached | pass |
| group-totals | pass | pass | pass |

## Interpretation

The controller bought two additional successes at added cost. It was considerably
cheaper than full thinking in this screen, but cost **more per successful task than
direct answering**: 18.11 versus 9.90 allocated trial seconds. That difference
must not be hidden by comparing only against thinking. The result supports a
quality/cost tradeoff for further testing, not a general efficiency win.

Three thinking requests hit the 4,096-token cap; another produced a completed
candidate that failed checks. These failures are budget-specific. They do not
establish what thinking would achieve with a larger budget. The policies use
their respective recommended sampling profiles, so this is not an isolated
causal test of the thinking flag. Repairs have an additional request budget.

## Frozen method

- Source: `70ce3c76638164f42a6df798993cefe0117546da`.
- Model and tokenizer: `b968826d9c46dd6066d109eabc6255188de91218`.
- Ten fixtures and manifest hashes: [plan.json](plan.json), copied from the pinned task set.
- Three policies, one trial per task, seed 42. Order shuffled once with Python Random(42) before generation.
- BF16, no quantization, TP=1, one sequence, 8,192-token context; prefix caching disabled for every trial.
- Direct sampling: temperature 0.7, top_p 0.8, top_k 20, thinking false. Thinking: 0.6, 0.95, 20, thinking true.
- Each generation capped at 4,096 tokens. Repair allowed only after failed visible acceptance, at most once.
- Inference runs on the rented GPU; verification runs in the pinned Docker image on the client, while the GPU remains allocated.
- No evaluation feedback, no trial retries, no changes to workloads, order or generation settings after generation began.

The pretrial [original plan](plan-original.json) was updated only to identify the
commit containing the driver and documentation. Both versions preserve the same
tasks, order and generation settings. No model request preceded the update.

## Timing and spending

Allocated trial time brackets each client process and its before/after metrics
calls, including verification waits and failed answers. The engine counter is
request processing wall time, not synchronized CUDA kernel time. Per-run summaries
retain null GPU-seconds; the separate driver brackets measure exclusive allocated
device time for this sequential screen. Client wall time and engine time are also
reported independently.

The full allocation lasted at most 1,574.203 seconds from creation to confirmed
deletion. Trials account for 1,129.885 seconds; setup, cleanup and gaps account
for the remaining 444.318 seconds. The shared overhead has no unique per-policy
allocation, so the table excludes it and the full rental reports it separately.
The whole screen yielded 24 policy/task successes, not 24 distinct tasks.

At the quoted $0.59/hour, GPU compute is estimated at **$0.2580**, before storage
and tax. The [billing read](billing-read.json) returned no finalized rows yet; that
does not mean zero spend. The experiment Pod was deleted, and a fresh Pod list
was empty. No network volume was created. The two-hour deadline was agent enforced,
not an automatic platform termination guard.

## Evidence and reproduction

- [results.json](results.json): every trial, portable command, timing, scoring and engine counter deltas.
- [report.json](report.json): aggregates and direct-stage comparisons.
- [invariants.json](invariants.json): request counts, evaluation-name exclusion and no generation after candidate freeze. Null marks capped baselines with no candidate to freeze.
- [trial-manifest.json](trial-manifest.json): SHA256 of every trial artifact, including exact request and response bodies.
- `trials/`: all raw HTTP JSON bodies, configurations, candidates, feedback and event ledgers. Response headers are not captured; status and body hashes are recorded.
- [metrics.tar.gz](metrics.tar.gz): all 60 unmodified before/after metric captures; [metrics-manifest.json](metrics-manifest.json) hashes the archive and each member.
- [environment.json](environment.json), [packages.txt](packages.txt), [server-args.json](server-args.json): observed serving setup.
- [allocation.json](allocation.json): rental window, quoted rate and cleanup evidence.

Read [the runbook](../../docs/direct-first-repair.md) and use the source commit above.
Prepare a session plan and allocation deadline before invoking
`python3 scripts/development-screen.py --session runs/new-screen`. The driver
expects a provisioned private endpoint; it neither rents nor deletes hardware.
Use a fresh directory and retain all failures. There is no resume/retry command.
Recompute the published aggregate without renting hardware:

```bash
python3 scripts/summarize-screen.py --experiment experiments/2026-10-12-direct-first
```

## Limits and next decision

One seed, ten tiny public tasks and four evaluation cases per task cannot establish
held-out correctness, a small quality margin or production cost savings. The CPU
verifier is a finite checker, not a formal proof system. Fixed profiles and
sequential serving leave concurrency, cache reuse and richer repairs untested.

Next, freeze harder independently evaluated repair workloads and compare direct
plus repair with stronger cheap controls at comparable quality and total budgets.
Keep direct answering in every comparison. Do not advance to serving-engine
integration on this development screen alone.
