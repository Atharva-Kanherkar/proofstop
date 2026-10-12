#!/usr/bin/env python3
"""Recompute a development report from published trials and exact metric captures."""
import argparse
import hashlib
import json
from pathlib import Path
import tarfile


def counter(content, name):
    return sum(float(line.split()[-1]) for line in content.decode().splitlines()
               if line.startswith(name + '{') or line.startswith(name + ' '))


def summarize(experiment):
    rows = json.loads((experiment / 'results.json').read_text())
    manifest = json.loads((experiment / 'metrics-manifest.json').read_text())
    archive = experiment / 'metrics.tar.gz'
    if hashlib.sha256(archive.read_bytes()).hexdigest() != manifest['archive_sha256']:
        raise ValueError('metrics archive hash mismatch')
    captures = {}
    with tarfile.open(archive, 'r:gz') as tar:
        for info in tar.getmembers():
            if not info.isfile():
                raise ValueError('unexpected archive member')
            body = tar.extractfile(info).read()
            if hashlib.sha256(body).hexdigest() != manifest['files'].get(info.name):
                raise ValueError('metric capture hash mismatch')
            captures[info.name] = body
    if set(captures) != set(manifest['files']):
        raise ValueError('incomplete metrics archive')
    for row in rows:
        before = captures[row['label'] + '-metrics-before.txt']
        after = captures[row['label'] + '-metrics-after.txt']
        row['engine_counters'] = {
            name: counter(after, 'vllm:' + name) - counter(before, 'vllm:' + name)
            for name in ['request_inference_time_seconds_sum', 'request_prefill_time_seconds_sum',
                         'request_decode_time_seconds_sum', 'generation_tokens_total', 'prompt_tokens_total']
        }
        usage = row['summary']['completion_tokens']
        if usage is not None and row['engine_counters']['generation_tokens_total'] != usage:
            raise ValueError('engine tokens disagree with reported usage: ' + row['label'])
    rate = json.loads((experiment / 'allocation.json').read_text())['rate_usd_per_hour']
    groups = []
    for policy in ['nonthinking', 'thinking', 'repair']:
        trials = [x for x in rows if x['policy'] == policy]
        wins = sum(x['summary']['outcome'] == 'passed' for x in trials)
        seconds = sum(x['allocated_trial_seconds'] for x in trials)
        def usage(field):
            values = [x['summary'][field] for x in trials]
            return sum(values) if all(x is not None for x in values) else None
        groups.append({
            'policy': policy, 'trials': len(trials), 'successful_tasks': wins,
            'generated_tokens': usage('completion_tokens'), 'prompt_tokens': usage('prompt_tokens'),
            'client_wall_seconds': sum(x['summary']['wall_ms'] for x in trials) / 1000,
            'allocated_trial_seconds': seconds,
            'allocated_trial_seconds_per_success': seconds / wins if wins else None,
            'engine_request_seconds': sum(x['engine_counters']['request_inference_time_seconds_sum'] for x in trials),
            'trial_gpu_cost_usd_estimate': seconds * rate / 3600,
            'generation_requests': sum(x.get('accounting', {}).get('generation_requests', 1) for x in trials),
            'acceptance_false_positives': sum(
                x['summary']['acceptance'] is not None and x['summary']['acceptance']['verdict'] == 'passed'
                and x['summary']['evaluation'] is not None and x['summary']['evaluation']['verdict'] == 'failed'
                for x in trials),
            'repair_attempts': sum(x.get('accounting', {}).get('generation_requests', 1) == 2 for x in trials),
        })
    matched = []
    for row in rows:
        if row['policy'] != 'repair':
            continue
        direct = next(x for x in rows if x['policy'] == 'nonthinking' and x['task'] == row['task'])
        first = experiment / 'trials' / direct['label'] / 'response.json'
        second = experiment / 'trials' / row['label'] / 'direct-response.json'
        a = json.loads(first.read_text())['choices'][0]['message']['content']
        b = json.loads(second.read_text())['choices'][0]['message']['content']
        matched.append({'task': row['task'], 'direct_text_matches': a == b,
                        'requests': row['accounting']['generation_requests']})
    return {'classification': 'public_synthetic_development', 'groups': groups,
            'direct_stage_checks': matched, 'trial_count': len(rows), 'notes': [
                'One seed and one trial per policy per task. Public synthetic tasks, not held-out quality evidence.',
                'Engine counters measure request processing time, not CUDA kernel time.',
                'Trial allocation brackets include verifier waits, client process and metric calls; setup and inter-trial gaps are separate.',
                'A repair has two per-request budgets, while direct and thinking have one. Sampling profiles differ by mode.',
            ]}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--experiment', type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(summarize(args.experiment), indent=2))


if __name__ == '__main__':
    main()
