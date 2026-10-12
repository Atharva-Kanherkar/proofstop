#!/usr/bin/env python3
"""Run a frozen development plan against an already provisioned private server.

Provisioning and cleanup are intentionally performed separately. This driver
refuses to overwrite trials, captures all outcomes, and never retries a trial.
"""
import argparse
import datetime
import json
from pathlib import Path
import subprocess
import time
import urllib.request


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--session', type=Path, required=True)
    parser.add_argument('--endpoint', default='http://127.0.0.1:18000')
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    session = args.session.resolve()
    # Check incomplete sessions too, before any network call or capture write.
    marker = session / 'screen-started.json'
    if marker.exists() or (session / 'results.json').exists() or (session / 'trials').exists() or any(session.glob('*-metrics-*.txt')):
        raise RuntimeError('refusing to rerun an existing or interrupted screen')
    plan = json.loads((session / 'plan.json').read_text())
    allocation = json.loads((session / 'allocation.json').read_text())
    deadline = datetime.datetime.fromisoformat(allocation['deadline_utc'].replace('Z', '+00:00'))
    head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
    if head != plan['source_commit']:
        raise RuntimeError('source commit differs from frozen plan')
    if (session / 'results.json').exists():
        raise RuntimeError('refusing to rerun an existing screen')
    for entry in plan['set']['tasks']:
        import hashlib
        digest = hashlib.sha256((root / entry['path'] / 'manifest.json').read_bytes()).hexdigest()
        if digest != entry['manifest_sha256']:
            raise RuntimeError('task set drift')
    # Exclusive creation also prevents two drivers racing on a fresh directory.
    with marker.open('x') as file:
        json.dump({'source_commit': head, 'started_utc': datetime.datetime.now(datetime.timezone.utc).isoformat()}, file)
        file.write('\n')
    results = []
    def snapshot(path):
        with urllib.request.urlopen(args.endpoint + '/metrics', timeout=15) as response:
            path.write_bytes(response.read(1024 * 1024))
    for index, item in enumerate(plan['order'], 1):
        if datetime.datetime.now(datetime.timezone.utc) + datetime.timedelta(minutes=21) >= deadline:
            raise RuntimeError('allocation deadline approaching; stop and clean up')
        label = f"{index:02}-{item['task']}-{item['policy']}"
        output = session / 'trials' / label
        command = ['cargo', 'run', '--locked', '--', 'repair' if item['policy'] == 'repair' else 'baseline',
                   '--task', 'tasks/dev/' + item['task'], '--endpoint', args.endpoint + '/v1',
                   '--model', plan['model'], '--output', str(output)]
        if item['policy'] != 'repair':
            config = 'configs/gpu-nonthinking.json' if item['policy'] == 'nonthinking' else 'configs/gpu-thinking.json'
            command += ['--config', config]
        bracket_started = time.time()
        snapshot(session / (label + '-metrics-before.txt'))
        print('START', label, flush=True)
        started = time.time()
        with (session / (label + '-command.log')).open('wb') as log:
            completed = subprocess.run(command, cwd=root, stdout=log, stderr=subprocess.STDOUT, timeout=1250)
        ended = time.time()
        summary = json.loads((output / 'summary.json').read_text())
        snapshot(session / (label + '-metrics-after.txt'))
        row = {**item, 'label': label, 'command': command, 'started_unix': started, 'ended_unix': ended,
               'allocated_trial_seconds': time.time() - bracket_started,
               'exit_code': completed.returncode, 'summary': summary}
        if (output / 'accounting.json').exists():
            row['accounting'] = json.loads((output / 'accounting.json').read_text())
        results.append(row)
        (session / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
        print('RESULT', label, summary['outcome'], summary['completion_tokens'], summary['wall_ms'], summary.get('error'), flush=True)
        # A budget-truncated answer is a measured failure. Other unexpected
        # protocol, candidate-format, or infrastructure faults require review.
        if summary.get('error') and summary['error'] != 'completion did not finish normally':
            raise RuntimeError('unexpected failure; do not conceal or retry: ' + summary['error'])
    print('ALL_TRIALS_FINISHED', flush=True)


if __name__ == '__main__':
    main()
