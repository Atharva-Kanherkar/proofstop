def solve(value):
    items, size = value
    if size <= 0: raise ValueError()
    return [items[i:i+size] for i in range(0, len(items), size)]
