#!/usr/bin/env python3
"""Check the calibration record against captured usage, source and file hashes."""
import argparse
import hashlib
import json
from pathlib import Path


def report_value(root, key):
    return json.loads((root / 'report.json').read_text())[key]


def summarize(root):
    manifest = json.loads((root / 'evidence-manifest.json').read_text())
    for name, expected in manifest.items():
        if hashlib.sha256((root / name).read_bytes()).hexdigest() != expected:
            raise ValueError('capture hash mismatch: ' + name)
    trial = root / 'trials/baseline-seed-43'
    summary = json.loads((trial / 'summary.json').read_text())
    job = json.loads((root / 'queue/001-provider-result.json').read_text())
    response = json.loads((trial / 'response.json').read_text())
    if job['status'] != 'COMPLETED' or job['output'] != [response]:
        raise ValueError('provider and bridge response differ')
    for key in ['prompt_tokens', 'completion_tokens']:
        if response['usage'][key] != summary[key] or report_value(root, key) != summary[key]:
            raise ValueError('provider usage mismatch')
    report = json.loads((root / 'report.json').read_text())
    source = Path(__file__).resolve().parent.parent / 'tasks/repository/naturalsize-rollover/repo/humanize/filesize.py'
    candidate = (trial / 'candidate.py').read_bytes()
    if candidate != source.read_bytes():
        raise ValueError('reported unchanged candidate differs from source')
    if hashlib.sha256(candidate).hexdigest() != report['candidate_sha256']:
        raise ValueError('candidate hash mismatch')
    if report['calibration_outcome'] != summary['outcome'] or report['finish_reason'] != response['choices'][0]['finish_reason']:
        raise ValueError('outcome mismatch')
    if report['queue_execution_ms'] != job['executionTime'] or report['queue_delay_ms'] != job['delayTime']:
        raise ValueError('queue timing mismatch')
    for suite in ['acceptance', 'evaluation']:
        if report[suite] != {key: summary[suite][key] for key in ['passed', 'cases']}:
            raise ValueError('verification counts mismatch')
    if len(list((root / 'trials').iterdir())) != 1 or report['forced_model_attempts'] != 0:
        raise ValueError('unexpected follow-up trial')
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--experiment', type=Path, required=True)
    print(json.dumps(summarize(parser.parse_args().experiment), indent=2))
