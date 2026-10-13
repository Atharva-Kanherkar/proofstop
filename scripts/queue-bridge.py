#!/usr/bin/env python3
"""Private HTTP bridge to an authenticated external queue broker.

No credentials are accepted or persisted. The broker reads pending JSON, submits
one queue job, saves its complete provider envelope separately, then writes the
response. This bridge never retries a request or provisions hardware.
"""
import argparse
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
from pathlib import Path
import time

ROUTES = {'/tokenize', '/detokenize', '/v1/chat/completions', '/v1/completions'}


def policy(route, body):
    if route not in ROUTES:
        raise ValueError('unsupported route')
    if route in {'/tokenize', '/detokenize'}:
        return {'executionTimeout': 30000, 'ttl': 1200000}
    cap = body.get('max_tokens')
    if not isinstance(cap, int) or isinstance(cap, bool) or not 1 <= cap <= 16384:
        raise ValueError('generation requires a bounded token cap')
    seconds = 60 if cap <= 512 else 180 if cap <= 2048 else 600
    return {'executionTimeout': seconds * 1000, 'ttl': 1200000}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--spool', type=Path, required=True)
    parser.add_argument('--port', type=int, default=18001)
    args = parser.parse_args()
    args.spool.mkdir(parents=True, exist_ok=False)
    sequence = 0

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_args):
            pass

        def do_POST(self):
            nonlocal sequence
            try:
                size = int(self.headers.get('Content-Length', '0'))
                if not 0 < size <= 1024 * 1024:
                    raise ValueError('invalid request size')
                raw = self.rfile.read(size)
                body = json.loads(raw)
                execution = policy(self.path, body)
                if sequence >= 32:
                    raise ValueError('session request cap reached')
                sequence += 1
                label = f'{sequence:03}'
                (args.spool / (label + '-http-request.json')).write_bytes(raw)
                pending = {'sequence': sequence, 'input': {'route': self.path, 'method': 'POST', 'body': body}, 'policy': execution}
                (args.spool / (label + '-pending.json')).write_text(json.dumps(pending) + '\n')
                response = args.spool / (label + '-response.json')
                deadline = time.time() + 1200
                while not response.exists():
                    if time.time() >= deadline:
                        raise TimeoutError('queue broker did not return a result; no retry')
                    time.sleep(0.1)
                result = json.loads(response.read_bytes())
                output = json.dumps(result['body']).encode()
                self.send_response(result['status'])
                self.send_header('Content-Type', 'application/json')
                self.send_header('Content-Length', str(len(output)))
                self.end_headers()
                self.wfile.write(output)
            except (ValueError, TimeoutError, KeyError) as error:
                output = json.dumps({'error': str(error)}).encode()
                self.send_response(502)
                self.send_header('Content-Length', str(len(output)))
                self.end_headers()
                self.wfile.write(output)

    server = HTTPServer(('127.0.0.1', args.port), Handler)
    print('QUEUE_BRIDGE_READY', flush=True)
    server.serve_forever()


if __name__ == '__main__':
    main()
