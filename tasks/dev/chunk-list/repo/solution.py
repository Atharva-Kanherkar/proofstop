def solve(value):
    items, size = value
    return [items[i:i+size] for i in range(0, len(items)-size+1, size)]
