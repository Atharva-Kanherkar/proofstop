#!/usr/bin/env python3
"""Recompute the repository study from raw usage and metric captures."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import tarfile


def counter(content, name):
    return sum(float(line.split()[-1]) for line in content.decode().splitlines()
               if line.startswith(name + '{') or line.startswith(name + ' '))


def summarize(experiment):
    rows = json.loads((experiment / 'results.json').read_text())
    plan = json.loads((experiment / 'plan.json').read_text())
    if [(row['seed'], row['policy']) for row in rows] != [(item['seed'], item['policy']) for item in plan['order']]:
        raise ValueError('attempts disagree with frozen order')
    prompt = (experiment / 'prompt.txt').read_text()
    if hashlib.sha256(prompt.encode()).hexdigest() != plan['prompt_sha256']:
        raise ValueError('frozen prompt hash mismatch')
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
    wire_manifest = json.loads((experiment / 'trial-manifest.json').read_text())
    for name, expected in wire_manifest.items():
        if hashlib.sha256((experiment / name).read_bytes()).hexdigest() != expected:
            raise ValueError('trial capture hash mismatch: ' + name)
    for row in rows:
        trial = experiment / 'trials' / row['label']
        stages = ['direct', 'repair'] if row['policy'] == 'repair' else ['']
        responses = [trial / (stage + '-response.json' if stage else 'response.json') for stage in stages]
        responses = [path for path in responses if path.exists()]
        expected_requests = row.get('accounting', {}).get('generation_requests', 1)
        if len(responses) != expected_requests or expected_requests > 2:
            raise ValueError('missing response or extra requests: ' + row['label'])
        envelopes = [json.loads(path.read_bytes()) for path in responses]
        for path, envelope in zip(responses, envelopes):
            request = json.loads(path.with_name(path.name.replace('response', 'request')).read_bytes())
            cap = plan['repair_budget'] if path.name == 'repair-response.json' else plan['thinking_budget'] if row['policy'] == 'thinking' else plan['direct_budget']
            if request['seed'] != row['seed'] or request['model'] != plan['model'] or request['max_tokens'] != cap or envelope['usage']['completion_tokens'] > cap:
                raise ValueError('request identity or output budget mismatch: ' + row['label'])
            if path.name != 'repair-response.json' and request['messages'][0]['content'] != prompt:
                raise ValueError('generation prompt differs from frozen task: ' + row['label'])
        for field in ['prompt_tokens', 'completion_tokens']:
            total = sum(item['usage'][field] for item in envelopes)
            if total != row['summary'][field]:
                raise ValueError('raw response usage mismatch: ' + row['label'])
        if row['engine_counters']['prompt_tokens_total'] != row['summary']['prompt_tokens']:
            raise ValueError('engine prompt tokens disagree: ' + row['label'])
        if row['policy'] == 'repair' and expected_requests == 2:
            events = [json.loads(line) for line in (trial / 'events.jsonl').read_text().splitlines()]
            authorized = next((i for i, e in enumerate(events) if e['event'] == 'repair_authorized'), None)
            evaluation = next((i for i, e in enumerate(events) if e['event'] == 'evaluation_finished'), None)
            if authorized is None or (evaluation is not None and evaluation < authorized):
                raise ValueError('repair authorization invariant failed: ' + row['label'])
            acceptance = events[authorized - 1]
            if acceptance['event'] != 'acceptance_finished' or acceptance['fields']['verification']['verdict'] != 'failed':
                raise ValueError('repair was not authorized by failed visible checks')
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
            'policy': policy, 'trials': len(trials), 'seeds': [x['seed'] for x in trials], 'successful_attempts': wins,
            'failures_by_reason': dict(Counter(x['summary'].get('error') or x['summary']['outcome'] for x in trials if x['summary']['outcome'] != 'passed')),
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
        direct = next(x for x in rows if x['policy'] == 'nonthinking' and x['seed'] == row['seed'])
        first = experiment / 'trials' / direct['label'] / 'response.json'
        second = experiment / 'trials' / row['label'] / 'direct-response.json'
        a = json.loads(first.read_text())['choices'][0]['message']['content']
        b = json.loads(second.read_text())['choices'][0]['message']['content']
        matched.append({'seed': row['seed'], 'direct_text_matches': a == b,
                        'requests': row['accounting']['generation_requests'],
                        'direct_passed': direct['summary']['outcome'] == 'passed',
                        'repair_passed': row['summary']['outcome'] == 'passed'})
    return {'classification': 'public_historical_development', 'groups': groups,
            'direct_stage_checks': matched, 'trial_count': len(rows),
            'client_interruption': json.loads((experiment / 'client-interruption.json').read_text()),
            'paired_outcomes': [{
                'seed': seed,
                **{policy: next(x['summary']['outcome'] for x in rows if x['seed'] == seed and x['policy'] == policy)
                   for policy in ['nonthinking', 'thinking', 'repair']}
            } for seed in sorted({row['seed'] for row in rows})], 'notes': [
                'Three seeds on one public historical repair task. Attempts are not independent tasks or held-out quality evidence.',
                'Engine counters measure request processing time, not CUDA kernel time.',
                'Seed 44 thinking allocation includes 773 seconds of observed client idle sleep. Do not treat allocation totals as an unbiased policy cost comparison.',
                'Trial allocation brackets include verifier waits, client process and metric calls; setup and inter-trial gaps are separate.',
                'Direct ceiling is 4096 tokens; thinking and direct-plus-repair both have total ceilings of 12288. Sampling profiles differ by mode.',
            ]}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--experiment', type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(summarize(args.experiment), indent=2))


if __name__ == '__main__':
    main()
