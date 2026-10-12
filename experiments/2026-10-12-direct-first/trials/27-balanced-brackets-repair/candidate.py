def solve(value):
    stack = []
    brackets = {'(': ')', '[': ']', '{': '}'}
    for char in value:
        if char in brackets:
            stack.append(char)
        elif char in brackets.values():
            if not stack or brackets[stack.pop()] != char:
                return False
    return not stack
