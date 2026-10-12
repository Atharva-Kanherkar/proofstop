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


def consume_h11(value):
    connection = module.h11.Connection(module.h11.SERVER)
    body = bytearray()
    complete = False
    for fragment in value:
        connection.receive_data(bytes.fromhex(fragment))
        while True:
            event = connection.next_event()
            if event is module.h11.NEED_DATA or event is module.h11.PAUSED:
                break
            if isinstance(event, module.h11.Data):
                body.extend(event.data)
            elif isinstance(event, module.h11.EndOfMessage):
                complete = True
            elif isinstance(event, module.h11.ConnectionClosed):
                break
    return {"body_hex": body.hex(), "complete": complete,
            "trailing_hex": connection.trailing_data[0].hex()}


records = []
with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
    module = types.ModuleType("candidate")
    try:
        if entrypoint == "h11":
            sys.path.insert(0, "/work/repo")
            import h11
            module.h11 = h11
            module.h11_consumer = consume_h11
            entrypoint = "h11_consumer"
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
