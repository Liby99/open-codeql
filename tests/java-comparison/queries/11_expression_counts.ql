/**
 * @name Expression kinds
 * @description Lists distinct expression kinds present in the database.
 * @kind table
 * @id ocql-test/expression-counts
 */

from int kind
where exprs(_, kind, _, _, _)
select kind
