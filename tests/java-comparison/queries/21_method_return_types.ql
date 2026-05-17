/**
 * @name Source methods with params count
 * @description Counts how many methods have both parameters and statements (source-only).
 * @kind table
 * @id ocql-test/source-methods-with-params
 */

from int cnt
where
  cnt = count(@method m |
    exists(@param p | params(p, _, _, m, _)) and
    exists(@stmt s | stmts(s, _, _, _, m))
  )
select cnt
