/**
 * @name Import types
 * @description Lists all import type values (single/on-demand/static).
 * @kind table
 * @id ocql-test/import-types
 */

from int kind
where imports(_, _, _, kind)
select kind
