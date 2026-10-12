# First baseline: acceptance contract

## Functional behavior

- Lock a Rust controller with an HTTP inference backend and a disposable Docker
  Python verifier. Python is the task language, not the controller language.
- Add one pinned, synthetic miniature repository with a nested-interval bug in
  `merge_ranges`. Document its contract and minimal failing input.
- Include visible acceptance examples in the model prompt. Keep evaluation
  cases and reference outputs out of inference requests and candidate containers.
- Verify hashes before using task files. Reject changed fixtures and unsupported
  task/schema versions instead of silently running a different workload.
- Provide `inspect`, `verify`, and `baseline` commands. Baseline uses a bounded,
  single non-streaming generation request against a user-selected vLLM-compatible
  server. It does not implement early exit or claim compute savings.
- Require a digest-pinned Python Docker image for execution. Disable networking,
  run without privileges, mount candidates read-only, and bound resources and time.
- Never execute generated code directly on the host. Kill and remove containers
  after timeout or error, and fail closed when Docker is unavailable.
- Accept only one complete Python code block as a candidate; reject truncation,
  ambiguity, empty output, and unsupported inference finish reasons.
- Create a new run directory without overwriting previous results. Save exact
  synthetic request/response bodies, frozen candidate source, timing and usage
  events, and an explicit outcome for every attempted baseline request.
- Record missing token counts and GPU timing as unknown. Do not relabel client
  wall time as measured GPU-seconds or infer a savings result from mock responses.

## Unit tests

- Fixture hashes reject tampering, unsafe paths, and unsupported versions.
- Prompt construction excludes evaluation cases and their expected outputs.
- Candidate parsing rejects incomplete, multiple, or empty code blocks.
- Inference parsing rejects truncated and missing choices and preserves missing
  usage as unknown.
- Verifier distinguishes passed, failed, and inconclusive; malformed or extra
  output, timeouts, and process errors never become acceptance.
- Resource limits, image digest validation, and read-only Docker arguments are
  tested at their configuration boundary.

## Integration and functional tests

- A real local HTTP capture server records a baseline request and returns a
  deterministic fixture response. Verify the wire body and outcome artifacts.
- HTTP failures and invalid/truncated responses leave recorded failed runs.
- Reusing an output directory fails without overwriting its contents.
- Real Docker: the original bug fails acceptance, the reference fix passes both
  suites, a deliberately overfitted candidate passes acceptance but fails
  evaluation, and an infinite loop times out with no surviving container.

## Smoke tests

- `cargo run -- inspect --task tasks/merge-ranges` prints the pinned task identity.
- `cargo run -- verify` exercises real Docker on the buggy and fixed solutions.

## End-to-end tests

- Mock inference plus real Docker proves the HTTP-to-candidate-to-verifier-to-ledger
  integration. Label this explicitly as harness validation, not model evidence.
- Live GPU model execution is pending an authorized spending ceiling and access.
  Do not call the experimental or GPU milestone complete without that run.

## Manual checks

- Run `cargo test --all-targets`, `cargo fmt --check`, and
  `cargo clippy --all-targets -- -D warnings`.
- Review the implementation against this contract and the README invariants.
- Update README status, runnable commands, architecture, first task, and remaining
  milestones to describe exactly what exists.
- Scan committed files for credentials and em dashes. Preserve unrelated work.
