/**
 * @name Call targets
 * @description Lists resolved call target method names.
 * @kind table
 * @id ocql-test/call-targets
 */

from string targetName
where
  exists(@method targetMethod |
    callableBinding(_, targetMethod) and
    methods(targetMethod, targetName, _, _, _, _)
  )
select targetName
