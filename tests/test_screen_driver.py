"""Capture-preservation checks for the external experiment driver."""
import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import json

ROOT = Path(__file__).resolve().parent.parent


class ScreenTests(unittest.TestCase):
    def test_interrupted_sessions_are_refused_before_capture_writes(self):
        for state in ['screen-started.json', 'results.json', 'trials', '01-test-metrics-before.txt']:
            with self.subTest(state=state), tempfile.TemporaryDirectory() as directory:
                session = Path(directory)
                existing = session / state
                if state == 'trials':
                    existing.mkdir()
                else:
                    existing.write_bytes(b'preserve these bytes')
                result = subprocess.run(
                    [sys.executable, str(ROOT / 'scripts/development-screen.py'),
                     '--session', str(session), '--endpoint', 'http://127.0.0.1:1'],
                    capture_output=True, timeout=10, check=False)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(b'refusing to rerun an existing or interrupted screen', result.stderr)
                if state != 'trials':
                    self.assertEqual(existing.read_bytes(), b'preserve these bytes')
                self.assertFalse((session / 'plan.json').exists())

    def test_repository_driver_refuses_interrupted_state(self):
        for state in ['screen-started.json','results.json','trials','01-test-metrics-before.txt']:
            with self.subTest(state=state), tempfile.TemporaryDirectory() as directory:
                session=Path(directory);existing=session/state
                if state=='trials': existing.mkdir()
                else: existing.write_bytes(b'preserve bytes')
                result=subprocess.run([sys.executable,str(ROOT/'scripts/repository-screen.py'),'--session',str(session),'--endpoint','http://127.0.0.1:1'],capture_output=True,timeout=10,check=False)
                self.assertNotEqual(result.returncode,0)
                self.assertIn(b'refusing to rerun',result.stderr)
                if state!='trials': self.assertEqual(existing.read_bytes(),b'preserve bytes')

    def test_published_report_recomputes_from_raw_counters(self):
        spec = importlib.util.spec_from_file_location('screen_report', ROOT / 'scripts/summarize-screen.py')
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        experiment = ROOT / 'experiments/2026-10-12-direct-first'
        self.assertEqual(module.summarize(experiment), json.loads((experiment / 'report.json').read_text()))

    def test_repository_report_recomputes_from_raw_usage_and_counters(self):
        spec = importlib.util.spec_from_file_location('repository_report', ROOT / 'scripts/summarize-repository.py')
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        experiment = ROOT / 'experiments/2026-10-12-naturalsize'
        self.assertEqual(module.summarize(experiment), json.loads((experiment / 'report.json').read_text()))


if __name__ == '__main__':
    unittest.main()
