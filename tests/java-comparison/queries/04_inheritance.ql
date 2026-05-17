/**
 * @name Inheritance relationships
 * @description Lists all class-extends-class relationships.
 * @kind table
 * @id ocql-test/inheritance
 */

from @classorinterface sub, @classorinterface sup, string subName, string supName
where
  extendsReftype(sub, sup) and
  classes_or_interfaces(sub, subName, _, _) and
  classes_or_interfaces(sup, supName, _, _)
select subName, supName
