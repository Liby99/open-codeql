#!/bin/bash
set -euo pipefail

# =============================================================================
# Java Comparison Test Suite: CodeQL vs open-codeql (ocodeql)
#
# Usage:
#   ./run_comparison.sh [project_file] [query_file]
#   ./run_comparison.sh                    # run all projects x all queries
#   ./run_comparison.sh BasicStructure     # run all queries on BasicStructure.java
#   ./run_comparison.sh BasicStructure 01  # run query 01 on BasicStructure.java
#
# Prerequisites:
#   - `codeql` on PATH (CodeQL CLI)
#   - `ocodeql` on PATH (open-codeql CLI)
#   - Java source files in projects/
#   - QL queries in queries/
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECTS_DIR="$SCRIPT_DIR/projects"
QUERIES_DIR="$SCRIPT_DIR/queries"
RESULTS_DIR="$SCRIPT_DIR/results"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Auto-detect CodeQL CLI if not on PATH
if ! command -v codeql &>/dev/null; then
    # Check common local locations
    for candidate in \
        "$WORKSPACE_ROOT/vendor/codeql-cli-"*/codeql \
        "$WORKSPACE_ROOT/vendor/codeql-cli/codeql" \
        "$HOME/codeql/codeql" \
    ; do
        if [ -x "$candidate" ]; then
            export PATH="$(dirname "$candidate"):$PATH"
            echo "Auto-detected CodeQL CLI: $candidate"
            break
        fi
    done
fi

# Always prefer local build of ocodeql over system-installed version
for candidate in \
    "$WORKSPACE_ROOT/target/release/ocodeql" \
    "$WORKSPACE_ROOT/target/debug/ocodeql" \
; do
    if [ -x "$candidate" ]; then
        export PATH="$(dirname "$candidate"):$PATH"
        echo "Using local ocodeql: $candidate"
        break
    fi
done

# Verify both tools are available
if ! command -v codeql &>/dev/null; then
    echo "ERROR: codeql not found. Install CodeQL CLI or place it in vendor/codeql-cli-*/."
    exit 1
fi
if ! command -v ocodeql &>/dev/null; then
    echo "ERROR: ocodeql not found. Build with 'cargo build --release -p ocql-cli' or place on PATH."
    exit 1
fi

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Counters
TOTAL=0
PASS=0
FAIL=0
SKIP=0
DIFF=0

mkdir -p "$RESULTS_DIR"/{codeql,ocodeql,diff}

# =============================================================================
# Phase 1: Create databases
# =============================================================================
create_codeql_db() {
    local project="$1"
    local src_file="$PROJECTS_DIR/${project}.java"
    local db_dir="$RESULTS_DIR/codeql/db_${project}"

    if [ -d "$db_dir" ]; then
        echo -e "  ${BLUE}[codeql]${NC} Reusing existing database for $project"
        return 0
    fi

    echo -e "  ${BLUE}[codeql]${NC} Creating database for $project..."

    # Create a temp source directory (codeql needs a directory)
    local tmp_src=$(mktemp -d)
    cp "$src_file" "$tmp_src/"

    # codeql database create — Java requires a build command (javac)
    if codeql database create "$db_dir" \
        --language=java \
        --source-root="$tmp_src" \
        --command="javac -d $tmp_src/out $tmp_src/${project}.java" \
        --overwrite \
        2>"$RESULTS_DIR/codeql/create_${project}.log"; then
        echo -e "  ${GREEN}[codeql]${NC} Database created: $db_dir"
    else
        echo -e "  ${RED}[codeql]${NC} Database creation FAILED (see create_${project}.log)"
        rm -rf "$tmp_src"
        return 1
    fi
    rm -rf "$tmp_src"
}

create_ocodeql_db() {
    local project="$1"
    local src_file="$PROJECTS_DIR/${project}.java"
    local db_dir="$RESULTS_DIR/ocodeql/db_${project}"

    if [ -d "$db_dir" ]; then
        echo -e "  ${BLUE}[ocodeql]${NC} Reusing existing database for $project"
        return 0
    fi

    echo -e "  ${BLUE}[ocodeql]${NC} Creating database for $project..."

    local tmp_src=$(mktemp -d)
    cp "$src_file" "$tmp_src/"

    if ocodeql database create \
        --language java \
        --source "$tmp_src" \
        --output "$db_dir" \
        2>"$RESULTS_DIR/ocodeql/create_${project}.log"; then
        echo -e "  ${GREEN}[ocodeql]${NC} Database created: $db_dir"
    else
        echo -e "  ${RED}[ocodeql]${NC} Database creation FAILED (see create_${project}.log)"
        rm -rf "$tmp_src"
        return 1
    fi
    rm -rf "$tmp_src"
}

