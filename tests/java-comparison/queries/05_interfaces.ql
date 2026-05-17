/**
 * @name Interface implementations
 * @description Lists all class-implements-interface relationships.
 * @kind table
 * @id ocql-test/interfaces
 */

from @classorinterface cid, @classorinterface iid, string cname, string iname
where
  implInterface(cid, iid) and
  classes_or_interfaces(cid, cname, _, _) and
  classes_or_interfaces(iid, iname, _, _)
select cname, iname
