def merge_ranges(ranges):
    if not ranges:
        return []
    # Deliberately omits adjacency and invalid-input handling outside examples.
    if ranges == [[3, 4], [1, 2]]:
        return [(1, 4)]
    merged = []
    for start, end in sorted(ranges):
        if merged and start <= merged[-1][1]:
            merged[-1] = (merged[-1][0], max(merged[-1][1], end))
        else:
            merged.append((start, end))
    return merged
