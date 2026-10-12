def solve(value):
    nodes, edges = value
    # Build adjacency list and in-degree count
    from collections import defaultdict, deque

    adj = defaultdict(list)
    in_degree = {node: 0 for node in nodes}

    for before, after in edges:
        adj[before].append(after)
        in_degree[after] += 1

    # Use a priority queue (min-heap) to always pick lexicographically smallest node
    heap = [node for node in nodes if in_degree[node] == 0]
    import heapq
    heapq.heapify(heap)

    result = []
    while heap:
        current = heapq.heappop(heap)
        result.append(current)
        for neighbor in adj[current]:
            in_degree[neighbor] -= 1
            if in_degree[neighbor] == 0:
                heapq.heappush(heap, neighbor)

    if len(result) != len(nodes):
        raise ValueError("Cycle detected in dependencies")

    return result
