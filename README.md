# ProofStop

**Cost-aware early exit for verifiable reasoning.**

ProofStop is a proposed open-source inference controller that decides when to
continue reasoning, probe a candidate answer, or stop generation. Its objective
is to reduce compute cost per successful task, including the cost of verification.

**Status:** research proposal. This repository contains the implementation and
evaluation plan. There is no runtime, installation procedure, or measured
efficiency result yet.

## Research question

Can an online controller reduce the total compute required to solve a task by
testing candidate answers before a reasoning model finishes its natural
continuation?

An earlier answer can be cheaper, but extracting and checking it also costs
compute. Frequent probes can erase the savings. An incomplete verifier can accept
an incorrect answer. Cancellation may arrive after additional tokens have been
generated. ProofStop will measure these costs and failure modes explicitly.

The initial hypothesis is that a controller which adapts its probing schedule can
outperform both a fixed reasoning budget and periodic probing on tasks with
executable acceptance checks. This is a hypothesis to test, not an established
advantage or a claim of a new early-stopping algorithm.

## Initial scope

The first workload is Python function synthesis with a provided function
signature, task description, and executable acceptance checks. A separately
held-out test suite evaluates the returned function.

The controller receives acceptance outcomes, not evaluation results. Neither
test suite is included in model prompts. Passing acceptance checks means only
that those checks passed; it does not establish general program correctness.

