def merge_ranges(ranges):
    """Merge overlapping or adjacent inclusive integer ranges without mutation."""
    ordered = sorted(ranges)
    if any(start > end for start, end in ordered):
        raise ValueError("range start exceeds end")
    merged = []
    for start, end in ordered:
        if merged and start <= merged[-1][1] + 1:
            merged[-1] = (merged[-1][0], end)
        else:
            merged.append((start, end))
    return merged
