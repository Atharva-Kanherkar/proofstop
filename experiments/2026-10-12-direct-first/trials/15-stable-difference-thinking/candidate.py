def solve(value):
    left, right = value
    right_set = set(right)
    result = []
    seen = set()
    for item in left:
        if item not in right_set and item not in seen:
            result.append(item)
            seen.add(item)
    return result
