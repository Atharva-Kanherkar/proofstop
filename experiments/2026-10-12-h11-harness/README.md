# Historical h11 repository task: consumer verification

This validates the repository repair harness, not model ability or inference
savings. No model request or GPU rental occurred in this step.

The buggy v0.14.0 runtime consistently accepts malformed chunk footers through
its real `h11.Connection` server API. Replacing only `_readers.py` with the archived
v0.16.0 file consistently rejects them and passes all sixteen checks.

| Candidate | Repetitions | Acceptance | Evaluation |
| --- | ---: | ---: | ---: |
| Archived v0.14.0 target | 3 | 3/4 every run | 9/12 every run |
| v0.16.0 target on unchanged v0.14.0 runtime | 3 | 4/4 every run | 12/12 every run |

For invalid footers, the old consumer reports a complete message and a body;
the control reports `RemoteProtocolError`. These observed outputs are preserved,
rather than inferring acceptance from a failed assertion. No proxy exploit is
claimed. This is an already-fixed historical public development task.

Source: `2dce90f4ea27f20bc825e6ca0a58a35f30c468d3`.
Runtime commit: `467c5cfc5f9a1364c7da6b754d591092ee409931`.
Control target commit: `1c5b07581f058886c8bdd87adababd7d959dc7ca`.
The [task metadata](../../tasks/repository/h11-chunk-footer/upstream.json)
records source links, license and scope. The snapshot contains all runtime
modules and needs no external dependencies beyond the pinned Python image.

The suites were independently authored and committed in `ce84460` before the
repaired target was inspected. They are public and therefore not held-out
research evaluation. The control validates this narrow target-file replacement,
not an upgrade of the whole runtime.

Evidence:

- [results.json](results.json): commands, exit codes, target hashes, per-case verdicts and exact worker stdout.
- `transcripts/`: exact UTF-8 consumer output and CLI stdout for all six runs.
- [input-fragments.json](transcripts/input-fragments.json): exact receive_data boundaries, including EOF; corresponding `.bin` files contain concatenated HTTP bytes.
- [evidence-manifest.json](evidence-manifest.json): SHA256 for every capture and the structured results.

Rust compares returned data and errors with expected answers outside Docker.
The input-only worker receives hex fragments, not suite labels or expectations.
Container files are limited to the read-only runtime and worker. Repository
stdout is retained for auditing, but only visible pass/fail status can enter
repair feedback.

An additional real-Docker test applies an intentionally incomplete fix: validate
only the first footer byte. Acceptance passes, evaluation fails on the invalid
second byte, and the controller makes no further generation request. Local HTTP
capture tests also verify baseline and one-repair execution using full real
replacement modules. Those responses are deterministic harness controls, not
model results.

Reproduce from the repository root:

```bash
cargo run --locked -- verify \
  --task tasks/repository/h11-chunk-footer \
  --candidate tasks/repository/h11-chunk-footer/repo/h11/_readers.py
cargo run --locked -- verify \
  --task tasks/repository/h11-chunk-footer \
  --candidate tests/fixtures/repository/h11-readers-fixed.py
cargo test --locked --test repository -- --ignored --test-threads=1
```

The first command exits unsuccessfully as expected. See the
[repository runbook](../../docs/repository-repair.md) for limitations and the
context/budget checks required before a live GPU comparison.
