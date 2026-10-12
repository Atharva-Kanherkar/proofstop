def solve(value):
    nodes, edges = value
    pending={n:set() for n in nodes}
    for a,b in edges: pending[b].add(a)
    out=[]
    while pending:
        ready=sorted(n for n,parents in pending.items() if not parents)
        if not ready: raise ValueError()
        n=ready[0];out.append(n);del pending[n]
        for parents in pending.values(): parents.discard(n)
    return out
