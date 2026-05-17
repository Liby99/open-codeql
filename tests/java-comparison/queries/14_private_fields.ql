/**
 * @name Private fields
 * @description Lists all private fields with their declaring class.
 * @kind table
 * @id ocql-test/private-fields
 */

from @field fid, string fname, @classorinterface cid, string cname, @modifier mod
where
  fields(fid, fname, _, cid) and
  classes_or_interfaces(cid, cname, _, _) and
  hasModifier(fid, mod) and
  modifiers(mod, "private")
select cname, fname
