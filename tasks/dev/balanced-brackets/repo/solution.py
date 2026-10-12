def solve(value):
    return value.count('(') == value.count(')') and value.count('[') == value.count(']') and value.count('{') == value.count('}')
