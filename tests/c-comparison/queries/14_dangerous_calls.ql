/**
 * @name Calls to dangerous library functions
 * @description Finds calls to known-dangerous C library functions (gets, strcpy, sprintf).
 * @kind table
 * @id ocql-test-c/dangerous-calls
 */

from @expr call, @function target, string name
where
  funbind(call, target) and
  functions(target, name, _) and
  (name = "gets" or name = "strcpy" or name = "sprintf")
select name
