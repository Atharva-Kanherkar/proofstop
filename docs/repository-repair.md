# Repairing a real repository file

The first repository workload replaces `h11/_readers.py` inside the actual h11
runtime package. It does not replace the parser with a standalone `solve` function.
The task is a historical, publicly documented defect, already fixed upstream.
It is a development workload, not a novel vulnerability or a held-out benchmark.

```mermaid
flowchart LR
    A[Pinned h11 runtime] --> B[Replace one parser file]
    B --> C[Fresh isolated Docker container]
    C --> D[Real h11.Connection API]
    D --> E[Host scores visible checks]
    E --> F[Stop or one bounded repair]
    F --> G[Separate evaluation]
```

## Pinned workload

- Repository: [python-hyper/h11](https://github.com/python-hyper/h11).
- Buggy runtime: v0.14.0, commit `467c5cfc5f9a1364c7da6b754d591092ee409931`.
- Target-file control: v0.16.0, commit `1c5b07581f058886c8bdd87adababd7d959dc7ca`.
- Upstream status checked October 13, 2026 in Asia/Tokyo: **historical-fixed**.
- Sources: [advisory](https://github.com/python-hyper/h11/security/advisories/GHSA-vqfr-h8mv-ghfj)
  and [0.16.0 changelog](https://h11.readthedocs.io/en/v0.16.0/changes.html).
- License: upstream MIT license is preserved in the runtime snapshot.

The archived parser skips two bytes after each nonzero chunk without checking
that they are CRLF. The task requires strict footer validation without breaking
incremental input. We demonstrate parser acceptance or rejection locally. We do
not demonstrate a proxy exploit or claim a current h11 vulnerability.

The snapshot contains all eleven Python runtime modules, `py.typed`, and the
license, copied byte-for-byte from the pinned commit. It omits upstream tests and
packaging files. h11 has no external runtime dependencies. Each file, both suites,
the spec and upstream metadata are hashed in the task manifest. The loader keeps
validated bytes in memory, so later filesystem changes do not change execution.
The manifest is a versioned research artifact, not a signed provenance certificate.

## Acceptance and evaluation

Four visible acceptance examples cover valid input, a malformed footer, fragmented
CRLF and an empty body. Twelve separately authored evaluation cases cover:

- Invalid first and second footer bytes arriving in separate fragments.
- A malformed footer on a later chunk.
- Correct footer bytes arriving one at a time.
- Multiple chunks, extensions and trailers.
- Incomplete input and premature EOF.
- Trailing pipelined bytes and unrelated Content-Length handling.
- An entire request arriving byte by byte.

The cases were frozen before inspecting the upstream repaired target. They were
written from the framing contract, not copied from upstream tests. This does not
make them held-out research evidence: all cases and the historical fix are public.

Inputs are arrays of hex-encoded fragments. The input-only worker feeds them to
`h11.Connection(h11.SERVER).receive_data`, drains `next_event`, combines `Data`
payloads, records `EndOfMessage`, and returns the remaining `trailing_data` bytes.
Rust keeps expected outputs and compares them outside Docker. An empty fragment
means EOF. Protocol errors must propagate through the real connection API.

The model sees the buggy target, read-only ReceiveBuffer context and acceptance
examples. It receives neither the repaired file nor evaluation cases or upstream
metadata. A repair receives only the previous candidate and visible pass/fail
status. Repository verification captures exact UTF-8 worker stdout in the ledger
for auditing consumer outcomes; this output is excluded from repair feedback.

The sandbox contains only `repo/` and the worker. Only `_readers.py` is replaced.
Dependencies remain byte-identical and read-only. Expected answers, suite files,
task names, manifests and upstream metadata are not mounted. Existing Docker
isolation, timeouts, cleanup and output limits apply.

## Run locally

From the repository root, with the pinned image already pulled:

```bash
cargo run --locked -- inspect --task tasks/repository/h11-chunk-footer --show-prompt
cargo run --locked -- verify \
  --task tasks/repository/h11-chunk-footer \
  --candidate tasks/repository/h11-chunk-footer/repo/h11/_readers.py
```

The second command fails as expected: acceptance 3/4 and evaluation 9/12.
The fixed-file control passes 4/4 and 12/12 on the same old runtime.
[Three repeated reproductions and controls](../experiments/2026-10-12-h11-harness/README.md)
preserve exact consumer output and input wire bytes:

```bash
cargo run --locked -- verify \
  --task tasks/repository/h11-chunk-footer \
  --candidate tests/fixtures/repository/h11-readers-fixed.py
cargo test --locked --test repository -- --ignored --test-threads=1
```

An intentionally incomplete fix checks only the first footer byte. It passes all
visible cases but fails evaluation. The end-to-end test proves that evaluation
failure cannot generate another repair.

## Next GPU experiment

The baseline and `repair` CLI now accept this task path. Freeze the serving
context, output budgets and trial order before running them. Tokenize the complete
prompt and reserve room for a full replacement file and the expanded repair
prompt. Do not blindly reuse the previous 8,192-token server context. The existing
screen driver supports the ten synthetic tasks only; this repository workload
needs a separately frozen experiment plan.

This change establishes a reproducible repository task and its consumer boundary.
Its inference flow is tested with a local capture server and real Docker, which
validates the harness but does not measure model ability or GPU savings. No GPU
was rented for this step. Add more independently evaluated repository workloads
before making a broad efficiency claim.