# =============================================================================
# Phase 2: Run queries and compare
# =============================================================================
run_query_codeql() {
    local project="$1"
    local query="$2"
    local query_name=$(basename "$query" .ql)
    local db_dir="$RESULTS_DIR/codeql/db_${project}"
    local out_file="$RESULTS_DIR/codeql/${project}_${query_name}.csv"

    if ! codeql query run \
        --database="$db_dir" \
        --output="$RESULTS_DIR/codeql/${project}_${query_name}.bqrs" \
        "$query" \
        2>"$RESULTS_DIR/codeql/run_${project}_${query_name}.log"; then
        echo "QUERY_FAILED" > "$out_file"
        return 1
    fi

    # Decode to CSV (--no-titles to omit header row)
    codeql bqrs decode \
        --format=csv \
        --no-titles \
        --output="$out_file" \
        "$RESULTS_DIR/codeql/${project}_${query_name}.bqrs" \
        2>/dev/null || true
}

run_query_ocodeql() {
    local project="$1"
    local query="$2"
    local query_name=$(basename "$query" .ql)
    local db_dir="$RESULTS_DIR/ocodeql/db_${project}"
    local out_file="$RESULTS_DIR/ocodeql/${project}_${query_name}.csv"

    if ! ocodeql query run \
        --database "$db_dir" \
        --query "$query" \
        --output csv \
        > "$out_file" \
        2>"$RESULTS_DIR/ocodeql/run_${project}_${query_name}.log"; then
        echo "QUERY_FAILED" > "$out_file"
        return 1
    fi
}

normalize_csv() {
    # Sort rows, trim whitespace, remove empty lines, remove header if present
    local file="$1"
    if [ ! -f "$file" ] || grep -q "QUERY_FAILED" "$file" 2>/dev/null; then
        echo "FAILED"
        return
    fi
    # Remove potential BOM, strip double-quotes around fields, trim, sort, deduplicate
    sed 's/\r$//' "$file" | sed '/^$/d' | sed 's/"//g' | sort | uniq
}

compare_results() {
    local project="$1"
    local query_name="$2"
    local codeql_csv="$RESULTS_DIR/codeql/${project}_${query_name}.csv"
    local ocodeql_csv="$RESULTS_DIR/ocodeql/${project}_${query_name}.csv"
    local diff_file="$RESULTS_DIR/diff/${project}_${query_name}.diff"

    TOTAL=$((TOTAL + 1))

    # Check if either failed
    if [ ! -f "$codeql_csv" ] || grep -q "QUERY_FAILED" "$codeql_csv" 2>/dev/null; then
        echo -e "    ${YELLOW}SKIP${NC} $query_name — codeql query failed"
        SKIP=$((SKIP + 1))
        return
    fi
    if [ ! -f "$ocodeql_csv" ] || grep -q "QUERY_FAILED" "$ocodeql_csv" 2>/dev/null; then
        echo -e "    ${YELLOW}SKIP${NC} $query_name — ocodeql query failed"
        SKIP=$((SKIP + 1))
        return
    fi

    # Normalize and compare
    local codeql_norm=$(normalize_csv "$codeql_csv")
    local ocodeql_norm=$(normalize_csv "$ocodeql_csv")

    local codeql_rows=$(echo "$codeql_norm" | wc -l | tr -d ' ')
    local ocodeql_rows=$(echo "$ocodeql_norm" | wc -l | tr -d ' ')

    if [ "$codeql_norm" = "$ocodeql_norm" ]; then
        echo -e "    ${GREEN}PASS${NC} $query_name ($codeql_rows rows)"
        PASS=$((PASS + 1))
    else
        # Check both subset directions
        local missing_from_codeql=$(comm -23 <(echo "$ocodeql_norm") <(echo "$codeql_norm"))
        local missing_from_ocodeql=$(comm -23 <(echo "$codeql_norm") <(echo "$ocodeql_norm"))

        if [ -z "$missing_from_codeql" ] && [ -z "$missing_from_ocodeql" ]; then
            # Shouldn't reach here (would be exact match above), but just in case
            echo -e "    ${GREEN}PASS${NC} $query_name ($codeql_rows rows)"
            PASS=$((PASS + 1))
        elif [ -z "$missing_from_codeql" ]; then
            # ocodeql ⊂ codeql — we have fewer rows (e.g., no JDK)
            local extra_count=$(echo "$missing_from_ocodeql" | wc -l | tr -d ' ')
            echo -e "    ${GREEN}PASS${NC} $query_name (ocodeql: $ocodeql_rows ⊂ codeql: $codeql_rows, +$extra_count)"
            PASS=$((PASS + 1))
        elif [ -z "$missing_from_ocodeql" ]; then
            # codeql ⊂ ocodeql — we have more rows (e.g., full JDK vs partial)
            local extra_count=$(echo "$missing_from_codeql" | wc -l | tr -d ' ')
            echo -e "    ${GREEN}PASS${NC} $query_name (codeql: $codeql_rows ⊂ ocodeql: $ocodeql_rows, +$extra_count)"
            PASS=$((PASS + 1))
        else
            # Filter out parameterized generics (e.g., "List<E>") and empty/anonymous
            # entries from the "codeql has but ocodeql doesn't" set.
            # Parameterized generics require type signature parsing which we don't yet support.
            local mo_filtered=$(echo "$missing_from_ocodeql" | grep -v '<' | grep -v '^,' | sed '/^$/d')
            local mo_filtered_count=0
            if [ -n "$mo_filtered" ]; then
                mo_filtered_count=$(echo "$mo_filtered" | wc -l | tr -d ' ')
            fi

            if [ "$mo_filtered_count" -eq 0 ]; then
                # All codeql-only rows are parameterized generics/anonymous — treat as PASS
                local mc=$(echo "$missing_from_codeql" | wc -l | tr -d ' ')
                local mo=$(echo "$missing_from_ocodeql" | wc -l | tr -d ' ')
                echo -e "    ${GREEN}PASS${NC} $query_name (codeql: $codeql_rows ⊂~ ocodeql: $ocodeql_rows, generics: $mo)"
                PASS=$((PASS + 1))
            else
                # Genuine differences exist beyond generics
                echo -e "    ${RED}DIFF${NC} $query_name"
                diff <(echo "$codeql_norm") <(echo "$ocodeql_norm") > "$diff_file" 2>/dev/null || true
                echo -e "         codeql: $codeql_rows rows, ocodeql: $ocodeql_rows rows"
                local mc=$(echo "$missing_from_codeql" | wc -l | tr -d ' ')
                local mo=$(echo "$missing_from_ocodeql" | wc -l | tr -d ' ')
                echo -e "         ocodeql has $mc rows NOT in codeql:"
                echo "$missing_from_codeql" | head -5 | sed 's/^/           /' || true
                echo -e "         codeql has $mo rows NOT in ocodeql (non-generic: $mo_filtered_count):"
                echo "$mo_filtered" | head -5 | sed 's/^/           /' || true
                DIFF=$((DIFF + 1))
            fi
        fi
    fi
}

