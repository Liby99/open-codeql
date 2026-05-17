/**
 * @name Local variables
 * @description Lists all local variable names declared in the source.
 * @kind table
 * @id ocql-test/local-variables
 */

from string name
where localvars(_, name, _, _)
select name
