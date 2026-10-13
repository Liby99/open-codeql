/**
 * @name User-type name + kind tuples
 * @description Lists each user-type as a (name, kind) pair.
 * @kind table
 * @id ocql-test-c/usertype-signatures
 */

from @usertype t, string name, int kind
where usertypes(t, name, kind)
select name, kind