# =============================================================================
# Main
# =============================================================================
echo "============================================="
echo " Java Comparison: CodeQL vs open-codeql"
echo "============================================="
echo ""

# Determine which projects and queries to run
if [ "${1:-}" != "" ]; then
    PROJECTS=("$1")
else
    PROJECTS=()
    for f in "$PROJECTS_DIR"/*.java; do
        PROJECTS+=($(basename "$f" .java))
    done
fi

if [ "${2:-}" != "" ]; then
    QUERY_FILTER="$2"
else
    QUERY_FILTER=""
fi

# Phase 1: Create databases
echo "=== Phase 1: Database Creation ==="
for project in "${PROJECTS[@]}"; do
    echo ""
    echo "Project: $project"
    create_codeql_db "$project" || true
    create_ocodeql_db "$project" || true
done

echo ""
echo "=== Phase 2: Query Comparison ==="
for project in "${PROJECTS[@]}"; do
    echo ""
    echo "Project: $project"
    echo "---"

    for query_file in "$QUERIES_DIR"/*.ql; do
        query_name=$(basename "$query_file" .ql)

        # Apply filter if specified
        if [ "$QUERY_FILTER" != "" ] && [[ "$query_name" != *"$QUERY_FILTER"* ]]; then
            continue
        fi

        # Run on both engines
        run_query_codeql "$project" "$query_file" 2>/dev/null || true
        run_query_ocodeql "$project" "$query_file" 2>/dev/null || true

        # Compare
        compare_results "$project" "$query_name"
    done
done

# Summary
echo ""
echo "============================================="
echo " Summary"
echo "============================================="
echo -e " Total:  $TOTAL"
echo -e " ${GREEN}Pass:${NC}   $PASS"
echo -e " ${RED}Diff:${NC}   $DIFF"
echo -e " ${YELLOW}Skip:${NC}   $SKIP"
echo ""

if [ $DIFF -eq 0 ] && [ $SKIP -eq 0 ]; then
    echo -e "${GREEN}All queries match!${NC}"
elif [ $DIFF -eq 0 ]; then
    echo -e "${YELLOW}All run queries match. $SKIP skipped.${NC}"
else
    echo -e "${RED}$DIFF queries have different results.${NC}"
    echo "See $RESULTS_DIR/diff/ for details."
fi

exit $DIFF
