/**
 * @name Source annotation count
 * @description Counts annotations on source methods (methods with stmts = source only).
 * @kind table
 * @id ocql-test/annotations
 */

from int cnt
where
  cnt = count(@expr e |
    exprs(e, 66, _, _, _) and
    exists(@callable c | stmts(_, _, _, _, c) and exprs(e, _, _, c, _))
  )
select cnt
