/**
 * @name Literal value text
 * @description Lists the textual values recorded for literal expressions.
 * @kind table
 * @id ocql-test-c/literal-values
 */

from string text
where valuetext(_, text)
select text
