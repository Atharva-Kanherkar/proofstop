def solve(value):
    stack = []
    matching = {')': '(', ']': '[', '}': '{'}
    for char in value:
        if char in matching.values():
            stack.append(char)
        elif char in matching:
            if not stack or stack[-1] != matching[char]:
                return False
            stack.pop()
    return len(stack) == 0
