# Direct-first repair test contract

## Functional Behavior
- A direct request explicitly disables thinking. A passing acceptance verdict freezes the candidate and permits no further generation.
- A failed acceptance verdict permits exactly one request with thinking enabled, the original prompt, previous answer, and visible per-case pass/fail feedback. Evaluation never enters feedback.
- Transport errors, malformed or truncated answers, and inconclusive verification abort without retry. Failed repairs terminate.
- Evaluation scores the last frozen valid candidate after generation ends. Evaluation failure never resumes generation.
- Capture both requests and responses, configs, candidate versions, events, total usage and wall time. Missing usage makes aggregate counts null. Refuse existing output directories. The screen driver also refuses interrupted sessions before overwriting metrics or making network calls.
- Support ten diverse public synthetic development tasks through schema 2 with a fixed solution.py file and solve(single_array_argument) interface. Preserve schema 1 merge-ranges behavior and pinned file validation.

## Unit Tests
- Validate schema 2 identity and fixed file set; reject unknown schema, unsafe identity, changed hashes.
- Feedback contains only visible case names and pass/fail, with no evaluation cases or candidate output.
- Per-case scoring requires correct result or error and unchanged input. JSON numeric results treat exact integer/float equivalents equally, without rounding large integers.

## Integration / Functional Tests
- Real HTTP capture and Docker: direct pass makes one request; failure then successful repair makes two; failed repair never makes a third.
- Evaluation-only failure never generates repair. Inconclusive verification stops immediately.
- HTTP failures and truncation preserve incurred usage and raw bytes, without retry.
- Each of ten buggy sources fails acceptance; each separately stored reference passes acceptance and evaluation in Docker.

## Smoke Tests
- CLI help exposes repair; inspect validates all ten fixtures.
- Existing baseline and online tests remain green.

## E2E Tests
- Compare direct, full thinking, and direct plus one repair on the frozen ten-task development set with a pinned Qwen3/vLLM GPU setup. Capture failures as well as successes. This is a development screen, not held-out quality evidence.
- Count all generation and verification costs. Report engine request time separately from allocated GPU rental time; never substitute token savings for measured cost savings.

## Manual / cURL Tests
- cargo test --locked --all-targets
- cargo test --locked --tests -- --ignored --test-threads=1
- cargo fmt --check
- cargo clippy --locked --all-targets -- -D warnings
- cargo run --locked -- repair --model Qwen/Qwen3-8B --task tasks/dev/binary-search --output runs/repair-smoke
