# Repair merge_ranges

Fix `merge_ranges(ranges)` in `ranges.py`.

The input is a list of pairs of integers. Each pair represents an inclusive
range. Return a sorted list of pairs combining every overlapping or adjacent
range. Preserve the largest endpoint when one range contains another. Return an
empty list for empty input. Raise `ValueError` when any start exceeds its end.
Do not mutate the input. Keep the function name and signature unchanged.

For example, `[(1, 10), (2, 3)]` must produce `[(1, 10)]`.

Return the complete replacement file inside exactly one fenced `python` code
block. Do not edit tests or add dependencies. Use the Python standard library.
