/**
 * @name Enum constants
 * @description Lists all enum constant field names.
 * @kind table
 * @id ocql-test/enum-constants
 */

from @field f, string name
where
  isEnumConst(f) and
  fields(f, name, _, _)
select name
