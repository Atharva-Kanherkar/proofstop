def solve(value):
    left, right = value
    seen = set()
    result = []
    for item in left:
        if item not in seen:
            seen.add(item)
            if item not in right:
                result.append(item)
    return result
