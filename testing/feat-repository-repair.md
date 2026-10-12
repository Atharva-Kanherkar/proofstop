# Repository repair test contract

## Functional Behavior
- Add one historical real-repository development task: python-hyper/h11 v0.14.0 chunk-footer validation, already fixed upstream in v0.16.0. Do not describe it as a current or novel vulnerability.
- Pin the complete h11 runtime package, upstream identity, license, target file, specification, and both suites. The model replaces h11/_readers.py inside that package, rather than implementing a toy wrapper.
- Exercise the real h11.Connection server API with synthetic HTTP bytes split across receive_data calls. Require RemoteProtocolError for invalid nonzero-chunk footers and preserve valid, partial, multi-chunk and pipelined message behavior.
- Independently author acceptance and evaluation cases from the HTTP contract, before checking the upstream repaired file. Evaluation and reference code never enter generation or acceptance feedback.
- Sandbox mounts runtime code and trusted input-only worker, without expected answers, case names, upstream metadata, task manifests, or suite files. Only the target module changes. No network or host-execution fallback.
- Repository verification retains exact UTF-8 worker stdout for inspecting consumer outcomes; acceptance feedback still contains only case names and pass/fail.
- Baseline and bounded repair support repository tasks. Existing task formats and online policy remain compatible. Capture exact prompts and target source; evaluation cannot resume generation.

## Unit Tests
- Validate repository identity, fixed runtime file set, source hashes, paths and symlink rejection. Reject changed dependency modules as well as changed target files.
- Repository prompts contain target and selected dependency context but no evaluation names, upstream repaired source or advisory reference patch.

## Integration / Functional Tests
- Actual unmodified h11 v0.14.0 fails malformed-footer cases; a v0.16.0 target-file control passes independently authored acceptance and evaluation on the same v0.14.0 runtime.
- Check fragmented valid CRLF, split invalid footer, a second bad chunk, extensions/trailers, incomplete input, EOF and pipelined trailing data.
- A model-style replacement file runs through actual h11.Connection in Docker. Only package code and worker are mounted. No expected answers enter the container.
- All existing Rust, HTTP, Docker and Python screen tests remain green.

## Smoke Tests
- inspect --task tasks/repository/h11-chunk-footer --show-prompt validates the fixture.
- verify the archived target and repaired control via the CLI, with failure and success exit codes respectively.

## E2E Tests
- Local capture server plus real Docker exercises repository baseline and one-repair flow. This validates the harness, not model ability or GPU efficiency.
- Live GPU comparison on real repository tasks is a subsequent experiment after this workload and its verification contract are frozen. No GPU is needed to establish this task.

## Manual / cURL Tests
- cargo test --locked --all-targets
- cargo test --locked --tests -- --ignored --test-threads=1
- cargo fmt --check
- cargo clippy --locked --all-targets -- -D warnings
- python3 -B -m unittest discover -s tests -p 'test_screen_driver.py'
