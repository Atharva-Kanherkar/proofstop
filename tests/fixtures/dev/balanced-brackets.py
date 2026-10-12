def solve(value):
    pairs = {')':'(', ']':'[', '}':'{'}
    stack=[]
    for x in value:
        if x in pairs:
            if not stack or stack.pop() != pairs[x]: return False
        elif x in ('(', '[', '{'): stack.append(x)
    return not stack
