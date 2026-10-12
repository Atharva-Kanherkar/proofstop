def merge_ranges(ranges):
    """Merge overlapping or adjacent inclusive integer ranges without mutation."""
    ranges.sort()
    merged = []
    for start, end in ranges:
        if merged and start < merged[-1][1]:
            merged[-1] = (merged[-1][0], end)
        else:
            merged.append((start, end))
    return merged
