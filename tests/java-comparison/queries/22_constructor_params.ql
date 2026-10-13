/**
 * @name Source constructor parameter count
 * @description Counts constructor parameters across all source-defined classes.
 * @kind table
 * @id ocql-test/source-constructor-params
 */

from int cnt
where
  cnt = count(@param p, @constructor c |
    params(p, _, _, c, _) and
    constrs(c, _, _, _, _, _) and
    exists(@stmt s | stmts(s, _, _, _, c))
  )
select cnt
