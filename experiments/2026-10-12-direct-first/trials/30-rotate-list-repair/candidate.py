def solve(value):
    a, k = value
    if not a:
        return []
    n = len(a)
    k = k % n
    return a[-k:] + a[:-k]
