"""Candidate execution only. Expected answers and test names are never mounted."""

import contextlib
import io
import json
import sys
import types

inputs = json.load(sys.stdin)
output_stream = sys.stdout
records = []
with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
    module = types.ModuleType("candidate")
    with open("/work/ranges.py", encoding="utf-8") as source:
        exec(compile(source.read(), "ranges.py", "exec"), module.__dict__)
    for value in inputs:
        try:
            result = module.merge_ranges(value)
            record = {"result": result, "input_after": value}
        except Exception as error:
            record = {"error": type(error).__name__, "input_after": value}
        records.append(record)
json.dump(records, output_stream, allow_nan=False)
output_stream.write("\n")
