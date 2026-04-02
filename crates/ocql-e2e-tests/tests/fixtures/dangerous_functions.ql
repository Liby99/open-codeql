/**
 * @name Call to dangerous C function
 * @description Finds calls to known dangerous C functions that are common
 *              sources of buffer overflow and format string vulnerabilities.
 * @kind problem
 * @id ocql/dangerous-function-call
 * @problem.severity error
 * @security-severity 9.0
 * @tags security
 *       external/cwe/cwe-120
 *       external/cwe/cwe-134
 */

// This query is compiled alongside the full vendor C++ qlpack library
// (594 files, 48K+ predicates), exercising the complete pipeline:
//   HIR -> MIR -> engine rules -> evaluate
//
// It finds calls to dangerous C functions by examining the expression tree:
//   call_expression (kind=97, @routineexpr) has child 0 = callee identifier with valuetext

predicate isDangerousName(string name) {
    name = "gets" or
    name = "strcpy" or
    name = "sprintf" or
    name = "strcat"
}

// A call expression (kind 97) whose callee (child index 0) has a dangerous name
predicate dangerousCall(int call_id, string callee_name) {
    exprs(call_id, 97, _) and
    exprparents(callee_id, 0, call_id) and
    valuetext(callee_id, callee_name) and
    isDangerousName(callee_name)
}

// Find the enclosing function name for context
predicate dangerousFinding(string callee_name, string in_function) {
    dangerousCall(call_id, callee_name) and
    enclosingfunction(call_id, func_id) and
    functions(func_id, in_function, _)
}

from string callee, string caller
where dangerousFinding(callee, caller)
select callee, caller
