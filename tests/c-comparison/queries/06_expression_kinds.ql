/**
 * @name Expression kinds
 * @description Lists distinct expression kind codes that appear.
 * @kind table
 * @id ocql-test-c/expression-kinds
 */

from int kind
where exists(@expr e | exprs(e, kind, _))
select kind
