/**
 * @name List constructors
 * @description Lists all constructors with their declaring class.
 * @kind table
 * @id ocql-test/constructors
 */

from @classorinterface cid, string cname, @constructor coid, string coname
where
  constrs(coid, coname, _, _, cid, _) and
  classes_or_interfaces(cid, cname, _, _)
select cname, coname
