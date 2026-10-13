/**
 * @name Function name + kind tuples
 * @description Lists each function as a (name, kind) pair.
 * @kind table
 * @id ocql-test-c/function-signatures
 */

from @function f, string name, int kind
where functions(f, name, kind)
select name, kind
