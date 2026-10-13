/**
 * @name Source type variables
 * @description Counts type variables on source-defined callables (limited to source via stmts).
 * @kind table
 * @id ocql-test/source-typevars
 */

from int cnt
where
  cnt = count(@typevariable tv, @callable c |
    typeVars(tv, _, _, c) and
    exists(@stmt s | stmts(s, _, _, _, c))
  )
select cnt
