def solve(value):
    totals={}
    for key, amount in value: totals[key]=totals.get(key,0)+amount
    return [[key,total] for key,total in totals.items()]
