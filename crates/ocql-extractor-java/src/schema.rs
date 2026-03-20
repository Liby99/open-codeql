use ocql_schema::{parse_dbscheme, DbScheme};

/// A .dbscheme for Java extraction, modeled after semmlecode.dbscheme.
///
/// This is a growing subset of the full CodeQL Java schema.
pub fn java_schema() -> DbScheme {
    parse_dbscheme(JAVA_DBSCHEME).expect("built-in Java schema should parse")
}

const JAVA_DBSCHEME: &str = r#"
/* ========== Files and locations ========== */

files(
    unique int id: @file,
    string name: string ref
);

folders(
    unique int id: @folder,
    string name: string ref
);

@container = @file | @folder

locations_default(
    unique int id: @location_default,
    int file: @file ref,
    int beginLine: int ref,
    int beginColumn: int ref,
    int endLine: int ref,
    int endColumn: int ref
);

hasLocation(
    int locatableid: @locatable ref,
    int id: @location_default ref
);

/* ========== Packages ========== */

packages(
    unique int id: @package,
    string nodeName: string ref
);

cupackage(
    unique int id: @file ref,
    int packageid: @package ref
);

/* ========== Types ========== */

primitives(
    unique int id: @primitive,
    string nodeName: string ref
);

classes_or_interfaces(
    unique int id: @classorinterface,
    string nodeName: string ref,
    int parentid: @package ref,
    int sourceid: @classorinterface ref
);

isInterface(
    unique int id: @classorinterface ref
);

isRecord(
    unique int id: @classorinterface ref
);

isEnumType(
    int classid: @classorinterface ref
);

isAnnotType(
    int interfaceid: @classorinterface ref
);

extendsReftype(
    int id1: @reftype ref,
    int id2: @classorinterface ref
);

implInterface(
    int id1: @classorinterface ref,
    int id2: @classorinterface ref
);

enclInReftype(
    unique int child: @reftype ref,
    int parent: @reftype ref
);

/* ========== Fields ========== */

fields(
    unique int id: @field,
    string nodeName: string ref,
    string typeName: string ref,
    int parentid: @classorinterface ref
);

isEnumConst(
    int fieldid: @field ref
);

/* ========== Methods and constructors ========== */

methods(
    unique int id: @method,
    string nodeName: string ref,
    string signature: string ref,
    string typeName: string ref,
    int parentid: @classorinterface ref,
    int sourceid: @method ref
);

constrs(
    unique int id: @constructor,
    string nodeName: string ref,
    string signature: string ref,
    string typeName: string ref,
    int parentid: @classorinterface ref,
    int sourceid: @constructor ref
);

@callable = @method | @constructor

/* ========== Parameters ========== */

#keyset[parentid,pos]
params(
    unique int id: @param,
    string typeName: string ref,
    int pos: int ref,
    int parentid: @callable ref,
    int sourceid: @param ref
);

paramName(
    unique int id: @param ref,
    string nodeName: string ref
);

/* ========== Local variables ========== */

localvars(
    unique int id: @localvar,
    string nodeName: string ref,
    string typeName: string ref,
    int parentid: @element ref
);

/* ========== Modifiers ========== */

modifiers(
    unique int id: @modifier,
    string nodeName: string ref
);

hasModifier(
    int id1: @modifiable ref,
    int id2: @modifier ref
);

/* ========== Imports ========== */

imports(
    unique int id: @import,
    int holder: @element ref,
    string name: string ref,
    int kind: int ref
);

/* ========== Statements ========== */

#keyset[parent,idx]
stmts(
    unique int id: @stmt,
    int kind: int ref,
    int parent: @element ref,
    int idx: int ref,
    int bodydecl: @callable ref
);

/* ========== Expressions ========== */

#keyset[parent,idx]
exprs(
    unique int id: @expr,
    int kind: int ref,
    string typeName: string ref,
    int parent: @element ref,
    int idx: int ref
);

callableEnclosingExpr(
    unique int id: @expr ref,
    int callable_id: @callable ref
);

/* ========== Annotations ========== */

annotations(
    unique int id: @annotation,
    int parentid: @element ref,
    string typeName: string ref
);

/* ========== Javadoc / comments ========== */

javadoc(
    unique int id: @javadoc,
    int parentid: @element ref
);

comments(
    unique int id: @comment,
    string contents: string ref,
    int location: @location_default ref
);

/* ========== Type variables (generics) ========== */

#keyset[parentid,pos]
typeVars(
    unique int id: @typevariable,
    string nodeName: string ref,
    int pos: int ref,
    int parentid: @element ref
);

#keyset[parentid,pos]
typeBounds(
    unique int id: @typebound,
    string typeName: string ref,
    int pos: int ref,
    int parentid: @typevariable ref
);

/* ========== Literal values ========== */

namestrings(
    string name: string ref,
    string value: string ref,
    int parent: @element ref
);

/* ========== Union types ========== */

@boundedtype = @typevariable | @wildcard

@reftype = @classorinterface | @boundedtype

@type = @primitive | @reftype

@variable = @localvar | @field

@modifiable = @classorinterface | @method | @constructor | @field | @param | @localvar

@element = @package | @modifier | @annotation | @classorinterface
         | @method | @constructor | @field | @param | @localvar
         | @typevariable | @stmt | @expr | @import | @javadoc | @comment
         | @primitive

@locatable = @element | @typebound | @file

@wildcard = @typevariable
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_java_schema_parses() {
        let schema = java_schema();
        let table_count = schema.tables().count();
        eprintln!("Java schema has {} tables", table_count);
        assert!(table_count >= 25, "Should have >= 25 tables, got {}", table_count);
    }
}
