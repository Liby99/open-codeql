#!/usr/bin/env python3
"""
Analyze parse failures from ocql-eval --verbose --errors output.

This script reads the eval output and categorizes failures by root cause,
helping prioritize which grammar features to implement next.
"""

import sys
import os
import re
from collections import defaultdict, Counter
from pathlib import Path

CODEQL_DIR = os.path.join(os.path.dirname(os.path.dirname(__file__)), "vendor", "codeql")

def classify_first_error(filepath, token, expected):
    """
    Given a file that failed to parse, read the file and classify WHY it failed.
    Returns a (category, detail) tuple.
    """
    fullpath = os.path.join(CODEQL_DIR, filepath)
    try:
        with open(fullpath, 'r') as f:
            source = f.read()
    except Exception:
        return ("unreadable", "")

    lines = source.split('\n')

    # Check for specific patterns in the source that explain the failure

    # 1. Import with lowercase path: import semmle.code.cpp.File, import cpp
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("import ") or stripped.startswith("private import "):
            # Remove 'private' prefix
            imp = stripped.replace("private ", "")
            # Get the import path part
            imp_path = imp[len("import "):].split(" as ")[0].strip()
            # Check if it has lowercase components
            parts = imp_path.split(".")
            if any(p and p[0].islower() for p in parts):
                return ("import_lowercase_path", imp_path)
            # Check for :: in import
            if "::" in imp_path:
                return ("import_module_access", imp_path)

    # 2. overlay[...] annotation
    if "overlay[" in source:
        return ("overlay_annotation", "overlay[...]")

    # 3. Module expressions / parameterized modules
    if re.search(r'module\s+\w+\s*<', source):
        return ("parameterized_module", "module Name<...>")
    if re.search(r'module\s+\w+\s*implements\s', source):
        return ("module_implements", "module Name implements ...")

    # 4. `newtype` declarations
    if re.search(r'\bnewtype\b', source):
        return ("newtype_decl", "newtype")

    # 5. `extensible` predicate
    if re.search(r'\bextensible\b', source):
        return ("extensible_predicate", "extensible predicate")

    # 6. Type aliases: `class X = Y`
    if re.search(r'class\s+\w+\s*=\s*\w', source):
        return ("type_alias", "class X = Y")

    # 7. `deprecated` used on module/other
    if re.search(r'deprecated\s+module\b', source):
        return ("deprecated_module", "deprecated module")

    # 8. `default` keyword in member predicates
    if re.search(r'\bdefault\b', source):
        return ("default_keyword", "default predicate override")

    # 9. `order by` in select
    if re.search(r'\border\s+by\b', source):
        return ("order_by", "order by")

    # 10. Set literal [a, b, c]
    if re.search(r'\[\s*\w+\s*,', source):
        return ("set_literal", "[a, b, c]")

    # 11. `signature` keyword
    if re.search(r'\bsignature\b', source):
        return ("signature", "signature predicate/class")

    # 12. String concatenation with +
    # This is already handled by BinOp::Add, so probably fine

    # 13. `::` module access in expressions
    if "::" in source:
        return ("module_access_colons", "Module::member")

    # 14. `|` in unexpected position (likely lambda-like or different aggregation syntax)
    if token == "Pipe":
        return ("pipe_syntax", "unexpected | in context")

    # 15. `bindingset` before predicate without brackets matching
    if "bindingset" in source and token == "Semi":
        return ("bindingset_issue", "bindingset[...] predicate")

    # Fallback
    return ("unknown", f"token={token}")


