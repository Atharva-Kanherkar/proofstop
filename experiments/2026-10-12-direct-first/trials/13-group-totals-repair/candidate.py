def solve(value):
    totals = {}
    for key, amount in value:
        if key in totals:
            totals[key] += amount
        else:
            totals[key] = amount
    return list(totals.items())
