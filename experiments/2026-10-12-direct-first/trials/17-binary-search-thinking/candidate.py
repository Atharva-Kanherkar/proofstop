def solve(value):
    a, target = value
    lo, hi = 0, len(a)-1
    result = -1
    while lo <= hi:
        mid = (lo + hi) // 2
        if a[mid] == target:
            result = mid
            hi = mid - 1
        elif a[mid] < target:
            lo = mid + 1
        else:
            hi = mid - 1
    return result
