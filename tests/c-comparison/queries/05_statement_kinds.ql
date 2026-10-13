/**
 * @name Statement kinds
 * @description Lists distinct statement kind codes that appear.
 * @kind table
 * @id ocql-test-c/statement-kinds
 */

from int kind
where exists(@stmt s | stmts(s, kind, _))
select kind
