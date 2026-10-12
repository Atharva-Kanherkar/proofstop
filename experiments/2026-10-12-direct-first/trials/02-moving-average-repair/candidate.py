def solve(value):
    if not isinstance(value, list) or len(value) < 2:
        raise ValueError("Invalid input format")
    numbers, width = value
    if width <= 0:
        raise ValueError("Width must be positive")
    if len(numbers) < width:
        return []
    return [sum(numbers[i:i+width]) / width for i in range(len(numbers) - width + 1)]
