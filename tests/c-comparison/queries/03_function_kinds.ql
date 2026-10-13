/**
 * @name Function kinds
 * @description Lists each function's kind code.
 * @kind table
 * @id ocql-test-c/function-kinds
 */

from @function f, string name, int kind
where functions(f, name, kind)
select name, kind
