def solve(value):
    left, right = value
    return sorted(set(left) - set(right))
