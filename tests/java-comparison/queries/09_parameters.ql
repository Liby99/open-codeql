/**
 * @name Method parameters
 * @description Lists all method parameters with their names and positions.
 * @kind table
 * @id ocql-test/parameters
 */

from @param pid, int pos, @method mid, string mname, string pname
where
  params(pid, _, pos, mid, _) and
  paramName(pid, pname) and
  methods(mid, mname, _, _, _, _)
select mname, pname, pos
