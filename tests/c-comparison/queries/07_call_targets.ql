/**
 * @name Call targets
 * @description Pairs each call expression with its resolved function name.
 * @kind table
 * @id ocql-test-c/call-targets
 */

from @expr call, @function target, string name
where
  funbind(call, target) and
  functions(target, name, _)
select name
