/**
 * @name Uncalled source functions
 * @description Functions that are NOT the target of any call expression.
 *   In a small test project this typically reduces to `main` (and library externs).
 * @kind table
 * @id ocql-test-c/uncalled-functions
 */

from @function f, string name
where
  functions(f, name, _) and
  not exists(@expr call | funbind(call, f))
select name
