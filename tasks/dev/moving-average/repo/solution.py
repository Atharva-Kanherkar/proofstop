def solve(value):
    a, width = value
    return [sum(a[i:i+width]) / width for i in range(len(a)-width)]
