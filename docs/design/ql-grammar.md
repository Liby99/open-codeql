# QL Grammar Specification

This is a working grammar for the QL language, derived from the official
QL language specification. This will guide the parser implementation.

## Notation

```
A B       — sequence (A followed by B)
A | B     — alternation (A or B)
A?        — optional (zero or one A)
A*        — repetition (zero or more A)
A+        — repetition (one or more A)
'literal' — keyword or operator token
```

## 1. Modules

```
qlFile     = module

module     = moduleHeader? moduleBody

moduleHeader = annotation* 'module' upperName
               typeParams? implementsClause? '{'

moduleBody = moduleMember*

moduleMember = import
             | predicate
             | classDecl
             | moduleDecl
             | alias
             | select

moduleDecl = annotation* 'module' upperName
             typeParams? implementsClause?
             '{' moduleMember* '}'

typeParams = '<' sigParam (',' sigParam)* '>'

sigParam   = 'module' upperName        // module signature param
           | 'class' upperName         // type signature param
           | predicateSig              // predicate signature param

implementsClause = 'implements' moduleExpr (',' moduleExpr)*
```

## 2. Imports

```
import     = annotation* 'import' importExpr ('as' upperName)?

importExpr = qualifiedName
           | moduleExpr
```

## 3. Types

```
typeExpr    = primitiveType
            | 'boolean'
            | 'date'
            | 'float'
            | 'int'
            | 'string'
            | className
            | dbType
            | moduleSelection '::' upperName

primitiveType = 'boolean' | 'date' | 'float' | 'int' | 'string'

className   = upperName

dbType      = '@' lowerName

typeUnion   = 'class' upperName '=' upperName ('or' upperName)+ ';'
```

## 4. Predicates

```
predicate  = annotation* predicateHead predicateBody

predicateHead = 'predicate' lowerName '(' paramList ')'
              | typeExpr lowerName '(' paramList ')'

predicateBody = '{' formula '}'
              | ';'                      // abstract/external

paramList  = (param (',' param)*)?

param      = typeExpr lowerName
```

## 5. Classes

```
classDecl  = annotation* 'class' upperName
             'extends' typeExpr (',' typeExpr)*
             ('instanceof' typeExpr (',' typeExpr)*)?
             '{' classMember* '}'

classMember = charPredicate
            | memberPredicate
            | fieldDecl

charPredicate = annotation* upperName '(' ')' '{' formula '}'

memberPredicate = annotation* predicateHead predicateBody

fieldDecl  = annotation* typeExpr lowerName ';'
```

## 6. Algebraic Datatypes

```
newtypeDecl = 'newtype' upperName '=' newtypeBranch ('or' newtypeBranch)*

newtypeBranch = upperName '(' paramList ')' ('{' formula '}')?
```

## 7. Aliases

```
alias      = annotation* moduleAlias
           | annotation* typeAlias
           | annotation* predicateAlias

moduleAlias    = 'module' upperName '=' moduleExpr ';'
typeAlias      = 'class' upperName '=' typeExpr ';'
predicateAlias = 'predicate' lowerName '=' predicateRef ';'

predicateRef = qualifiedPredicateRef '/' INTEGER
```

## 8. Signatures

```
moduleSig  = 'signature' 'module' upperName typeParams?
             '{' sigMember* '}'
           | 'signature' 'module' upperName typeParams? ';'

typeSig    = 'signature' 'class' upperName
             ('extends' typeExpr (',' typeExpr)*)?
             '{' sigPredicate* '}'
           | 'signature' 'class' upperName
             ('extends' typeExpr (',' typeExpr)*)? ';'

predicateSig = 'signature' predicateHead ';'
             | 'signature' 'predicate' lowerName '(' paramList ')' ';'

sigMember  = typeSig | predicateSig | 'default' predicate

sigPredicate = predicateHead ';'
```

## 9. Select (Query)

```
select     = fromClause? whereClause? selectClause

fromClause = 'from' varDecl (',' varDecl)*
whereClause = 'where' formula
selectClause = 'select' selectExpr (',' selectExpr)* orderBy?

selectExpr = expr ('as' lowerName)?

orderBy    = 'order' 'by' orderByExpr (',' orderByExpr)*
orderByExpr = lowerName ('asc' | 'desc')?

varDecl    = typeExpr lowerName
```

## 10. Formulas

```
formula    = disjunction

disjunction = implication ('or' implication)*

implication = conjunction ('implies' conjunction ('else' conjunction)?)?

conjunction = primary ('and' primary)*

primary    = comparison
           | instanceOf
           | inRange
           | negation
           | quantifier
           | predicateCall
           | parenFormula
           | ifThenElse
           | 'any' '(' ')'
           | 'none' '(' ')'

comparison = expr compOp expr
compOp     = '=' | '!=' | '<' | '>' | '<=' | '>='

instanceOf = expr 'instanceof' typeExpr

inRange    = expr 'in' expr

negation   = 'not' primary

quantifier = 'exists' '(' varDecl+ '|' formula ')'
           | 'exists' '(' expr ')'
           | 'forall' '(' varDecl+ '|' formula '|' formula ')'
           | 'forex' '(' varDecl+ '|' formula '|' formula ')'

predicateCall = qualifiedPredicateRef '(' argList ')'
              | expr '.' lowerName closureOp? '(' argList ')'

closureOp  = '+' | '*'

parenFormula = '(' formula ')'

ifThenElse = 'if' formula 'then' formula 'else' formula
```

