/**
 * @name Static methods
 * @description Lists all static methods and their declaring class.
 * @kind table
 * @id ocql-test/static-methods
 */

from @method mid, string mname, @classorinterface cid, string cname, @modifier mod
where
  methods(mid, mname, _, _, cid, _) and
  classes_or_interfaces(cid, cname, _, _) and
  hasModifier(mid, mod) and
  modifiers(mod, "static")
select cname, mname
