/**
 * @name List all methods
 * @description Lists all method names with their declaring class.
 * @kind table
 * @id ocql-test/list-methods
 */

from @method mid, string mname, @classorinterface cid, string cname
where
  methods(mid, mname, _, _, cid, _) and
  classes_or_interfaces(cid, cname, _, _)
select cname, mname
