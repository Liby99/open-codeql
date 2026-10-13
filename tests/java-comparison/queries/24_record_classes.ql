/**
 * @name Record classes
 * @description Counts source-declared record classes.
 * @kind table
 * @id ocql-test/record-classes
 */

from int cnt
where
  cnt = count(@classorinterface c |
    isRecord(c) and
    exists(@callable cb, @stmt s |
      stmts(s, _, _, _, cb) and
      (
        exists(@method m | methods(m, _, _, _, c, _) and m = cb)
        or exists(@constructor co | constrs(co, _, _, _, c, _) and co = cb)
      )
    )
  )
select cnt
