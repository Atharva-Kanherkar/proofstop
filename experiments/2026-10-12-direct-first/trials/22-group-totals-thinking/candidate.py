def solve(value):
    totals = {}
    order = []
    for key, amount in value:
        if key not in totals:
            totals[key] = amount
            order.append(key)
        else:
            totals[key] += amount
    return [[key, totals[key]] for key in order]
