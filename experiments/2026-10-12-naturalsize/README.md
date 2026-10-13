# File-size formatting repair on Qwen3-8B

Nine frozen attempts compare three policies on one public historical defect in python-humanize/humanize. The task repairs rounded unit rollover, for example `1000.0 kB` becoming `1.0 MB`. Three seeds are three attempts on the same task, not three independent tasks or held-out quality evidence.

| Policy | Passed attempts | Output tokens | Allocated trial seconds | Seconds per success | Estimated trial GPU cost |
| --- | --- | --- | --- | --- | --- |
| nonthinking | 0/3 | 2,283 | 89.3 | undefined | $0.0146 |
| thinking | 0/3 | 36,864 | 1979.2 | undefined | $0.3244 |
| repair | 0/3 | 26,859 | 899.5 | undefined | $0.1474 |

Direct answering has a 4,096-token ceiling. Thinking has 12,288. Direct plus one repair has 4,096 initial tokens and at most 8,192 repair tokens, sharing the same maximum total output ceiling as thinking. Sampling profiles differ by mode. Every failure remains in the totals; there are no retries or post-result task changes.

A successful attempt passes all four visible acceptance cases and twelve separate evaluation cases. Evaluation is never supplied to generation or repair feedback. The pinned runtime calls the actual `humanize.naturalsize` API. The archived upstream repair passes all sixteen cases, establishing a working control.

## Paired outcomes

| Seed | Direct | Thinking | Direct plus repair |
| --- | --- | --- | --- |
| 42 | failed_checks | failed | failed |
| 43 | failed_checks | failed | failed |
| 44 | failed_checks | failed | failed |

## Failure categories

- nonthinking: failed_checks (3)
- thinking: completion did not finish normally (3)
- repair: completion did not finish normally (3)

The exact task, prompt, configurations, source commit and order are frozen in [plan.json](plan.json). Raw request/response bodies, candidates, verification stdout and event ledgers are under [trials](trials/), with hashes in [trial-manifest.json](trial-manifest.json). Engine metrics are preserved unmodified in `metrics.tar.gz`, verified against `metrics-manifest.json`.

## Timing and rental

Trial allocation includes inference, local Docker verification, client overhead and metric calls while the dedicated GPU remains allocated. Engine request timing is reported separately in [report.json](report.json); it is not CUDA kernel time. Token reduction alone does not prove GPU efficiency.

The single A40 rental lasted at most 8614.6 seconds, for estimated GPU compute cost $1.4118 at $0.59/hour. This includes setup, all pilot generation, the corrected study, gaps and teardown. The estimate excludes storage and tax. The post-deletion [billing API snapshot](billing-read.json) reports $1.4331 including observed GPU and disk charges; it is not a final invoice. The Pod was deleted and absence verified.

The driver finished at approximately 18:26 UTC, but cleanup was not confirmed until approximately 19:41 UTC. The client stopped progressing after the driver ended; the sleep inhibitor had been bound only to that driver. The two-hour agent-enforced deadline was exceeded by approximately 23 minutes. The full post-study delay is charged in the rental duration above. Future runs need sleep inhibition through cleanup and teardown independent of the interactive session.

## Client timing interruption

During seed 44 thinking, macOS entered idle sleep for 773 seconds while the remote server continued and completed generation. The original allocation bracket retains that charged time. Idle sleep was then prevented for the rest of the run. [client-interruption.json](client-interruption.json) records the window and treatment. No trial was rerun and no time was subtracted. These allocation totals cannot support an unbiased policy cost ranking.

## Interrupted pilot

An initial pilot used a parser that incorrectly split indented Markdown examples inside Python docstrings. The pilot requested four-backtick fences; two completed answers instead used ordinary three-backtick fences containing indented docstring examples and were rejected by that parser; a third thinking attempt exhausted its 12,288-token budget. The driver was stopped before the fourth trial. This is a harness fault, not a valid policy comparison.

The [pilot artifacts](pilot/) retain all three requests and responses, original frozen identities, two driver result rows and a separate three-attempt ledger. Third-trial allocation timing is unavailable because the driver was paused; it is explicitly null. Pilot cost is included in the single rental above and must not be counted twice.

The corrected study changed only fence parsing and corresponding output instructions. Cases, seeds, policy order, sampling, model revision and budgets were unchanged. It started on the same warm server after the pilot, with prefix caching disabled. Corrected prompt IDs were rechecked against the live tokenizer before any generation.

## Recompute

```bash
python3 scripts/summarize-repository.py --experiment experiments/2026-10-12-naturalsize
```

Model/tokenizer: Qwen/Qwen3-8B at `b968826d9c46dd6066d109eabc6255188de91218`. vLLM 0.10.2, torch 2.8.0+cu128, BF16, 24,576-token context, one sequence, one A40 and prefix caching off. [Environment](environment.json) and [server evidence](server-evidence/) record the actual installation.

This is a development comparison on one known task. It cannot establish general quality, a new algorithm, or broad cost savings.
