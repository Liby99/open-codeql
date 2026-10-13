/**
 * @name Call kinds
 * @description The kind column of every `iscall` row.
 * @kind table
 * @id ocql-test-c/call-kinds
 */

from @expr e, int kind
where iscall(e, kind)
select kind
