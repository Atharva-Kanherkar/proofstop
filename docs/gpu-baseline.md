# First GPU baseline

This is a development fixture run, not an efficiency benchmark. The authorized
ceiling is $25 total, including setup, compute, storage, and applicable taxes.
Start with a four-hour allocation limit and remove experiment resources after
copying the evidence locally. A shell timeout does not stop rental billing.

## Locked experiment

| Setting | Value |
| --- | --- |
| Model | `Qwen/Qwen3-8B` |
| Model and tokenizer revision | `b968826d9c46dd6066d109eabc6255188de91218` |
| Engine | `vllm==0.10.2` |
| Torch | `2.8.0`, as required by vLLM 0.10.2 |
| Transformers | `4.55.2` |
| Precision | BF16, no quantization |
| Context | 8192 tokens |
| Tensor parallelism / concurrent sequences | 1 / 1 |
| GPU memory utilization | 0.85 |
| Generation | 4096 tokens maximum; temperature 0.6; seed 42 |
| Request timeout | 600 seconds |
| Task | `merge-ranges`, `synthetic-v1` |
| Expected GPU | One Ampere GPU with 48 GB; record the actual allocated SKU |

The engine version matches the CUDA 12.8 / Torch 2.8 template stack. This is a
compatibility pin, not a claim that this is the newest engine. Transitive packages
must be captured with `pip freeze`; they are not fully locked by these pins.
Record the actual driver, GPU UUID, image digest, package versions, and allocation
start/end before treating this as reproducible evidence.

## Funding and provisioning

Runpod requires prepaid credits. If empty, add $10 first with auto-pay disabled.
Read live stock and price before provisioning. Use an official PyTorch 2.8 / CUDA
12.8 template, with SSH enabled, a registered public key, and only `22/tcp`
exposed. Do not expose an unauthenticated model API or enable Jupyter.
Use ephemeral storage for this disposable run; do not create a network volume.
Copy all evidence before removing the Pod. An account balance is not a precise
experiment spending limiter; enforce the allocation deadline separately.

## GPU machine setup

Run only on the rented GPU machine, in a dedicated environment. Never upload an
API key or an SSH private key to the Pod.

```sh
python3 -m venv /workspace/proofstop-venv
/workspace/proofstop-venv/bin/pip install \
  'vllm==0.10.2' 'transformers==4.55.2'
mkdir -p /workspace/proofstop-evidence
/workspace/proofstop-venv/bin/pip freeze \
  > /workspace/proofstop-evidence/packages.txt
nvidia-smi -q > /workspace/proofstop-evidence/gpu.txt
```

Run the server under a supervised process, saving stdout/stderr to
`/workspace/proofstop-evidence/server.log`. The startup command is:

```sh
/workspace/proofstop-venv/bin/vllm serve Qwen/Qwen3-8B \
  --revision b968826d9c46dd6066d109eabc6255188de91218 \
  --tokenizer-revision b968826d9c46dd6066d109eabc6255188de91218 \
  --host 127.0.0.1 --port 8000 \
  --dtype bfloat16 --max-model-len 8192 \
  --tensor-parallel-size 1 --max-num-seqs 1 \
  --gpu-memory-utilization 0.85 --seed 42 \
  --generation-config vllm
```

`--generation-config vllm` prevents a model repository generation configuration
from silently overriding serving defaults. Leave the Qwen thinking template at
its default. Do not change the prompt or settings in response to evaluation tests.

## Local verification and request

Use the actual direct SSH address returned by Runpod. Forward local port 8000 to
remote `127.0.0.1:8000` with `ssh -N -L`; use a dedicated known-hosts file.
Verify `/v1/models` and save `/metrics` before and after the request. These
snapshots supplement the ledger; they do not measure allocated GPU-seconds.

From the ProofStop repository root, with local Docker running and the pinned
Python image already pulled:

```sh
cargo run --locked -- verify --config configs/gpu-baseline.json \
  --candidate tests/fixtures/reference.py
curl --fail http://127.0.0.1:8000/v1/models
cargo run --locked -- baseline --config configs/gpu-baseline.json \
  --endpoint http://127.0.0.1:8000/v1 \
  --model Qwen/Qwen3-8B --output runs/gpu-baseline-001
```

The runner saves failures as well as passing candidates. A truncated answer is a
failed attempt; do not drop it or quietly expand its budget. Repeat only with
unique output directories and explicitly record each attempt.

## Evidence and cleanup

Save the Pod identity, quoted rate, actual allocation interval, GPU information,
package list, server arguments/log, metrics snapshots, and task run directories.
Separate model loading/warmup from requests while retaining both in total cost.
The existing summary leaves GPU-seconds and hardware fields unknown; a companion
record supplies environment identity, not a replacement measurement.

Download the remote evidence, terminate only the Pod created for this experiment,
and confirm it is absent with a fresh Pod list. Record actual billed cost when
billing records arrive. Never infer token savings or quality generalization from
this single public task.

Sources checked during preparation:

- [Pinned model](https://huggingface.co/Qwen/Qwen3-8B/tree/b968826d9c46dd6066d109eabc6255188de91218)
- [vLLM 0.10.2 installation](https://docs.vllm.ai/en/v0.10.2/getting_started/installation/gpu.html)
- [Runpod billing](https://docs.runpod.io/accounts-billing/billing)
