# First live GPU fixture experiment

On 2026-10-12, Qwen3-8B on one rented A40 produced two truncated thinking
attempts and one passing non-thinking control on the pinned `merge-ranges` task.
This is a single public development fixture. It establishes live execution and
an important control to include; it does not establish early-exit savings.

| Attempt | Completion tokens | Client request/run time | Result |
| --- | ---: | ---: | --- |
| Thinking, original sampling | 4096 | 130.353 s | Truncated; no candidate tested |
| Thinking, explicit Qwen sampling | 4096 | 129.450 s | Truncated; no candidate tested |
| Non-thinking control | 126 | 4.806 s | Final answer; acceptance 4/4, evaluation 8/8 |

Thinking time is the Rust run's client wall time; control time is the standalone
HTTP request's client wall time, with another 0.326 s in Docker verification.
These are not engine GPU-seconds. Every attempt used seed 42, a 4096-token output
cap, the same model/tokenizer revision, and the same visible prompt. The control
chat template added four prompt tokens: 508 versus 504.

## What differed

The original request omitted top-p and top-k and the server used
`--generation-config vllm`. The second set temperature 0.6, top-p 0.95, and top-k
20. Both used the default thinking template and hit the fixed output cap.
The final control set `chat_template_kwargs.enable_thinking=false`, temperature
0.7, top-p 0.8, and top-k 20, following the model's non-thinking recommendations.
It ran through a standalone HTTP capture client and the existing Rust Docker
verifier, rather than the `baseline` CLI, which currently does not expose the
thinking switch. Response validation required one stopped choice, no tool calls,
and exactly one complete Python fence before verification.

The control changed several settings and ran on a warmed server with prefix
caching enabled. This is not a matched latency experiment or an isolated causal
test of thinking mode. It was selected after inspecting the initial failure and
model documentation. No evaluation results were used to edit the prompt or code.
A single task and seed cannot estimate general quality or repeatability.

## Evidence

Each attempt directory includes the exact synthetic request and response.
Thinking directories include the Rust configuration, task pins, events, and
failure summary. The control includes its declared plan, frozen candidate,
summary, and Docker verification output. The JSON summaries retain unknown
GPU-seconds as unknown.

- [Original thinking artifacts](thinking-original/request.json)
- [Explicit-sampling thinking artifacts](thinking-explicit-sampling/request.json)
- [Non-thinking request](nonthinking-control/request.json)
- [Generated fix](nonthinking-control/candidate.py)
- [Verification](nonthinking-control/verification.jsonl)
- [Environment and allocation](environment.json)
- [Installed packages](packages.txt)
- [Exact server command](server-args.json)

Full server logs and metrics snapshots are retained in local `runs/gpu-session-001`.
The non-thinking HTTP client was an experimental capture script; its request and
response are recorded here rather than being misrepresented as a supported CLI
policy. To reproduce that request, serve the pinned stack, POST its `request.json`
to `/v1/chat/completions`, validate a complete Python candidate, then run `verify`.
See [the GPU runbook](../../docs/gpu-baseline.md) for the serving and isolation setup.

## Cost and cleanup

The catalog and Pod read quoted $0.59 per GPU-hour. The recorded allocation
interval is at most 993 seconds, corresponding to approximately $0.163 of
compute. Disk charges and applicable taxes are additional; final billed cost is
pending because the billing read had no records yet. This was not a free run.
The experiment Pod was terminated and a fresh Pod list was empty. No network
volume was created.

## Implication for the next experiment

Include a non-thinking baseline before evaluating any stopping controller.
Add a proper recorded thinking-mode switch to the runner, then use a development
set with tasks where full reasoning can finish successfully. Keep failed and
truncated attempts in cost accounting. The next research milestone still needs
online candidate probes, continuation accounting, and held-out evaluation.

[Qwen's documented mode-specific sampling](https://huggingface.co/Qwen/Qwen3-8B/blob/b968826d9c46dd6066d109eabc6255188de91218/README.md#best-practices)
provided the settings for the separate controls.
