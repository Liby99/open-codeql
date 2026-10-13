/**
 * @name Call count per target
 * @description Number of call sites bound to each target function.
 * @kind table
 * @id ocql-test-c/call-count-per-target
 */

from @function target, string name, int n
where
  functions(target, name, _) and
  n = count(@expr call | funbind(call, target))
select name, n