The first experiment uses one open-weight reasoning model, one GPU, sequential
requests, and bounded context and output lengths. The provisional model is
[Qwen3-8B](https://huggingface.co/Qwen/Qwen3-8B), whose official model card documents
thinking and non-thinking modes. Its exact revision, tokenizer, chat template,
precision, and inference dependencies must be pinned before execution.

Initially out of scope: model training, open-ended answer grading, distributed
serving, multi-model routing, and production deployment.

## Proposed architecture

```mermaid
flowchart TD
    A[Task and pinned configuration] --> B[Generate reasoning]
    B --> C{Controller decision}
    C -->|Continue| B
    C -->|Probe| D[Isolated candidate branch]
    D --> E[Execute acceptance checks]
    E -->|Reject or inconclusive| B
    E -->|Accept| F[Stop parent and return candidate]
    C -->|Budget exhausted| G[Finalize within remaining budget]
    F --> H[Held-out evaluation and cost ledger]
    G --> H
```

| Component | Responsibility |
| --- | --- |
| Inference adapter | Track token IDs and checkpoints; extract a candidate without corrupting the parent; stop generation and account for any work after the stop decision. |
| Probe controller | Select checkpoints and enforce limits on probe count, candidate length, and total work. |
| Acceptance verifier | Execute predefined checks in an isolated environment and return accept, reject, or inconclusive. |
| Cost ledger | Record parent and probe tokens, prefill and decode work, cache overhead, verifier time, and completion or cancellation events. |
| Evaluator | Score returned answers with held-out tests and compare paired runs across policies. |

A candidate branch is a logical requirement, not an assumption that an HTTP API
supports atomic KV-cache forking. The first adapter may replay the exact token
prefix, with cache reuse where supported. It must measure replay and cache costs.
Direct state reuse is a later optimization if the experiment warrants it.

The candidate extraction protocol, including any intervention that ends thinking,
must be explicit and identical across applicable baselines. A request-level
controller must not pause unrelated requests through an engine-wide pause API.

### Runtime invariants

- Probing must not alter the parent's token prefix, sampling state, or cache.
- Rejecting a candidate must preserve the parent's ability to continue.
- Acceptance requires a complete, parseable candidate and completed checks.
- Parse failures, timeouts, crashes, and uncertain verifier outcomes do not count
  as acceptance.
- Held-out evaluation must remain inaccessible to the controller.
- Every probe and continuation counts toward the total compute budget.
- Reaching the budget produces a bounded finalization attempt or an explicit
  failure, never an unbounded retry loop.
- A stop decision and confirmed engine termination are separate ledger events.

## Implementation plan

### Milestone 0: Lock the experimental contract

Select and pin the model, engine, tokenizer, hardware, decoding settings, dataset
revision, and acceptance/evaluation split. Define the candidate extraction method
and verify that the selected backend can continue from an exact token prefix.

Create separate development and evaluation partitions. Use development tasks to
choose budgets and checkpoints; freeze the policy before evaluating it. Record
GPU rental terms and a spending ceiling before provisioning hardware.

**Exit criterion:** a versioned configuration and task manifest sufficient for
another person to repeat the experiment.

### Milestone 1: Establish the baseline and accounting

Run unmodified reasoning to natural completion within a fixed maximum budget.
Save token-level output and timing events for synthetic or licensed workloads.
Add the isolated Python test runner and held-out evaluator.

Implement a segmented continuation control with no probes. Compare it with the
uninterrupted baseline to isolate checkpoint and replay overhead before attributing
any effect to early stopping.

**Exit criterion:** reproducible baseline scores and an event ledger that accounts
for all requests, failures, and compute stages.

### Milestone 2: Implement bounded periodic probing

At configured checkpoints, generate a bounded candidate, execute acceptance
checks, and stop the parent only after acceptance. Continue after rejection or an
inconclusive outcome. Initially serialize probes and continuation so accounting
and state ownership are easy to inspect.

Test branch isolation, continuation after rejection, malformed output, verifier
timeouts, budget exhaustion, and cancellation. Add exact-check domains such as
arithmetic or constraint satisfaction to distinguish verifier weakness from
controller defects.

**Exit criterion:** a working online loop with invariant coverage and complete
accounting. No retrospective token truncation is presented as online savings.

### Milestone 3: Run the first feasibility experiment

Evaluate 30 held-out coding tasks across the policies below. Pair runs by task
and seed. Use identical prompts, model weights, decoding settings, precision,
and maximum total budgets, except for the policy under test.

This is a feasibility screen. It can reveal overhead or obvious failure modes;
it cannot establish a small accuracy difference or general efficiency claim.

**Exit criterion:** a reproducible report covering successes, failures, acceptance
errors, latency, and total cost for every policy, including negative results.

### Milestone 4: Add a cost-aware controller

Start with a simple policy that adjusts probe intervals using observed probe
cost, recent rejection outcomes, and remaining budget. Compare it with periodic
probing at several fixed intervals. Do not introduce a learned controller until
simple policies justify that additional complexity.

Tune only on development tasks. Freeze parameters before a larger evaluation
with multiple seeds, confidence intervals, and a second verifiable workload.

**Exit criterion:** a demonstrated benefit over the strongest tested baseline,
or a documented finding that adaptive probing does not help.

### Milestone 5: Integrate with a serving engine

If online probing produces net savings, optimize the adapter for state reuse,
request-local cancellation, and bounded asynchronous verification. Evaluate the
controller under concurrency and measure throughput, tail latency, and memory
pressure. Include an ablation that separates state reuse from policy improvements.

**Exit criterion:** serving-level savings without violating runtime invariants or
the predeclared task-success requirement.

## Evaluation protocol

### Required baselines

| Policy | Purpose |
| --- | --- |
| Full reasoning | Natural completion with a fixed maximum budget. |
| Short fixed budget | Test whether simply allocating less reasoning is sufficient. |
| Segmented continuation without probes | Isolate checkpoint, replay, and cache overhead. |
| Periodic probing | Establish a simple online early-exit baseline. |
| Adaptive probing | Test the proposed cost-aware controller. |

Include relevant interwhen or PUMA policies where a faithful, reproducible
comparison is practical. Distinguish offline analyses from online execution.

### Metrics

The primary metric is **allocated GPU-seconds per held-out successful task**.
For the initial single-GPU sequential benchmark, divide total synchronized
end-to-end benchmark duration by the number of successful tasks. Include failed
attempts, probe work, replay, and time the allocated GPU waits for verification.
An experiment with zero successes has no finite cost-per-success value.

Report GPU kernel timing separately where available. Allocated device time is
not the same as GPU utilization, energy consumption, or an API token bill.

Also report:

- Held-out success rate, paired outcomes, and acceptance false positives.
- Parent reasoning tokens, candidate tokens, and all other generated tokens.
- Prefill work, measured cache reuse, and checkpoint or replay overhead.
- Verifier CPU time, wall time, timeouts, and unsuccessful probes.
- End-to-end latency, including median and tail percentiles.
- Stop-decision-to-termination delay and work performed during that delay.
- Peak GPU memory and, later, throughput at several concurrency levels.
- Actual rental expenditure, with setup and warm-up costs reported separately.

Token reduction alone is not an efficiency result. Offline analysis that first
generates a complete trace cannot substantiate online compute savings.

### Go / no-go criteria

The provisional target for a larger evaluation is at least **15% lower allocated
GPU-seconds per successful task** than the strongest baseline. This is a target,
not a prediction.

Before that evaluation, predeclare a task-success non-inferiority margin, sample
size, and paired uncertainty analysis. A proposed starting margin is two
percentage points. Do not claim success merely because a small experiment fails
to detect a regression.

Proceed to engine integration only if the larger experiment supports both net
savings and the predeclared success criterion. Rework or stop the approach if
verification overhead erases the benefit, acceptance errors exceed the chosen
tolerance, or a fixed budget performs as well at lower cost.

## Reproducibility

Each run must record:

- Repository commit, model and tokenizer revisions, engine version, and dependency lock.
- GPU model, driver, CUDA version, precision, and relevant cache configuration.
- Dataset revision, task IDs, partition assignments, prompts, and test-suite versions.
- Sampling settings, seeds, budgets, probe schedule, and candidate extraction protocol.
- Per-request token IDs, timing events, verifier outcomes, and termination status.
- Aggregated results with all failures included and a command to regenerate the report.

Use synthetic or appropriately licensed workloads. Keep credentials and private
prompts out of artifacts. Generated code must execute in a disposable environment
with networking disabled and bounded CPU, memory, process count, and runtime.

## Related work

ProofStop builds on existing work. Its intended contribution is an online
cost-aware controller and reproducible serving measurements; that contribution
remains to be established.

| Project | Relevant work | Implication for ProofStop |
| --- | --- | --- |
| [interwhen](https://github.com/microsoft/interwhen) | Intermediate verification, inference monitoring, and early stopping. | Reuse ideas and compare against appropriate monitors; verification during generation is not a novelty claim. |
| [PUMA](https://github.com/giovanni-vaccarino/PUMA) | Progress-aware early exit using redundancy detection and answer verification. Its README currently identifies the released pipeline as offline. | Measure actual online cost and inspect future online releases before claiming a distinct contribution. |
| [vLLM](https://docs.vllm.ai/en/latest/) | Serving, prefix caching, and generation lifecycle primitives. | A candidate backend; validate request-local control and cache behavior rather than assuming fork support. |
| [Qwen3-8B](https://huggingface.co/Qwen/Qwen3-8B) | Open-weight model with documented thinking and non-thinking modes. | A provisional model for the initial single-GPU experiment. |

Related-work references were checked on October 12, 2026. The landscape and public
APIs can change; recheck them before freezing an implementation or publication.

## Contributing

The first useful contributions are the pinned baseline runner, cost ledger,
isolated verifier, and inference adapter described above. Open a design issue
before adding a backend or changing the evaluation contract.

Contributions should include invariant-level tests and reproducible evidence for
performance claims. Negative results are useful. Keep raw licensed experiment
artifacts separate from source and document how results were derived.

## License

ProofStop is licensed under the [MIT License](LICENSE). Models, datasets, and
dependencies retain their respective licenses.