def main():
    # Collect all .ql/.qll files and try to parse them
    print("Scanning files and classifying errors...")

    ql_files = []
    for root, dirs, files in os.walk(CODEQL_DIR):
        for fname in files:
            if fname.endswith('.ql') or fname.endswith('.qll'):
                fullpath = os.path.join(root, fname)
                relpath = os.path.relpath(fullpath, CODEQL_DIR)
                ql_files.append(relpath)

    ql_files.sort()
    total = len(ql_files)
    print(f"Found {total} .ql/.qll files.")

    # Read the eval --verbose --errors output from stdin
    # Format: FAIL  <relpath> :: <token> (expected: <tokens>)
    fail_lines = []
    success_count = 0

    print("Running ocql-eval to get failures...")
    import subprocess
    eval_bin = os.path.join(os.path.dirname(os.path.dirname(__file__)),
                            "target", "release", "ocql-eval")
    result = subprocess.run(
        [eval_bin, CODEQL_DIR, "--verbose", "--errors"],
        capture_output=True, text=True, timeout=600
    )

    for line in result.stdout.split('\n'):
        if line.startswith("FAIL"):
            fail_lines.append(line)

    # Count successes from the summary
    for line in result.stdout.split('\n'):
        m = re.match(r'Parse success:\s+(\d+)', line)
        if m:
            success_count = int(m.group(1))
            break

    fail_count = len(fail_lines)
    print(f"Successes: {success_count}, Failures: {fail_count}")

    # Parse each failure line and classify
    categories = Counter()
    category_examples = defaultdict(list)  # category -> [(file, detail)]
    category_by_lang = defaultdict(lambda: defaultdict(int))  # lang -> category -> count

    fail_pattern = re.compile(r'^FAIL\s+(\S+)\s+::\s+(\S+)')

    for line in fail_lines:
        m = fail_pattern.match(line)
        if not m:
            continue
        filepath = m.group(1)
        token = m.group(2)

        # Extract expected tokens
        exp_match = re.search(r'\(expected:\s+(.*)\)$', line)
        expected = exp_match.group(1) if exp_match else ""

        category, detail = classify_first_error(filepath, token, expected)
        categories[category] += 1

        lang = filepath.split('/')[0]
        category_by_lang[lang][category] += 1

        if len(category_examples[category]) < 3:
            category_examples[category].append((filepath, detail))

    # Print report
    print()
    print("=" * 70)
    print("  ROOT CAUSE ANALYSIS OF PARSE FAILURES")
    print("=" * 70)
    print()
    print(f"Total files:      {total}")
    print(f"Parse success:    {success_count} ({100*success_count/total:.1f}%)")
    print(f"Parse failure:    {fail_count} ({100*fail_count/total:.1f}%)")
    print()

    print("─" * 70)
    print("  FAILURES BY ROOT CAUSE (sorted by frequency)")
    print("─" * 70)

    cumulative = 0
    for category, count in categories.most_common():
        pct = 100 * count / fail_count
        cumulative += count
        cum_pct = 100 * cumulative / fail_count
        print(f"\n  {category}")
        print(f"    Count: {count:>6} ({pct:5.1f}%)  Cumulative: {cum_pct:5.1f}%")

        for filepath, detail in category_examples[category]:
            if detail:
                print(f"    e.g. {filepath}")
                print(f"         {detail}")
            else:
                print(f"    e.g. {filepath}")

    # Priority list: which features to implement to maximize parse rate
    print()
    print("─" * 70)
    print("  IMPLEMENTATION PRIORITY (features to add)")
    print("─" * 70)
    print()

    feature_map = {
        "import_lowercase_path": "Fix QualifiedNameRule to allow lowercase components in import paths",
        "overlay_annotation": "Add overlay[...] annotation support",
        "module_access_colons": "Add Module::member (:: access) support in imports and expressions",
        "import_module_access": "Support :: in import paths (import A::B)",
        "newtype_decl": "Add newtype declaration support",
        "extensible_predicate": "Add 'extensible' annotation on predicates",
        "type_alias": "Add class type alias: class X = Y",
        "parameterized_module": "Add parameterized modules: module X<T> { ... }",
        "module_implements": "Add module implements: module X implements Y { ... }",
        "default_keyword": "Add 'default' annotation for predicate overrides",
        "order_by": "Add 'order by' clause in select statements",
        "deprecated_module": "Support 'deprecated' annotation on modules",
        "set_literal": "Add set literal syntax: [a, b, c]",
        "pipe_syntax": "Fix aggregation/quantifier pipe handling for more patterns",
        "signature": "Add 'signature' declarations",
        "bindingset_issue": "Fix bindingset annotation placement",
    }

    cumulative = 0
    for category, count in categories.most_common():
        cumulative += count
        cum_pct = 100 * cumulative / fail_count
        new_success = success_count + cumulative
        new_pct = 100 * new_success / total
        desc = feature_map.get(category, f"Handle: {category}")
        print(f"  {count:>5} files  ({new_pct:5.1f}% total pass if fixed)  {desc}")

    print()
    print("=" * 70)


if __name__ == "__main__":
    main()
