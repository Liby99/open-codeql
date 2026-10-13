/**
 * @name Object creation in source
 * @description Counts new-expressions (kind 52) in source-defined callables.
 * @kind table
 * @id ocql-test/source-object-creation
 */

from int cnt
where
  cnt = count(@expr e |
    exists(@callable c |
      callableEnclosingExpr(e, c) and
      exists(@stmt s | stmts(s, _, _, _, c))
    ) and
    exprs(e, 52, _, _, _)
  )
select cnt
