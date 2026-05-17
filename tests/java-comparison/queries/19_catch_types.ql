/**
 * @name Catch clause count
 * @description Counts the number of catch clauses in the source.
 * @kind table
 * @id ocql-test/catch-count
 */

from int cnt
where cnt = count(@stmt s | stmts(s, 22, _, _, _))
select cnt