## 11. Expressions

```
expr       = addExpr

addExpr    = mulExpr (('+' | '-') mulExpr)*

mulExpr    = unaryExpr (('*' | '/' | '%') unaryExpr)*

unaryExpr  = '-' unaryExpr
           | '+' unaryExpr
           | castExpr

castExpr   = postfixExpr ('.' '(' typeExpr ')')?
           | '(' typeExpr ')' castExpr

postfixExpr = primaryExpr ('.' lowerName closureOp? '(' argList ')')*

primaryExpr = literal
            | variable
            | 'this'
            | 'result'
            | '_'                        // don't-care
            | aggregation
            | parenExpr
            | callExpr
            | superExpr
            | range
            | setLiteral

literal    = INTEGER
           | FLOAT
           | STRING
           | 'true' | 'false'

variable   = lowerName

aggregation = aggKind '(' aggBody ')'
            | 'rank' '[' expr ']' '(' aggBody ')'

aggKind    = 'count' | 'min' | 'max' | 'avg' | 'sum'
           | 'concat' | 'unique' | 'any'
           | 'strictcount' | 'strictsum' | 'strictconcat'

aggBody    = varDecl* ('|' formula)? ('|' expr orderBy?)?

callExpr   = qualifiedPredicateRef '(' argList ')'

superExpr  = typeExpr '.' 'super' '.' lowerName '(' argList ')'

range      = '[' expr '..' expr ']'

setLiteral = '[' expr (',' expr)* ']'

parenExpr  = '(' expr ')'

argList    = (expr (',' expr)*)?
```

## 12. Annotations

```
annotation = '@' annotationName
           | 'pragma' '[' pragmaName ']'
           | 'bindingset' '[' lowerName (',' lowerName)* ']'
           | 'language' '[' languageFeature ']'
           | visibilityAnnotation

annotationName = 'abstract' | 'cached' | 'external' | 'extensible'
               | 'transient' | 'final' | 'override' | 'deprecated'
               | 'query' | 'additional' | 'private' | 'library'

visibilityAnnotation = 'private' | 'deprecated'

pragmaName = 'inline' | 'inline_late' | 'noinline'
           | 'nomagic' | 'noopt'
           | 'only_bind_out' | 'only_bind_into'

languageFeature = 'monotonicAggregates'
```

## 13. Module Expressions

```
moduleExpr = qualifiedName
           | moduleExpr '::' upperName
           | moduleExpr '<' moduleArg (',' moduleArg)* '>'

moduleArg  = moduleExpr
           | typeExpr
           | predicateRef

qualifiedName = (upperName '.')* upperName
```

## 14. Lexical Elements

```
// Identifiers
upperName  = [A-Z] [a-zA-Z0-9_]*
lowerName  = [a-z] [a-zA-Z0-9_]*

// Literals
INTEGER    = [0-9]+
FLOAT      = [0-9]+ '.' [0-9]+
STRING     = '"' (escape | [^"\\])* '"'

escape     = '\\' [nrt"\\]
           | '\\' 'u' hexDigit hexDigit hexDigit hexDigit

// Comments
LINE_COMMENT  = '//' [^\n]*
BLOCK_COMMENT = '/*' .* '*/'
QLDOC_COMMENT = '/**' .* '*/'

// Keywords (reserved)
keywords   = 'and' | 'any' | 'as' | 'asc' | 'boolean' | 'by'
           | 'class' | 'count' | 'date' | 'desc' | 'else'
           | 'exists' | 'extends' | 'false' | 'float' | 'forall'
           | 'forex' | 'from' | 'if' | 'implements' | 'import'
           | 'implies' | 'in' | 'instanceof' | 'int' | 'max'
           | 'min' | 'module' | 'newtype' | 'none' | 'not'
           | 'or' | 'order' | 'predicate' | 'result' | 'select'
           | 'signature' | 'string' | 'sum' | 'super' | 'then'
           | 'this' | 'true' | 'unique' | 'where' | 'avg'
           | 'concat' | 'rank' | 'strictcount' | 'strictsum'
           | 'strictconcat'
```

## 15. Precedence and Associativity

### Formula precedence (lowest to highest):
1. `implies` (right-associative)
2. `or` (left-associative)
3. `and` (left-associative)
4. `if-then-else`
5. `not` (prefix)

### Expression precedence (lowest to highest):
1. `+`, `-` (left-associative)
2. `*`, `/`, `%` (left-associative)
3. Unary `-`, `+` (prefix)
4. Cast (postfix `.()` or prefix `()`)
5. Member access `.` (left-associative)
6. Primary expressions

## Notes for Parser Implementation

1. **Ambiguity between formulas and expressions**: A predicate call can appear
   in both formula and expression context. In formula context, it's a call to
   a predicate without result. In expression context, it's a call to a predicate
   with result. The parser needs to handle this via context or backtracking.

2. **Cast ambiguity**: `(Type)expr` looks like a parenthesized expression.
   Resolved by checking if the parenthesized content is a type name.

3. **Closure operators**: `+` and `*` after a predicate name in a call are
   closure operators, not arithmetic. Context determines interpretation.

4. **Aggregation vs call**: `count(...)` could be a regular call or an
   aggregation. Resolved by checking if the name is an aggregation keyword.
