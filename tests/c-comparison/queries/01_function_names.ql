/**
 * @name Function names
 * @description Lists all function names extracted from the database.
 * @kind table
 * @id ocql-test-c/function-names
 */

from @function f, string name, int kind
where functions(f, name, kind)
select name
