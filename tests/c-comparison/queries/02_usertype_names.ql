/**
 * @name User-defined type names
 * @description Lists all user-defined type names (structs, unions, enums, typedefs).
 * @kind table
 * @id ocql-test-c/usertype-names
 */

from @usertype t, string name, int kind
where usertypes(t, name, kind)
select name
