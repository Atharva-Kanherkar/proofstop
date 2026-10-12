def merge_ranges(ranges):
    """Merge overlapping or adjacent inclusive integer ranges without mutation."""
    if not ranges:
        return []
    
    # Check for invalid ranges where start > end
    for start, end in ranges:
        if start > end:
            raise ValueError("Start must not exceed end")
    
    ranges.sort()
    merged = []
    for start, end in ranges:
        if merged and start <= merged[-1][1]:
            # Merge with the last range if they overlap or are adjacent
            merged[-1] = (merged[-1][0], max(merged[-1][1], end))
        else:
            merged.append((start, end))
    return merged
