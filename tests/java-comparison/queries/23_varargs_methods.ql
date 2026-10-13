/**
 * @name Source varargs parameter count
 * @description Counts how many varargs parameters exist on source-defined callables.
 * @kind table
 * @id ocql-test/source-varargs
 */

from int cnt
where
  cnt = count(@param p, @callable c |
    isVarargsParam(p) and
    params(p, _, _, c, _) and
    exists(@stmt s | stmts(s, _, _, _, c))
  )
select cnt
