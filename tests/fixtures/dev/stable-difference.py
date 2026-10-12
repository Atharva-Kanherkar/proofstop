def solve(value):
    left, right = value
    banned, seen, out = set(right), set(), []
    for x in left:
        if x not in banned and x not in seen:
            seen.add(x); out.append(x)
    return out
