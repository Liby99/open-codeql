/**
 * @name User-defined type kinds
 * @description Lists user-type names with their kind code.
 * @kind table
 * @id ocql-test-c/usertype-kinds
 */

from @usertype t, string name, int kind
where usertypes(t, name, kind)
select name, kind
