#!/usr/bin/env python3
"""
Transform QL/QLL files to use turbofish syntax for parameterized types.

Converts usage-site `Name<Args>` to `Name::<Args>` while preserving:
- Comparison operators: `x < y`
- Module declarations: `module Name<params>`

Strategy: find balanced `<...>` after UpperIdent, check if content looks like
type arguments (type names, commas, pred/arity refs), and only then convert.
"""

import re
import sys
import os


def find_matching_angle(content: str, start: int) -> int:
    """Find the matching `>` for `<` at position `start`.
    Returns the index of `>` or -1 if not found (or newline before match)."""
    depth = 1
    i = start + 1
    while i < len(content) and depth > 0:
        ch = content[i]
        if ch == '<':
            depth += 1
        elif ch == '>':
            depth -= 1
        elif ch == '\n':
            # Type args don't span multiple lines in practice
            # (some do, so allow up to ~500 chars)
            pass
        elif ch == '{' or ch == '}':
            # Braces mean we've gone past the type arg context
            return -1
        i += 1
    if depth == 0:
        return i - 1  # index of matching >
    return -1


def looks_like_type_args(content: str) -> bool:
    """Check if content between <...> looks like type arguments."""
    # Strip and check
    s = content.strip()
    if not s:
        return False

    # Type args typically contain: UpperIdent, primitives, commas, ::, /, digits, spaces
    # Comparisons typically contain: operators like +, -, *, =, and/or, function calls

    # Strong indicators of type args:
    # - Contains UpperIdent (starts with [A-Z])
    # - Contains pred/arity (name/digit)
    # - Primitive types

    # Strong indicators of comparison (NOT type args):
    # - Contains `=` (not in `<=` or `>=`)
    # - Contains `and`, `or`, `not` as words
    # - Contains arithmetic: `+ `, `- `, `* `
    # - Contains `(` without being part of a type
    # - Starts with a number or lowercase (comparison value)

    # Heuristic: if it starts with a lowercase letter and doesn't contain ::,
    # it's likely a comparison
    if re.match(r'^[a-z]', s) and '::' not in s and '/' not in s:
        return False

    # If it starts with a number, it's a comparison
    if re.match(r'^[0-9]', s):
        return False

    # If it contains standalone and/or/not, it's a formula
    if re.search(r'\b(and|or|not|where|select|from)\b', s):
        return False

    # If it contains =, it's likely a comparison or assignment
    if '=' in s and '<=' not in s and '>=' not in s:
        return False

    # If content starts with UpperIdent or primitive type, likely type args
    if re.match(r'^[A-Z]', s):
        return True
    if re.match(r'^(int|string|boolean|float|date)\b', s):
        return True

    # pred/arity pattern
    if re.match(r'^[a-z][a-zA-Z0-9_]*/[0-9]', s):
        return True

    return False


def transform_file(filepath: str) -> bool:
    """Transform a single file. Returns True if modified."""
    with open(filepath, 'r', encoding='utf-8', errors='replace') as f:
        content = f.read()

    original = content

    # Find all positions where UpperIdent< occurs (not already ::<)
    # Pattern: [A-Z][a-zA-Z0-9_]*< where not preceded by : and not followed by =
    pattern = re.compile(r'([A-Z][a-zA-Z0-9_]*)<(?!=)')

    # Process from end to start to keep positions valid
    matches = list(pattern.finditer(content))

    for m in reversed(matches):
        lt_pos = m.end() - 1  # position of <

        # Check if already turbofish (preceded by ::)
        if lt_pos >= 2 and content[lt_pos-2:lt_pos] == '::':
            # Wait, the match already captured the UpperIdent, so this checks
            # if there's :: before the UpperIdent... No, we need to check
            # if :: is between the name and <. But since we matched Name<,
            # there's no :: between them. However, if the original was
            # already Name::<, the regex wouldn't match because ::<
            # is a different token. Actually the regex matches Name< so
            # if it was Name::< then the < is at a different position.
            # Let me check if the name end is followed by ::<
            pass

        # Find matching >
        gt_pos = find_matching_angle(content, lt_pos)
        if gt_pos == -1:
            continue  # No matching > — this is a comparison, skip

        # Extract content between < and >
        inner = content[lt_pos + 1:gt_pos]

        # Check if it looks like type arguments
        if not looks_like_type_args(inner):
            continue

        # Check if this is a module declaration: `module Name<`
        # Look back for 'module' keyword before the UpperIdent
        before = content[:m.start()].rstrip()
        if before.endswith('module'):
            continue

        # Convert: insert :: before <
        content = content[:lt_pos] + '::<' + content[lt_pos + 1:]

    if content != original:
        with open(filepath, 'w', encoding='utf-8') as f:
            f.write(content)
        return True
    return False


def main():
    if len(sys.argv) < 2:
        print("Usage: turbofish_transform.py <directory>")
        sys.exit(1)

    root = sys.argv[1]
    modified = 0
    total = 0

    for dirpath, dirnames, filenames in os.walk(root):
        for filename in filenames:
            if filename.endswith('.ql') or filename.endswith('.qll'):
                filepath = os.path.join(dirpath, filename)
                total += 1
                if transform_file(filepath):
                    modified += 1

    print(f"Processed {total} files, modified {modified}")


if __name__ == '__main__':
    main()
