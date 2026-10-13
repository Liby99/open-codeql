/**
 * @name Default constructors
 * @description Counts how many default (synthesized) constructors exist on source classes.
 * @kind table
 * @id ocql-test/default-constructors
 */

from int cnt
where
  cnt = count(@constructor c | isDefConstr(c))
select cnt
