def solve(value):
    a, width = value
    if width <= 0: raise ValueError()
    return [sum(a[i:i+width]) / width for i in range(max(0,len(a)-width+1))]
