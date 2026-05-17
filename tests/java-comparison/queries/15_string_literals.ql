/**
 * @name String literals
 * @description Lists all string literal values in the code.
 * @kind table
 * @id ocql-test/string-literals
 */

from @expr id, string val
where
  exprs(id, 22, _, _, _) and
  namestrings(_, val, id)
select val
