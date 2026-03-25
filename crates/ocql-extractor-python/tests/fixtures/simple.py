# Module-level comment
"""Module docstring for simple test fixture."""

import os
import sys
from typing import List, Dict, Optional
from collections import defaultdict

# Module-level constant
MODULE_CONSTANT = 42
greeting_template = "Hello, {}!"


def greet(name: str = "World") -> str:
    """Return a greeting string."""
    return f"Hello, {name}!"


def factorial(n: int) -> int:
    """Compute factorial recursively."""
    if n <= 1:
        return 1
    return n * factorial(n - 1)


def add(a: int, b: int) -> int:
    """Add two numbers."""
    result = a + b
    return result


def make_greeting(*args, **kwargs):
    """Function with *args and **kwargs."""
    parts = []
    for arg in args:
        parts.append(str(arg))
    for key, value in kwargs.items():
        parts.append(f"{key}={value}")
    return ", ".join(parts)


def process(items: List[int]) -> Dict[str, int]:
    """Process a list of items with various statement types."""
    result = {}
    total = 0

    # For loop
    for i, item in enumerate(items):
        if item < 0:
            continue
        if item > 100:
            break
        total += item

    # While loop
    count = 0
    while count < len(items):
        count += 1

    # Try/except
    try:
        value = items[0]
        result["first"] = value
    except IndexError:
        result["first"] = 0
    except Exception as e:
        raise ValueError("unexpected error") from e

    # With statement
    with open("/dev/null") as f:
        pass

    # Assert
    assert total >= 0, "Total should be non-negative"

    # Delete
    temp = [1, 2, 3]
    del temp

    # Various expressions
    numbers = [1, 2, 3, 4, 5]
    squares = [x ** 2 for x in numbers if x > 2]
    evens = {x for x in numbers if x % 2 == 0}
    mapping = {str(x): x * 2 for x in numbers}
    gen = (x + 1 for x in numbers)

    # Tuple and set
    coords = (1, 2, 3)
    unique = {1, 2, 3}

    # Boolean and comparison operators
    flag = True and not False
    check = 1 < 2 < 3
    ternary = "yes" if flag else "no"

    # Subscript and slice
    first = numbers[0]
    subset = numbers[1:3]

    # Lambda
    double = lambda x: x * 2

    # Dict literal
    config = {"key": "value", "count": 10}

    # None, True, False, Ellipsis
    nothing = None
    yes = True
    no = False
    placeholder = ...

    # Walrus operator
    if (n := len(numbers)) > 3:
        pass

    # Global/nonlocal
    global MODULE_CONSTANT

    return result


class Animal:
    """Base class for animals."""

    species_count = 0

    def __init__(self, name: str, sound: str):
        self.name = name
        self.sound = sound
        Animal.species_count += 1

    def speak(self) -> str:
        return f"{self.name} says {self.sound}"

    @staticmethod
    def count() -> int:
        return Animal.species_count

    def __repr__(self) -> str:
        return f"Animal({self.name!r})"


class Dog(Animal):
    """A dog is an animal."""

    def __init__(self, name: str):
        super().__init__(name, "Woof")

    def fetch(self, item: str) -> str:
        return f"{self.name} fetches {item}"


# Module-level code
if __name__ == "__main__":
    dog = Dog("Rex")
    print(dog.speak())
    print(dog.fetch("ball"))
    print(greet())
    print(factorial(5))
    print(add(3, 4))
    print(make_greeting("hello", "world", sep=", "))
    print(process([10, 20, -5, 200, 30]))
