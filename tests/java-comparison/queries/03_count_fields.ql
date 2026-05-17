/**
 * @name List all fields
 * @description Lists all field names with their declaring class.
 * @kind table
 * @id ocql-test/list-fields
 */

from @field fid, string fname, @classorinterface cid, string cname
where
  fields(fid, fname, _, cid) and
  classes_or_interfaces(cid, cname, _, _)
select cname, fname
