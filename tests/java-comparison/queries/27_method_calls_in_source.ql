/**
 * @name Method calls in source
 * @description Counts method invocation expressions inside source-defined callables.
 * @kind table
 * @id ocql-test/source-method-calls
 */

from int cnt
where
  cnt = count(@expr e |
    exists(@callable c |
      callableEnclosingExpr(e, c) and
      exists(@stmt s | stmts(s, _, _, _, c))
    ) and
    exprs(e, 61, _, _, _)  // 61 = @methodaccess
  )
select cnt
