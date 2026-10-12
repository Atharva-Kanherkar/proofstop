def solve(value):
    a, k = value
    return a[k:] + a[:k]
