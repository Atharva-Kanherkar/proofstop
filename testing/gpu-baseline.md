# GPU baseline preparation: acceptance contract

## Functional behavior

- Pin the Qwen3-8B model and tokenizer revision, compatible vLLM version,
  precision, context length, seed, and single-request execution settings.
- Keep the model server private and connect through an SSH tunnel.
- Use the existing Rust runner and local Docker verifier; generated code never
  executes on the GPU machine or directly on the host.
- Allow a 600-second request timeout without changing the 4096-token budget.
- State the authorized $25 total ceiling and record paid allocation separately
  from request wall time. No early-exit or savings claim follows from this run.
- Do not provision until funding is confirmed. Stop at an account funding error.
- Capture environment identity, serving logs, metrics snapshots, and run artifacts.
- Remove experiment resources after downloading evidence, with read-back.

## Unit tests

No new application logic. Validate the new configuration with the existing loader.

## Integration and functional tests

Run the existing Rust tests and Docker reference verification with the GPU config.

## Smoke tests

Check the pinned revision against Hugging Face and vLLM compatibility metadata.
After provisioning, verify the actual GPU and engine and call `/v1/models` through
SSH before any baseline request.

## End-to-end tests

A live model request must produce a recorded outcome, including failure or
truncation. Model success is measured rather than required for harness validity.
Live execution remains pending funding and provisioning.

## Manual checks

Review the runbook, configuration, and README link. Check JSON and diff whitespace.

## Sampling correction after the first live attempt

- Preserve the original 4096-token truncated attempt unchanged.
- Add optional `top_p` and `top_k` configuration. Existing configs without them
  retain their request shape. Validate top-p in (0, 1] and top-k as -1 or positive.
- Emit configured sampling fields in the saved and transmitted request bytes.
- Pin the GPU config to Qwen recommended thinking sampling: temperature 0.6,
  top-p 0.95, top-k 20. Keep seed 42 and the 4096-token budget unchanged.
- Test absent-field compatibility, configured wire fields, and invalid values.
- Rerun through the existing Rust runner into a new output directory. Treat this
  as a separate sampling control rather than overwriting the initial experiment.
