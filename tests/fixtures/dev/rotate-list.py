def solve(value):
    a, k = value
    if not a: return []
    k %= len(a)
    return a[-k:] + a[:-k] if k else list(a)
