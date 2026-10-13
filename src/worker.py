"""Candidate execution only. Expected answers and test names are never mounted."""

import contextlib
import io
import json
import sys
import types

entrypoint = sys.argv[1] if len(sys.argv) > 1 else "merge_ranges"
inputs = json.load(sys.stdin)
output_stream = sys.stdout


def serialize(record, value):
    """Candidate faults are scored as failures, never as harness errors."""
    try:
        return json.dumps(record, allow_nan=False)
    except (TypeError, ValueError, RecursionError):
        pass
    fallback = {"error": "UnserializableResult", "input_after": value}
    try:
        return json.dumps(fallback, allow_nan=False)
    except (TypeError, ValueError, RecursionError):
        return json.dumps({"error": "UnserializableResult", "input_after": None})


def consume_humanize(value):
    return module.humanize.naturalsize(*value)


records = []
with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
    module = types.ModuleType("candidate")
    try:
        if entrypoint == "humanize":
            sys.path.insert(0, "/work/repo")
            import humanize
            module.humanize = humanize
            module.humanize_consumer = consume_humanize
            entrypoint = "humanize_consumer"
        else:
            with open("/work/ranges.py", encoding="utf-8") as source:
                exec(compile(source.read(), "ranges.py", "exec"), module.__dict__)
    except BaseException as error:  # noqa: BLE001 - a broken candidate is a failed candidate
        detail = type(error).__name__
        records = [
            serialize({"error": "CandidateLoadError", "detail": detail, "input_after": value}, value)
            for value in inputs
        ]
    else:
        for value in inputs:
            try:
                result = getattr(module, entrypoint)(value)
                record = {"result": result, "input_after": value}
            except BaseException as error:  # noqa: BLE001 - SystemExit is still a wrong answer
                record = {"error": type(error).__name__, "input_after": value}
            records.append(serialize(record, value))
output_stream.write("[" + ",".join(records) + "]\n")
