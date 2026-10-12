def solve(value):
    totals={}
    for key, amount in value: totals[key]=amount
    return sorted(totals.items())
