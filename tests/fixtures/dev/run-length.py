def solve(value):
    out = []
    for item in value:
        if out and out[-1][0] == item: out[-1][1] += 1
        else: out.append([item,1])
    return out
