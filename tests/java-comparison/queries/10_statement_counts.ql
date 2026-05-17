/**
 * @name Statement kinds
 * @description Lists distinct statement kinds present in the database.
 * @kind table
 * @id ocql-test/statement-counts
 */

from int kind
where stmts(_, kind, _, _, _)
select kind
