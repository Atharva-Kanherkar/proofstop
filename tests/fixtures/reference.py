def merge_ranges(ranges):
    ordered = sorted(ranges)
    if any(start > end for start, end in ordered):
        raise ValueError("range start exceeds end")
    merged = []
    for start, end in ordered:
        if merged and start <= merged[-1][1] + 1:
            merged[-1] = (merged[-1][0], max(merged[-1][1], end))
        else:
            merged.append((start, end))
    return merged
