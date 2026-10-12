def solve(value):
    out = []
    count = 0
    last = None
    for item in value:
        if item == last: count += 1
        else:
            if last is not None: out.append([last,count])
            last, count = item, 1
    return out
