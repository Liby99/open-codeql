/**
 * @name Called functions
 * @description Distinct names of functions that are the target of at least one call.
 * @kind table
 * @id ocql-test-c/called-functions
 */

from @function target, string name
where
  functions(target, name, _) and
  exists(@expr call | funbind(call, target))
select name
