def solve(value):
    out = []
    for item in value:
        if isinstance(item, list): out.extend(item)
        else: out.append(item)
    return out
