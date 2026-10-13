/**
 * @name Function entry points
 * @description Each function paired with the kind of its entry-point statement.
 * @kind table
 * @id ocql-test-c/function-entry-points
 */

from @function f, @stmt s, string fname, int kind
where
  function_entry_point(f, s) and
  functions(f, fname, _) and
  stmts(s, kind, _)
select fname, kind
