/**
 * @name Count classes and interfaces
 * @description Counts the number of classes and interfaces in the database.
 * @kind table
 * @id ocql-test/count-classes
 */

from @classorinterface id, string name
where classes_or_interfaces(id, name, _, _)
select name
