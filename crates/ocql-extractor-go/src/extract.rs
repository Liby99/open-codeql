use ocql_database::{EntityId, Value};
use ocql_extractor_common::{Extractor, FactEmitter, LocationEmitter, NodeExt};
use ocql_extractor_common::tree_sitter::{Language, Node, Tree};

/// Go extractor using tree-sitter.
///
/// Extracts:
/// - Package declarations, imports
/// - Function and method declarations
/// - Type declarations (structs, interfaces, aliases)
/// - Variable and constant declarations
/// - All statement types (if, for, range, switch, select, go, defer, return, etc.)
/// - All expression types (identifiers, literals, calls, selectors, binary/unary ops, etc.)
/// - Struct fields
/// - Comments
pub struct GoExtractor;

impl GoExtractor {
    pub fn new() -> Self {
        Self
    }
}

// Expression kind constants (matches CodeQL Go go.dbscheme)
#[allow(dead_code)]
const EXPR_BAD: i64 = 0;
const EXPR_IDENT: i64 = 1;
const EXPR_ELLIPSIS: i64 = 2;
const EXPR_INTLIT: i64 = 3;
const EXPR_FLOATLIT: i64 = 4;
const EXPR_IMAGLIT: i64 = 5;
const EXPR_CHARLIT: i64 = 6;
const EXPR_STRINGLIT: i64 = 7;
const EXPR_FUNCLIT: i64 = 8;
const EXPR_COMPOSITELIT: i64 = 9;
const EXPR_PARENEXPR: i64 = 10;
const EXPR_SELECTOREXPR: i64 = 11;
const EXPR_INDEXEXPR: i64 = 12;
const EXPR_SLICEEXPR: i64 = 15;
const EXPR_TYPEASSERTEXPR: i64 = 16;
const EXPR_CALLEXPR: i64 = 17;
const EXPR_STAREXPR: i64 = 18;
const EXPR_KEYVALUEEXPR: i64 = 19;
const EXPR_ARRAYTYPEEXPR: i64 = 20;
const EXPR_STRUCTTYPEEXPR: i64 = 21;
const EXPR_FUNCTYPEEXPR: i64 = 22;
const EXPR_INTERFACETYPEEXPR: i64 = 23;
const EXPR_MAPTYPEEXPR: i64 = 24;
const EXPR_PLUSEXPR: i64 = 26;
const EXPR_MINUSEXPR: i64 = 27;
const EXPR_NOTEXPR: i64 = 28;
const EXPR_COMPLEMENTEXPR: i64 = 29;
const EXPR_DEREFEXPR: i64 = 30;
const EXPR_ADDRESSEXPR: i64 = 31;
const EXPR_ARROWEXPR: i64 = 32;
const EXPR_LOREXPR: i64 = 33;
const EXPR_LANDEXPR: i64 = 34;
const EXPR_EQLEXPR: i64 = 35;
const EXPR_NEQEXPR: i64 = 36;
const EXPR_LSSEXPR: i64 = 37;
const EXPR_LEQEXPR: i64 = 38;
const EXPR_GTREXPR: i64 = 39;
const EXPR_GEQEXPR: i64 = 40;
const EXPR_ADDEXPR: i64 = 41;
const EXPR_SUBEXPR: i64 = 42;
const EXPR_OREXPR: i64 = 43;
const EXPR_XOREXPR: i64 = 44;
const EXPR_MULEXPR: i64 = 45;
const EXPR_QUOEXPR: i64 = 46;
const EXPR_REMEXPR: i64 = 47;
const EXPR_SHLEXPR: i64 = 48;
const EXPR_SHREXPR: i64 = 49;
const EXPR_ANDEXPR: i64 = 50;
const EXPR_ANDNOTEXPR: i64 = 51;

// Statement kind constants (matches CodeQL Go go.dbscheme)
#[allow(dead_code)]
const STMT_BAD: i64 = 0;
const STMT_DECL: i64 = 1;
const STMT_EMPTY: i64 = 2;
const STMT_LABELED: i64 = 3;
const STMT_EXPR: i64 = 4;
const STMT_SEND: i64 = 5;
const STMT_INC: i64 = 6;
const STMT_DEC: i64 = 7;
const STMT_GO: i64 = 8;
const STMT_DEFER: i64 = 9;
const STMT_RETURN: i64 = 10;
const STMT_BREAK: i64 = 11;
const STMT_CONTINUE: i64 = 12;
const STMT_GOTO: i64 = 13;
const STMT_FALLTHROUGH: i64 = 14;
const STMT_BLOCK: i64 = 15;
const STMT_IF: i64 = 16;
const STMT_CASECLAUSE: i64 = 17;
const STMT_EXPRSWITCH: i64 = 18;
const STMT_TYPESWITCH: i64 = 19;
const STMT_COMMCLAUSE: i64 = 20;
const STMT_SELECT: i64 = 21;
const STMT_FOR: i64 = 22;
const STMT_RANGE: i64 = 23;
const STMT_ASSIGN: i64 = 24;
const STMT_DEFINE: i64 = 25;

// Declaration kind constants (matches CodeQL Go go.dbscheme)
#[allow(dead_code)]
const DECL_BAD: i64 = 0;
const DECL_IMPORT: i64 = 1;
const DECL_CONST: i64 = 2;
const DECL_TYPE: i64 = 3;
const DECL_VAR: i64 = 4;
const DECL_FUNC: i64 = 5;

// Spec kind constants (matches CodeQL Go go.dbscheme)
const SPEC_IMPORT: i64 = 0;
const SPEC_VALUE: i64 = 1;
const SPEC_TYPEDEF: i64 = 2;
const SPEC_ALIAS: i64 = 3;

// Object kind constants (matches CodeQL Go go.dbscheme)
#[allow(dead_code)]
const OBJ_PKG: i64 = 0;
const OBJ_DECLTYPE: i64 = 1;
const OBJ_DECLCONST: i64 = 3;
const OBJ_DECLVAR: i64 = 5;
const OBJ_DECLFUNC: i64 = 6;
const OBJ_LABEL: i64 = 8;

// Comment kind constants
const COMMENT_LINE: i64 = 0;   // //
const COMMENT_BLOCK: i64 = 1;  // /* */

impl Extractor for GoExtractor {
    fn language(&self) -> Language {
        tree_sitter_go::LANGUAGE.into()
    }

    fn extensions(&self) -> &[&str] {
        &["go"]
    }

    fn extract_file(
        &self,
        emitter: &mut FactEmitter<'_>,
        file_id: EntityId,
        tree: &Tree,
        source: &[u8],
    ) {
        let root = tree.root_node();
        extract_source_file(emitter, file_id, &root, source);
    }
}

/// Extract the top-level source_file node.
fn extract_source_file(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    root: &Node<'_>,
    source: &[u8],
) {
    let mut decl_idx = 0i64;
    let mut comment_group_idx = 0i64;
    let mut cursor = root.walk();
    if cursor.goto_first_child() {
        loop {
            let node = cursor.node();
            if node.is_named() {
                match node.kind() {
                    "package_clause" => {
                        extract_package_clause(emitter, file_id, &node, source);
                    }
                    "import_declaration" => {
                        extract_import_declaration(emitter, file_id, &node, source, decl_idx);
                        decl_idx += 1;
                    }
                    "function_declaration" => {
                        extract_function_declaration(emitter, file_id, &node, source, decl_idx);
                        decl_idx += 1;
                    }
                    "method_declaration" => {
                        extract_method_declaration(emitter, file_id, &node, source, decl_idx);
                        decl_idx += 1;
                    }
                    "type_declaration" => {
                        extract_type_declaration(emitter, file_id, &node, source, decl_idx);
                        decl_idx += 1;
                    }
                    "var_declaration" => {
                        extract_var_declaration(emitter, file_id, &node, source, decl_idx);
                        decl_idx += 1;
                    }
                    "const_declaration" => {
                        extract_const_declaration(emitter, file_id, &node, source, decl_idx);
                        decl_idx += 1;
                    }
                    "comment" => {
                        extract_comment(emitter, file_id, &node, source, &mut comment_group_idx);
                    }
                    _ => {}
                }
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// Extract a package clause.
/// tree-sitter-go: package_clause -> "package" package_identifier
fn extract_package_clause(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    // The package name is a package_identifier child node
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "package_identifier" {
                let pkg_name = child.text(source);
                let pkg_id = emitter.alloc();
                let name_val = emitter.string(pkg_name);
                let path_val = emitter.string("");
                emitter.emit("packages", vec![
                    Value::Entity(pkg_id),
                    name_val,
                    path_val,
                ]);
                let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
                emitter.emit("has_location", vec![
                    Value::Entity(pkg_id),
                    Value::Entity(loc_id),
                ]);
                return;
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract an import declaration (single or grouped).
/// tree-sitter-go: import_declaration -> "import" import_spec | import_spec_list
fn extract_import_declaration(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    decl_idx: i64,
) {
    let decl_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    emitter.emit("decls", vec![
        Value::Entity(decl_id),
        Value::Int(DECL_IMPORT),
        Value::Entity(file_id),
        Value::Int(decl_idx),
    ]);
    emitter.emit("has_location", vec![
        Value::Entity(decl_id),
        Value::Entity(loc_id),
    ]);

    // Extract import specs
    let mut spec_idx = 0i64;
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "import_spec" {
                extract_import_spec(emitter, file_id, decl_id, &child, source, spec_idx);
                spec_idx += 1;
            } else if child.kind() == "import_spec_list" {
                let mut inner = child.walk();
                if inner.goto_first_child() {
                    loop {
                        let spec_node = inner.node();
                        if spec_node.kind() == "import_spec" {
                            extract_import_spec(emitter, file_id, decl_id, &spec_node, source, spec_idx);
                            spec_idx += 1;
                        }
                        if !inner.goto_next_sibling() { break; }
                    }
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract a single import spec.
/// tree-sitter-go: import_spec -> [name] interpreted_string_literal
fn extract_import_spec(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    decl_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    spec_idx: i64,
) {
    let spec_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    emitter.emit("specs", vec![
        Value::Entity(spec_id),
        Value::Int(SPEC_IMPORT),
        Value::Entity(decl_id),
        Value::Int(spec_idx),
    ]);
    emitter.emit("has_location", vec![
        Value::Entity(spec_id),
        Value::Entity(loc_id),
    ]);

    // Extract the import path as a string literal expression
    // The path is an interpreted_string_literal child
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "interpreted_string_literal" || child.kind() == "raw_string_literal" {
                extract_expr(emitter, file_id, &child, source, spec_id, 0);
                break;
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Find the first named child with a given kind.
fn find_child_by_kind<'a>(node: &Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == kind {
                return Some(child);
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
    None
}

/// Extract a function declaration.
/// tree-sitter-go: function_declaration -> "func" identifier parameter_list [type] [block]
fn extract_function_declaration(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    decl_idx: i64,
) {
    let decl_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    emitter.emit("decls", vec![
        Value::Entity(decl_id),
        Value::Int(DECL_FUNC),
        Value::Entity(file_id),
        Value::Int(decl_idx),
    ]);
    emitter.emit("has_location", vec![
        Value::Entity(decl_id),
        Value::Entity(loc_id),
    ]);

    // Extract the function name (identifier child)
    if let Some(name_node) = node.child_by_field("name") {
        let name = name_node.text(source);
        let obj_id = emitter.alloc();
        let name_val = emitter.string(name);
        emitter.emit("objects", vec![
            Value::Entity(obj_id),
            Value::Int(OBJ_DECLFUNC),
            name_val,
        ]);
        let name_loc = LocationEmitter::emit_for_node(emitter, file_id, &name_node);
        emitter.emit("has_location", vec![
            Value::Entity(obj_id),
            Value::Entity(name_loc),
        ]);

        // Also emit the name as an ident expression
        let ident_id = emitter.alloc();
        emitter.emit("exprs", vec![
            Value::Entity(ident_id),
            Value::Int(EXPR_IDENT),
            Value::Entity(decl_id),
            Value::Int(0),
        ]);
        let ident_loc = LocationEmitter::emit_for_node(emitter, file_id, &name_node);
        emitter.emit("has_location", vec![
            Value::Entity(ident_id),
            Value::Entity(ident_loc),
        ]);
    }

    // Extract parameters as fields
    if let Some(params) = node.child_by_field("parameters") {
        extract_parameter_fields(emitter, file_id, decl_id, &params, source);
    }

    // Extract return type
    if let Some(result) = node.child_by_field("result") {
        extract_type_expr(emitter, file_id, &result, source, decl_id, 0);
    }

    // Extract body
    if let Some(body) = node.child_by_field("body") {
        extract_block(emitter, file_id, &body, source, decl_id, 0);
    }
}

/// Extract a method declaration (function with receiver).
/// tree-sitter-go: method_declaration -> "func" parameter_list field_identifier parameter_list [type] [block]
fn extract_method_declaration(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    decl_idx: i64,
) {
    let decl_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    emitter.emit("decls", vec![
        Value::Entity(decl_id),
        Value::Int(DECL_FUNC),
        Value::Entity(file_id),
        Value::Int(decl_idx),
    ]);
    emitter.emit("has_location", vec![
        Value::Entity(decl_id),
        Value::Entity(loc_id),
    ]);

    // Extract the method name (field_identifier child)
    if let Some(name_node) = node.child_by_field("name") {
        let name = name_node.text(source);
        let obj_id = emitter.alloc();
        let name_val = emitter.string(name);
        emitter.emit("objects", vec![
            Value::Entity(obj_id),
            Value::Int(OBJ_DECLFUNC),
            name_val,
        ]);
        let name_loc = LocationEmitter::emit_for_node(emitter, file_id, &name_node);
        emitter.emit("has_location", vec![
            Value::Entity(obj_id),
            Value::Entity(name_loc),
        ]);

        let ident_id = emitter.alloc();
        emitter.emit("exprs", vec![
            Value::Entity(ident_id),
            Value::Int(EXPR_IDENT),
            Value::Entity(decl_id),
            Value::Int(0),
        ]);
        let ident_loc = LocationEmitter::emit_for_node(emitter, file_id, &name_node);
        emitter.emit("has_location", vec![
            Value::Entity(ident_id),
            Value::Entity(ident_loc),
        ]);
    }

    // Extract receiver as fields
    if let Some(receiver) = node.child_by_field("receiver") {
        extract_parameter_fields(emitter, file_id, decl_id, &receiver, source);
    }

    // Extract parameters
    if let Some(params) = node.child_by_field("parameters") {
        extract_parameter_fields(emitter, file_id, decl_id, &params, source);
    }

    // Extract body
    if let Some(body) = node.child_by_field("body") {
        extract_block(emitter, file_id, &body, source, decl_id, 0);
    }
}

/// Extract parameter list as field entities.
fn extract_parameter_fields(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    parent_id: EntityId,
    params: &Node<'_>,
    source: &[u8],
) {
    let mut field_idx = 0i64;
    let mut cursor = params.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "parameter_declaration" {
                let field_id = emitter.alloc();
                let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                emitter.emit("fields", vec![
                    Value::Entity(field_id),
                    Value::Entity(parent_id),
                    Value::Int(field_idx),
                ]);
                emitter.emit("has_location", vec![
                    Value::Entity(field_id),
                    Value::Entity(loc_id),
                ]);

                // Extract parameter names as ident expressions and objects
                let mut expr_idx = 0i64;
                let mut inner = child.walk();
                if inner.goto_first_child() {
                    loop {
                        let param_child = inner.node();
                        if param_child.kind() == "identifier" {
                            let param_name = param_child.text(source);
                            let obj_id = emitter.alloc();
                            let name_val = emitter.string(param_name);
                            emitter.emit("objects", vec![
                                Value::Entity(obj_id),
                                Value::Int(OBJ_DECLVAR),
                                name_val,
                            ]);
                            let ident_id = emitter.alloc();
                            emitter.emit("exprs", vec![
                                Value::Entity(ident_id),
                                Value::Int(EXPR_IDENT),
                                Value::Entity(field_id),
                                Value::Int(expr_idx),
                            ]);
                            let ident_loc = LocationEmitter::emit_for_node(emitter, file_id, &param_child);
                            emitter.emit("has_location", vec![
                                Value::Entity(ident_id),
                                Value::Entity(ident_loc),
                            ]);
                            expr_idx += 1;
                        }
                        if !inner.goto_next_sibling() { break; }
                    }
                }

                // Extract the type expression
                if let Some(type_node) = child.child_by_field("type") {
                    extract_type_expr(emitter, file_id, &type_node, source, field_id, expr_idx);
                }

                field_idx += 1;
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract a type expression node (used in parameter types, return types, etc.)
fn extract_type_expr(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    _source: &[u8],
    parent_id: EntityId,
    idx: i64,
) -> Option<EntityId> {
    let kind = match node.kind() {
        "type_identifier" | "identifier" => EXPR_IDENT,
        "pointer_type" => EXPR_STAREXPR,
        "array_type" | "slice_type" => EXPR_ARRAYTYPEEXPR,
        "map_type" => EXPR_MAPTYPEEXPR,
        "struct_type" => EXPR_STRUCTTYPEEXPR,
        "interface_type" => EXPR_INTERFACETYPEEXPR,
        "function_type" => EXPR_FUNCTYPEEXPR,
        "qualified_type" => EXPR_SELECTOREXPR,
        "channel_type" => EXPR_IDENT,
        _ => EXPR_IDENT,
    };

    let expr_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    emitter.emit("exprs", vec![
        Value::Entity(expr_id),
        Value::Int(kind),
        Value::Entity(parent_id),
        Value::Int(idx),
    ]);
    emitter.emit("has_location", vec![
        Value::Entity(expr_id),
        Value::Entity(loc_id),
    ]);

    Some(expr_id)
}

/// Extract a type declaration.
fn extract_type_declaration(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    decl_idx: i64,
) {
    let decl_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    emitter.emit("decls", vec![
        Value::Entity(decl_id),
        Value::Int(DECL_TYPE),
        Value::Entity(file_id),
        Value::Int(decl_idx),
    ]);
    emitter.emit("has_location", vec![
        Value::Entity(decl_id),
        Value::Entity(loc_id),
    ]);

    // Extract type specs and type aliases
    let mut spec_idx = 0i64;
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "type_spec" {
                extract_type_spec(emitter, file_id, decl_id, &child, source, spec_idx, false);
                spec_idx += 1;
            } else if child.kind() == "type_alias" {
                extract_type_spec(emitter, file_id, decl_id, &child, source, spec_idx, true);
                spec_idx += 1;
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract a single type spec or type alias.
fn extract_type_spec(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    decl_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    spec_idx: i64,
    is_alias: bool,
) {
    let spec_kind = if is_alias { SPEC_ALIAS } else { SPEC_TYPEDEF };

    let spec_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    emitter.emit("specs", vec![
        Value::Entity(spec_id),
        Value::Int(spec_kind),
        Value::Entity(decl_id),
        Value::Int(spec_idx),
    ]);
    emitter.emit("has_location", vec![
        Value::Entity(spec_id),
        Value::Entity(loc_id),
    ]);

    // Extract the type name
    // In type_spec: first child is type_identifier
    // In type_alias: first child is type_identifier
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "type_identifier" {
                let name = child.text(source);
                let obj_id = emitter.alloc();
                let name_val = emitter.string(name);
                emitter.emit("objects", vec![
                    Value::Entity(obj_id),
                    Value::Int(OBJ_DECLTYPE),
                    name_val,
                ]);
                let name_loc = LocationEmitter::emit_for_node(emitter, file_id, &child);
                emitter.emit("has_location", vec![
                    Value::Entity(obj_id),
                    Value::Entity(name_loc),
                ]);

                let ident_id = emitter.alloc();
                emitter.emit("exprs", vec![
                    Value::Entity(ident_id),
                    Value::Int(EXPR_IDENT),
                    Value::Entity(spec_id),
                    Value::Int(0),
                ]);
                let ident_loc = LocationEmitter::emit_for_node(emitter, file_id, &child);
                emitter.emit("has_location", vec![
                    Value::Entity(ident_id),
                    Value::Entity(ident_loc),
                ]);
                break;
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }

    // Extract the type body
    if let Some(type_node) = node.child_by_field("type") {
        match type_node.kind() {
            "struct_type" => {
                extract_struct_type(emitter, file_id, spec_id, &type_node, source);
            }
            "interface_type" => {
                extract_interface_type(emitter, file_id, spec_id, &type_node, source);
            }
            _ => {
                extract_type_expr(emitter, file_id, &type_node, source, spec_id, 1);
            }
        }
    } else {
        // For type_alias, look for a type child that's not the name
        let mut cursor = node.walk();
        let mut found_name = false;
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "type_identifier" {
                    if found_name {
                        // Second type_identifier is the aliased type
                        extract_type_expr(emitter, file_id, &child, source, spec_id, 1);
                        break;
                    }
                    found_name = true;
                }
                if !cursor.goto_next_sibling() { break; }
            }
        }
    }
}

/// Extract a struct type, including its field declarations.
fn extract_struct_type(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    parent_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let expr_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    emitter.emit("exprs", vec![
        Value::Entity(expr_id),
        Value::Int(EXPR_STRUCTTYPEEXPR),
        Value::Entity(parent_id),
        Value::Int(1),
    ]);
    emitter.emit("has_location", vec![
        Value::Entity(expr_id),
        Value::Entity(loc_id),
    ]);

    // Extract field declarations from field_declaration_list
    if let Some(field_list) = find_child_by_kind(node, "field_declaration_list") {
        let mut field_idx = 0i64;
        let mut cursor = field_list.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "field_declaration" {
                    let field_id = emitter.alloc();
                    let floc = LocationEmitter::emit_for_node(emitter, file_id, &child);
                    emitter.emit("fields", vec![
                        Value::Entity(field_id),
                        Value::Entity(expr_id),
                        Value::Int(field_idx),
                    ]);
                    emitter.emit("has_location", vec![
                        Value::Entity(field_id),
                        Value::Entity(floc),
                    ]);

                    // Extract field names as ident expressions and objects
                    let mut inner_idx = 0i64;
                    let mut inner = child.walk();
                    if inner.goto_first_child() {
                        loop {
                            let fc = inner.node();
                            if fc.kind() == "field_identifier" {
                                let fname = fc.text(source);
                                let obj_id = emitter.alloc();
                                let name_val = emitter.string(fname);
                                emitter.emit("objects", vec![
                                    Value::Entity(obj_id),
                                    Value::Int(OBJ_DECLVAR),
                                    name_val,
                                ]);

                                let ident_id = emitter.alloc();
                                emitter.emit("exprs", vec![
                                    Value::Entity(ident_id),
                                    Value::Int(EXPR_IDENT),
                                    Value::Entity(field_id),
                                    Value::Int(inner_idx),
                                ]);
                                let iloc = LocationEmitter::emit_for_node(emitter, file_id, &fc);
                                emitter.emit("has_location", vec![
                                    Value::Entity(ident_id),
                                    Value::Entity(iloc),
                                ]);
                                inner_idx += 1;
                            }
                            if !inner.goto_next_sibling() { break; }
                        }
                    }

                    // Extract field type
                    if let Some(type_node) = child.child_by_field("type") {
                        extract_type_expr(emitter, file_id, &type_node, source, field_id, inner_idx);
                    }

                    field_idx += 1;
                }
                if !cursor.goto_next_sibling() { break; }
            }
        }
    }
}

/// Extract an interface type, including its method specs.
fn extract_interface_type(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    parent_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let expr_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    emitter.emit("exprs", vec![
        Value::Entity(expr_id),
        Value::Int(EXPR_INTERFACETYPEEXPR),
        Value::Entity(parent_id),
        Value::Int(1),
    ]);
    emitter.emit("has_location", vec![
        Value::Entity(expr_id),
        Value::Entity(loc_id),
    ]);

    // Extract method specs as fields
    let mut field_idx = 0i64;
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "method_elem" || child.kind() == "method_spec" {
                let field_id = emitter.alloc();
                let floc = LocationEmitter::emit_for_node(emitter, file_id, &child);
                emitter.emit("fields", vec![
                    Value::Entity(field_id),
                    Value::Entity(expr_id),
                    Value::Int(field_idx),
                ]);
                emitter.emit("has_location", vec![
                    Value::Entity(field_id),
                    Value::Entity(floc),
                ]);

                // Extract the method name (field_identifier)
                if let Some(name_node) = find_child_by_kind(&child, "field_identifier") {
                    let mname = name_node.text(source);
                    let obj_id = emitter.alloc();
                    let name_val = emitter.string(mname);
                    emitter.emit("objects", vec![
                        Value::Entity(obj_id),
                        Value::Int(OBJ_DECLFUNC),
                        name_val,
                    ]);

                    let ident_id = emitter.alloc();
                    emitter.emit("exprs", vec![
                        Value::Entity(ident_id),
                        Value::Int(EXPR_IDENT),
                        Value::Entity(field_id),
                        Value::Int(0),
                    ]);
                    let iloc = LocationEmitter::emit_for_node(emitter, file_id, &name_node);
                    emitter.emit("has_location", vec![
                        Value::Entity(ident_id),
                        Value::Entity(iloc),
                    ]);
                }

                field_idx += 1;
            }
            // Embedded interface type
            if child.kind() == "type_identifier" || child.kind() == "qualified_type" {
                let field_id = emitter.alloc();
                let floc = LocationEmitter::emit_for_node(emitter, file_id, &child);
                emitter.emit("fields", vec![
                    Value::Entity(field_id),
                    Value::Entity(expr_id),
                    Value::Int(field_idx),
                ]);
                emitter.emit("has_location", vec![
                    Value::Entity(field_id),
                    Value::Entity(floc),
                ]);
                extract_type_expr(emitter, file_id, &child, source, field_id, 0);
                field_idx += 1;
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract a var declaration.
fn extract_var_declaration(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    decl_idx: i64,
) {
    let decl_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    emitter.emit("decls", vec![
        Value::Entity(decl_id),
        Value::Int(DECL_VAR),
        Value::Entity(file_id),
        Value::Int(decl_idx),
    ]);
    emitter.emit("has_location", vec![
        Value::Entity(decl_id),
        Value::Entity(loc_id),
    ]);

    let mut spec_idx = 0i64;
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "var_spec" {
                extract_value_spec(emitter, file_id, decl_id, &child, source, spec_idx, OBJ_DECLVAR);
                spec_idx += 1;
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract a const declaration.
fn extract_const_declaration(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    decl_idx: i64,
) {
    let decl_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    emitter.emit("decls", vec![
        Value::Entity(decl_id),
        Value::Int(DECL_CONST),
        Value::Entity(file_id),
        Value::Int(decl_idx),
    ]);
    emitter.emit("has_location", vec![
        Value::Entity(decl_id),
        Value::Entity(loc_id),
    ]);

    let mut spec_idx = 0i64;
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "const_spec" {
                extract_value_spec(emitter, file_id, decl_id, &child, source, spec_idx, OBJ_DECLCONST);
                spec_idx += 1;
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract a value spec (used for both var_spec and const_spec).
/// tree-sitter-go: var_spec/const_spec -> identifier+ ["=" expression_list]
fn extract_value_spec(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    decl_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    spec_idx: i64,
    obj_kind: i64,
) {
    let spec_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    emitter.emit("specs", vec![
        Value::Entity(spec_id),
        Value::Int(SPEC_VALUE),
        Value::Entity(decl_id),
        Value::Int(spec_idx),
    ]);
    emitter.emit("has_location", vec![
        Value::Entity(spec_id),
        Value::Entity(loc_id),
    ]);

    // Extract names (identifier children before "=" or type)
    let mut expr_idx = 0i64;
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "identifier" {
                let name = child.text(source);
                let obj_id = emitter.alloc();
                let name_val = emitter.string(name);
                emitter.emit("objects", vec![
                    Value::Entity(obj_id),
                    Value::Int(obj_kind),
                    name_val,
                ]);
                let oloc = LocationEmitter::emit_for_node(emitter, file_id, &child);
                emitter.emit("has_location", vec![
                    Value::Entity(obj_id),
                    Value::Entity(oloc),
                ]);

                let ident_id = emitter.alloc();
                emitter.emit("exprs", vec![
                    Value::Entity(ident_id),
                    Value::Int(EXPR_IDENT),
                    Value::Entity(spec_id),
                    Value::Int(expr_idx),
                ]);
                let iloc = LocationEmitter::emit_for_node(emitter, file_id, &child);
                emitter.emit("has_location", vec![
                    Value::Entity(ident_id),
                    Value::Entity(iloc),
                ]);
                expr_idx += 1;
            }
            // Extract type if present
            if child.kind() == "type_identifier" || child.kind() == "pointer_type"
                || child.kind() == "array_type" || child.kind() == "slice_type"
                || child.kind() == "map_type" || child.kind() == "struct_type"
                || child.kind() == "interface_type" || child.kind() == "function_type"
                || child.kind() == "qualified_type" || child.kind() == "channel_type"
            {
                extract_type_expr(emitter, file_id, &child, source, spec_id, expr_idx);
            }
            // Extract value expressions
            if child.kind() == "expression_list" {
                let mut val_idx = expr_idx;
                let mut inner = child.walk();
                if inner.goto_first_child() {
                    loop {
                        let val_child = inner.node();
                        if val_child.is_named() {
                            if let Some(_) = extract_expr(emitter, file_id, &val_child, source, spec_id, val_idx) {
                                val_idx += 1;
                            }
                        }
                        if !inner.goto_next_sibling() { break; }
                    }
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract a block statement.
/// tree-sitter-go: block -> "{" statement_list "}"
/// statement_list contains the actual statements.
fn extract_block(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
    index: i64,
) -> Option<EntityId> {
    if node.kind() != "block" {
        return None;
    }

    let stmt_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    emitter.emit("stmts", vec![
        Value::Entity(stmt_id),
        Value::Int(STMT_BLOCK),
        Value::Entity(parent_id),
        Value::Int(index),
    ]);
    emitter.emit("has_location", vec![
        Value::Entity(stmt_id),
        Value::Entity(loc_id),
    ]);

    // Find statement_list and extract its children
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "statement_list" {
                extract_statement_list(emitter, file_id, &child, source, stmt_id);
                break;
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }

    Some(stmt_id)
}

/// Extract statements from a statement_list node.
fn extract_statement_list(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let mut child_index = 0i64;
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.is_named() {
                if let Some(_) = extract_stmt(emitter, file_id, &child, source, parent_id, child_index) {
                    child_index += 1;
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract a statement, returning the entity ID.
fn extract_stmt(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
    index: i64,
) -> Option<EntityId> {
    // Handle block specially
    if node.kind() == "block" {
        return extract_block(emitter, file_id, node, source, parent_id, index);
    }

    let kind = match node.kind() {
        "if_statement" => Some(STMT_IF),
        "for_statement" => {
            // Check if range-based
            let is_range = find_child_by_kind(node, "range_clause").is_some();
            if is_range { Some(STMT_RANGE) } else { Some(STMT_FOR) }
        }
        "expression_switch_statement" => Some(STMT_EXPRSWITCH),
        "type_switch_statement" => Some(STMT_TYPESWITCH),
        "select_statement" => Some(STMT_SELECT),
        "return_statement" => Some(STMT_RETURN),
        "go_statement" => Some(STMT_GO),
        "defer_statement" => Some(STMT_DEFER),
        "send_statement" => Some(STMT_SEND),
        "expression_statement" => Some(STMT_EXPR),
        "inc_statement" => Some(STMT_INC),
        "dec_statement" => Some(STMT_DEC),
        "assignment_statement" => Some(STMT_ASSIGN),
        "short_var_declaration" => Some(STMT_DEFINE),
        "labeled_statement" => Some(STMT_LABELED),
        "break_statement" => Some(STMT_BREAK),
        "continue_statement" => Some(STMT_CONTINUE),
        "goto_statement" => Some(STMT_GOTO),
        "fallthrough_statement" => Some(STMT_FALLTHROUGH),
        "empty_statement" => Some(STMT_EMPTY),
        "var_declaration" | "const_declaration" | "type_declaration" => Some(STMT_DECL),
        "expression_case" | "type_case" | "default_case" => Some(STMT_CASECLAUSE),
        "communication_case" => Some(STMT_COMMCLAUSE),
        _ => None,
    };

    let stmt_kind = match kind {
        Some(k) => k,
        None => return None,
    };

    let stmt_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    emitter.emit("stmts", vec![
        Value::Entity(stmt_id),
        Value::Int(stmt_kind),
        Value::Entity(parent_id),
        Value::Int(index),
    ]);

    emitter.emit("has_location", vec![
        Value::Entity(stmt_id),
        Value::Entity(loc_id),
    ]);

    // Process children based on statement kind
    match node.kind() {
        "if_statement" => {
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, stmt_id, 0);
            }
            if let Some(init) = node.child_by_field("initializer") {
                extract_stmt(emitter, file_id, &init, source, stmt_id, 0);
            }
            if let Some(consequence) = node.child_by_field("consequence") {
                extract_block(emitter, file_id, &consequence, source, stmt_id, 1);
            }
            if let Some(alternative) = node.child_by_field("alternative") {
                if alternative.kind() == "block" {
                    extract_block(emitter, file_id, &alternative, source, stmt_id, 2);
                } else {
                    extract_stmt(emitter, file_id, &alternative, source, stmt_id, 2);
                }
            }
        }
        "for_statement" => {
            // Extract body
            if let Some(body) = node.child_by_field("body") {
                extract_block(emitter, file_id, &body, source, stmt_id, 0);
            }
            // Extract condition (simple for loop)
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, stmt_id, 0);
            }
            // Range clause or for clause
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "range_clause" {
                        if let Some(right) = child.child_by_field("right") {
                            extract_expr(emitter, file_id, &right, source, stmt_id, 0);
                        }
                        if let Some(left) = child.child_by_field("left") {
                            extract_expr_list(emitter, file_id, &left, source, stmt_id, 1);
                        }
                    }
                    if child.kind() == "for_clause" {
                        if let Some(init) = child.child_by_field("initializer") {
                            extract_stmt(emitter, file_id, &init, source, stmt_id, 0);
                        }
                        if let Some(cond) = child.child_by_field("condition") {
                            extract_expr(emitter, file_id, &cond, source, stmt_id, 0);
                        }
                        if let Some(update) = child.child_by_field("update") {
                            extract_stmt(emitter, file_id, &update, source, stmt_id, 1);
                        }
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "expression_switch_statement" => {
            if let Some(value) = node.child_by_field("value") {
                extract_expr(emitter, file_id, &value, source, stmt_id, 0);
            }
            let mut case_idx = 0i64;
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "expression_case" || child.kind() == "default_case" {
                        extract_stmt(emitter, file_id, &child, source, stmt_id, case_idx);
                        case_idx += 1;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "type_switch_statement" => {
            let mut case_idx = 0i64;
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "type_case" || child.kind() == "default_case" {
                        extract_stmt(emitter, file_id, &child, source, stmt_id, case_idx);
                        case_idx += 1;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "select_statement" => {
            let mut case_idx = 0i64;
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "communication_case" || child.kind() == "default_case" {
                        extract_stmt(emitter, file_id, &child, source, stmt_id, case_idx);
                        case_idx += 1;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "expression_case" | "type_case" | "default_case" | "communication_case" => {
            // Extract case values as expressions
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "expression_list" {
                        extract_expr_list(emitter, file_id, &child, source, stmt_id, 0);
                    }
                    // Extract case body from statement_list
                    if child.kind() == "statement_list" {
                        extract_statement_list(emitter, file_id, &child, source, stmt_id);
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "return_statement" => {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "expression_list" {
                        extract_expr_list(emitter, file_id, &child, source, stmt_id, 0);
                    } else if child.is_named() && child.kind() != "return" {
                        extract_expr(emitter, file_id, &child, source, stmt_id, 0);
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "go_statement" | "defer_statement" => {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() && child.kind() != "go" && child.kind() != "defer" {
                        extract_expr(emitter, file_id, &child, source, stmt_id, 0);
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "expression_statement" => {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, &child, source, stmt_id, 0);
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "send_statement" => {
            if let Some(channel) = node.child_by_field("channel") {
                extract_expr(emitter, file_id, &channel, source, stmt_id, 0);
            }
            if let Some(value) = node.child_by_field("value") {
                extract_expr(emitter, file_id, &value, source, stmt_id, 1);
            }
        }
        "inc_statement" | "dec_statement" => {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, &child, source, stmt_id, 0);
                        break;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "assignment_statement" => {
            if let Some(left) = node.child_by_field("left") {
                extract_expr_list(emitter, file_id, &left, source, stmt_id, 0);
            }
            if let Some(right) = node.child_by_field("right") {
                extract_expr_list(emitter, file_id, &right, source, stmt_id, 1);
            }
        }
        "short_var_declaration" => {
            if let Some(left) = node.child_by_field("left") {
                extract_expr_list(emitter, file_id, &left, source, stmt_id, 0);
            }
            if let Some(right) = node.child_by_field("right") {
                extract_expr_list(emitter, file_id, &right, source, stmt_id, 1);
            }
        }
        "labeled_statement" => {
            // Extract label name
            if let Some(label) = node.child_by_field("label") {
                let label_name = label.text(source);
                let obj_id = emitter.alloc();
                let name_val = emitter.string(label_name);
                emitter.emit("objects", vec![
                    Value::Entity(obj_id),
                    Value::Int(OBJ_LABEL),
                    name_val,
                ]);
            }
            // Extract the labeled statement body
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() && child.kind() != "label_name" {
                        extract_stmt(emitter, file_id, &child, source, stmt_id, 0);
                        break;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "var_declaration" | "const_declaration" | "type_declaration" => {
            // These are declaration statements
            let inner_decl_id = emitter.alloc();
            let inner_kind = match node.kind() {
                "var_declaration" => DECL_VAR,
                "const_declaration" => DECL_CONST,
                "type_declaration" => DECL_TYPE,
                _ => DECL_BAD,
            };
            let iloc = LocationEmitter::emit_for_node(emitter, file_id, node);
            emitter.emit("decls", vec![
                Value::Entity(inner_decl_id),
                Value::Int(inner_kind),
                Value::Entity(stmt_id),
                Value::Int(0),
            ]);
            emitter.emit("has_location", vec![
                Value::Entity(inner_decl_id),
                Value::Entity(iloc),
            ]);

            let mut spec_idx = 0i64;
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    match child.kind() {
                        "var_spec" => {
                            extract_value_spec(emitter, file_id, inner_decl_id, &child, source, spec_idx, OBJ_DECLVAR);
                            spec_idx += 1;
                        }
                        "const_spec" => {
                            extract_value_spec(emitter, file_id, inner_decl_id, &child, source, spec_idx, OBJ_DECLCONST);
                            spec_idx += 1;
                        }
                        "type_spec" => {
                            extract_type_spec(emitter, file_id, inner_decl_id, &child, source, spec_idx, false);
                            spec_idx += 1;
                        }
                        "type_alias" => {
                            extract_type_spec(emitter, file_id, inner_decl_id, &child, source, spec_idx, true);
                            spec_idx += 1;
                        }
                        _ => {}
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        _ => {}
    }

    Some(stmt_id)
}

/// Extract an expression list (comma-separated expressions).
fn extract_expr_list(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
    base_idx: i64,
) {
    if node.kind() == "expression_list" {
        let mut idx = base_idx;
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.is_named() {
                    if let Some(_) = extract_expr(emitter, file_id, &child, source, parent_id, idx) {
                        idx += 1;
                    }
                }
                if !cursor.goto_next_sibling() { break; }
            }
        }
    } else {
        extract_expr(emitter, file_id, node, source, parent_id, base_idx);
    }
}

/// Extract an expression.
fn extract_expr(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
    index: i64,
) -> Option<EntityId> {
    let kind = match node.kind() {
        "identifier" | "type_identifier" | "field_identifier" | "package_identifier" => Some(EXPR_IDENT),
        "int_literal" => Some(EXPR_INTLIT),
        "float_literal" => Some(EXPR_FLOATLIT),
        "imaginary_literal" => Some(EXPR_IMAGLIT),
        "rune_literal" => Some(EXPR_CHARLIT),
        "raw_string_literal" | "interpreted_string_literal" => Some(EXPR_STRINGLIT),
        "func_literal" => Some(EXPR_FUNCLIT),
        "composite_literal" => Some(EXPR_COMPOSITELIT),
        "parenthesized_expression" => Some(EXPR_PARENEXPR),
        "selector_expression" => Some(EXPR_SELECTOREXPR),
        "index_expression" => Some(EXPR_INDEXEXPR),
        "slice_expression" => Some(EXPR_SLICEEXPR),
        "type_assertion_expression" => Some(EXPR_TYPEASSERTEXPR),
        "call_expression" => Some(EXPR_CALLEXPR),
        "unary_expression" => {
            let op = find_go_operator(node, source);
            Some(match op.as_str() {
                "+" => EXPR_PLUSEXPR,
                "-" => EXPR_MINUSEXPR,
                "!" => EXPR_NOTEXPR,
                "^" => EXPR_COMPLEMENTEXPR,
                "*" => EXPR_DEREFEXPR,
                "&" => EXPR_ADDRESSEXPR,
                "<-" => EXPR_ARROWEXPR,
                _ => EXPR_PLUSEXPR,
            })
        }
        "binary_expression" => {
            let op = find_go_operator(node, source);
            Some(go_binary_op_kind(&op))
        }
        "keyed_element" => Some(EXPR_KEYVALUEEXPR),
        "variadic_argument" => Some(EXPR_ELLIPSIS),
        "array_type" | "slice_type" => Some(EXPR_ARRAYTYPEEXPR),
        "map_type" => Some(EXPR_MAPTYPEEXPR),
        "struct_type" => Some(EXPR_STRUCTTYPEEXPR),
        "interface_type" => Some(EXPR_INTERFACETYPEEXPR),
        "function_type" => Some(EXPR_FUNCTYPEEXPR),
        "pointer_type" => Some(EXPR_STAREXPR),
        "qualified_type" => Some(EXPR_SELECTOREXPR),
        "true" | "false" | "nil" | "iota" => Some(EXPR_IDENT),
        "literal_value" => return extract_literal_value_children(emitter, file_id, node, source, parent_id, index),
        _ => None,
    };

    let expr_kind = match kind {
        Some(k) => k,
        None => return None,
    };

    let expr_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    emitter.emit("exprs", vec![
        Value::Entity(expr_id),
        Value::Int(expr_kind),
        Value::Entity(parent_id),
        Value::Int(index),
    ]);

    emitter.emit("has_location", vec![
        Value::Entity(expr_id),
        Value::Entity(loc_id),
    ]);

    // Emit literal values
    match node.kind() {
        "int_literal" | "float_literal" | "imaginary_literal" | "rune_literal" => {
            let raw_text = node.text(source);
            let value_val = emitter.string(raw_text);
            let raw_val = emitter.string(raw_text);
            emitter.emit("literals", vec![
                Value::Entity(expr_id),
                value_val,
                raw_val,
            ]);
        }
        "raw_string_literal" | "interpreted_string_literal" => {
            let raw_text = node.text(source);
            // For string literals, strip quotes for the value
            let value = if raw_text.len() >= 2 {
                &raw_text[1..raw_text.len()-1]
            } else {
                raw_text
            };
            let value_val = emitter.string(value);
            let raw_val = emitter.string(raw_text);
            emitter.emit("literals", vec![
                Value::Entity(expr_id),
                value_val,
                raw_val,
            ]);
        }
        _ => {}
    }

    // Recurse into children
    match node.kind() {
        "call_expression" => {
            if let Some(func) = node.child_by_field("function") {
                extract_expr(emitter, file_id, &func, source, expr_id, 0);
            }
            if let Some(args) = node.child_by_field("arguments") {
                let mut idx = 1i64;
                let mut cursor = args.walk();
                if cursor.goto_first_child() {
                    loop {
                        let child = cursor.node();
                        if child.is_named() {
                            if let Some(_) = extract_expr(emitter, file_id, &child, source, expr_id, idx) {
                                idx += 1;
                            }
                        }
                        if !cursor.goto_next_sibling() { break; }
                    }
                }
            }
        }
        "selector_expression" | "qualified_type" => {
            if let Some(operand) = node.child_by_field("operand") {
                extract_expr(emitter, file_id, &operand, source, expr_id, 0);
            }
            if let Some(field) = node.child_by_field("field") {
                extract_expr(emitter, file_id, &field, source, expr_id, 1);
            }
        }
        "index_expression" => {
            if let Some(operand) = node.child_by_field("operand") {
                extract_expr(emitter, file_id, &operand, source, expr_id, 0);
            }
            if let Some(idx) = node.child_by_field("index") {
                extract_expr(emitter, file_id, &idx, source, expr_id, 1);
            }
        }
        "slice_expression" => {
            if let Some(operand) = node.child_by_field("operand") {
                extract_expr(emitter, file_id, &operand, source, expr_id, 0);
            }
            if let Some(start) = node.child_by_field("start") {
                extract_expr(emitter, file_id, &start, source, expr_id, 1);
            }
            if let Some(end) = node.child_by_field("end") {
                extract_expr(emitter, file_id, &end, source, expr_id, 2);
            }
            if let Some(capacity) = node.child_by_field("capacity") {
                extract_expr(emitter, file_id, &capacity, source, expr_id, 3);
            }
        }
        "type_assertion_expression" => {
            if let Some(operand) = node.child_by_field("operand") {
                extract_expr(emitter, file_id, &operand, source, expr_id, 0);
            }
            if let Some(type_node) = node.child_by_field("type") {
                extract_type_expr(emitter, file_id, &type_node, source, expr_id, 1);
            }
        }
        "composite_literal" => {
            // Extract the type
            if let Some(type_node) = node.child_by_field("type") {
                extract_expr(emitter, file_id, &type_node, source, expr_id, 0);
            }
            // Extract the body (literal_value)
            if let Some(body) = node.child_by_field("body") {
                let mut idx = 1i64;
                let mut cursor = body.walk();
                if cursor.goto_first_child() {
                    loop {
                        let child = cursor.node();
                        if child.is_named() {
                            if let Some(_) = extract_expr(emitter, file_id, &child, source, expr_id, idx) {
                                idx += 1;
                            }
                        }
                        if !cursor.goto_next_sibling() { break; }
                    }
                }
            }
        }
        "keyed_element" => {
            let mut child_idx = 0i64;
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        if let Some(_) = extract_expr(emitter, file_id, &child, source, expr_id, child_idx) {
                            child_idx += 1;
                        }
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "binary_expression" => {
            if let Some(left) = node.child_by_field("left") {
                extract_expr(emitter, file_id, &left, source, expr_id, 0);
            }
            if let Some(right) = node.child_by_field("right") {
                extract_expr(emitter, file_id, &right, source, expr_id, 1);
            }
        }
        "unary_expression" => {
            if let Some(operand) = node.child_by_field("operand") {
                extract_expr(emitter, file_id, &operand, source, expr_id, 0);
            }
        }
        "parenthesized_expression" => {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, &child, source, expr_id, 0);
                        break;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "func_literal" => {
            // Extract parameters
            if let Some(params) = node.child_by_field("parameters") {
                extract_parameter_fields(emitter, file_id, expr_id, &params, source);
            }
            // Extract body
            if let Some(body) = node.child_by_field("body") {
                extract_block(emitter, file_id, &body, source, expr_id, 0);
            }
        }
        "variadic_argument" => {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, &child, source, expr_id, 0);
                        break;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        _ => {}
    }

    Some(expr_id)
}

/// Helper: extract children of a literal_value as individual expression children of a parent.
fn extract_literal_value_children(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
    base_idx: i64,
) -> Option<EntityId> {
    let mut idx = base_idx;
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.is_named() {
                if let Some(_) = extract_expr(emitter, file_id, &child, source, parent_id, idx) {
                    idx += 1;
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
    None
}

/// Find the operator in a unary or binary expression.
fn find_go_operator(node: &Node<'_>, source: &[u8]) -> String {
    // tree-sitter-go stores operator as an unnamed child token
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if !child.is_named() {
                let text = child.text(source);
                match text {
                    "+" | "-" | "*" | "/" | "%" | "&" | "|" | "^"
                    | "<<" | ">>" | "&^" | "==" | "!=" | "<" | ">" | "<=" | ">="
                    | "&&" | "||" | "!" | "<-" => {
                        return text.to_string();
                    }
                    _ => {}
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
    String::new()
}

/// Map a Go binary operator to its expression kind.
fn go_binary_op_kind(op: &str) -> i64 {
    match op {
        "||" => EXPR_LOREXPR,
        "&&" => EXPR_LANDEXPR,
        "==" => EXPR_EQLEXPR,
        "!=" => EXPR_NEQEXPR,
        "<" => EXPR_LSSEXPR,
        "<=" => EXPR_LEQEXPR,
        ">" => EXPR_GTREXPR,
        ">=" => EXPR_GEQEXPR,
        "+" => EXPR_ADDEXPR,
        "-" => EXPR_SUBEXPR,
        "|" => EXPR_OREXPR,
        "^" => EXPR_XOREXPR,
        "*" => EXPR_MULEXPR,
        "/" => EXPR_QUOEXPR,
        "%" => EXPR_REMEXPR,
        "<<" => EXPR_SHLEXPR,
        ">>" => EXPR_SHREXPR,
        "&" => EXPR_ANDEXPR,
        "&^" => EXPR_ANDNOTEXPR,
        _ => EXPR_ADDEXPR,
    }
}

/// Extract a comment node.
fn extract_comment(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    comment_group_idx: &mut i64,
) {
    let text = node.text(source);
    let kind = if text.starts_with("//") { COMMENT_LINE } else { COMMENT_BLOCK };

    let group_id = emitter.alloc();
    let group_loc = LocationEmitter::emit_for_node(emitter, file_id, node);
    emitter.emit("comment_groups", vec![
        Value::Entity(group_id),
        Value::Entity(file_id),
        Value::Int(*comment_group_idx),
    ]);
    emitter.emit("has_location", vec![
        Value::Entity(group_id),
        Value::Entity(group_loc),
    ]);

    let comment_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    let text_val = emitter.string(text);
    emitter.emit("comments", vec![
        Value::Entity(comment_id),
        Value::Int(kind),
        Value::Entity(group_id),
        Value::Int(0),
        text_val,
    ]);
    emitter.emit("has_location", vec![
        Value::Entity(comment_id),
        Value::Entity(loc_id),
    ]);

    *comment_group_idx += 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocql_database::Database;
    use crate::schema::go_schema;

    fn print_tree(node: &Node, source: &[u8], indent: usize) {
        let kind = node.kind();
        let is_named = node.is_named();
        let text = if node.child_count() == 0 {
            format!(" {:?}", node.utf8_text(source).unwrap_or(""))
        } else {
            String::new()
        };
        eprintln!("{}{}{} [{}..{}]{}",
            " ".repeat(indent),
            if is_named { "" } else { "(" },
            kind,
            node.start_position().row,
            node.end_position().row,
            text
        );
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                print_tree(&cursor.node(), source, indent + 2);
                if !cursor.goto_next_sibling() { break; }
            }
        }
    }

    #[test]
    fn test_debug_tree() {
        let source = std::fs::read("tests/fixtures/simple.go").unwrap();
        let mut parser = tree_sitter::Parser::new();
        let lang: tree_sitter::Language = tree_sitter_go::LANGUAGE.into();
        parser.set_language(&lang).unwrap();
        let tree = parser.parse(&source, None).unwrap();
        print_tree(&tree.root_node(), &source, 0);
    }

    fn extract_test_file(filename: &str) -> Database {
        let source = std::fs::read(
            format!("tests/fixtures/{}", filename)
        ).unwrap();
        let schema = go_schema();
        let mut db = Database::from_schema(schema);
        let extractor = GoExtractor::new();
        let result = extractor.extract_source(&mut db, filename, &source);
        assert!(result.success, "Extraction failed for {}: {:?}", filename, result.error);
        db
    }

    #[test]
    fn test_simple_packages() {
        let db = extract_test_file("simple.go");
        let packages: Vec<_> = db.scan("packages").unwrap().collect();
        let names: Vec<_> = packages.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Packages: {:?}", names);
        assert!(names.contains(&"main"), "Should find 'main' package");
    }

    #[test]
    fn test_simple_decls() {
        let db = extract_test_file("simple.go");
        let decls: Vec<_> = db.scan("decls").unwrap().collect();
        let kinds: Vec<i64> = decls.iter().map(|t| t[1].as_int().unwrap()).collect();
        eprintln!("Decl kinds: {:?}", kinds);
        assert!(kinds.contains(&DECL_IMPORT), "Should have import declarations");
        assert!(kinds.contains(&DECL_FUNC), "Should have function declarations");
        assert!(kinds.contains(&DECL_TYPE), "Should have type declarations");
        assert!(kinds.contains(&DECL_VAR), "Should have var declarations");
        assert!(kinds.contains(&DECL_CONST), "Should have const declarations");
    }

    #[test]
    fn test_simple_functions() {
        let db = extract_test_file("simple.go");
        let objects: Vec<_> = db.scan("objects").unwrap().collect();
        let func_names: Vec<_> = objects.iter()
            .filter(|t| t[1].as_int().unwrap() == OBJ_DECLFUNC)
            .map(|t| db.strings.resolve(t[2].as_string().unwrap()).to_string())
            .collect();
        eprintln!("Function objects: {:?}", func_names);
        assert!(func_names.contains(&"main".to_string()), "Should find 'main' function");
        assert!(func_names.contains(&"add".to_string()), "Should find 'add' function");
        assert!(func_names.contains(&"String".to_string()), "Should find 'String' method");
    }

    #[test]
    fn test_simple_types() {
        let db = extract_test_file("simple.go");
        let objects: Vec<_> = db.scan("objects").unwrap().collect();
        let type_names: Vec<_> = objects.iter()
            .filter(|t| t[1].as_int().unwrap() == OBJ_DECLTYPE)
            .map(|t| db.strings.resolve(t[2].as_string().unwrap()).to_string())
            .collect();
        eprintln!("Type objects: {:?}", type_names);
        assert!(type_names.contains(&"Point".to_string()), "Should find 'Point' type");
        assert!(type_names.contains(&"Shape".to_string()), "Should find 'Shape' type");
    }

    #[test]
    fn test_simple_statements() {
        let db = extract_test_file("simple.go");
        let stmts: Vec<_> = db.scan("stmts").unwrap().collect();
        let kinds: Vec<i64> = stmts.iter().map(|t| t[1].as_int().unwrap()).collect();
        eprintln!("Statement kinds: {:?}", kinds);
        assert!(kinds.contains(&STMT_BLOCK), "Should have block statements");
        assert!(kinds.contains(&STMT_RETURN), "Should have return statements");
        assert!(kinds.contains(&STMT_IF), "Should have if statements");
        assert!(kinds.contains(&STMT_FOR), "Should have for statements");
        assert!(kinds.contains(&STMT_RANGE), "Should have range statements");
    }

    #[test]
    fn test_simple_expressions() {
        let db = extract_test_file("simple.go");
        let exprs: Vec<_> = db.scan("exprs").unwrap().collect();
        let kinds: Vec<i64> = exprs.iter().map(|t| t[1].as_int().unwrap()).collect();
        eprintln!("Expression kinds: {:?}", kinds);
        assert!(kinds.contains(&EXPR_IDENT), "Should have identifiers");
        assert!(kinds.contains(&EXPR_CALLEXPR), "Should have call expressions");
        assert!(kinds.contains(&EXPR_ADDEXPR), "Should have add expressions");
        assert!(kinds.contains(&EXPR_INTLIT), "Should have int literals");
        assert!(kinds.contains(&EXPR_STRINGLIT), "Should have string literals");
    }

    #[test]
    fn test_simple_literals() {
        let db = extract_test_file("simple.go");
        let literals: Vec<_> = db.scan("literals").unwrap().collect();
        let values: Vec<_> = literals.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap()).to_string()
        }).collect();
        eprintln!("Literals: {:?}", values);
        assert!(!literals.is_empty(), "Should have some literals");
    }

    #[test]
    fn test_simple_fields() {
        let db = extract_test_file("simple.go");
        let fields: Vec<_> = db.scan("fields").unwrap().collect();
        eprintln!("Fields: {} entries", fields.len());
        assert!(fields.len() >= 2, "Should have at least 2 fields (struct fields + params)");
    }

    #[test]
    fn test_simple_specs() {
        let db = extract_test_file("simple.go");
        let specs: Vec<_> = db.scan("specs").unwrap().collect();
        let kinds: Vec<i64> = specs.iter().map(|t| t[1].as_int().unwrap()).collect();
        eprintln!("Spec kinds: {:?}", kinds);
        assert!(kinds.contains(&SPEC_IMPORT), "Should have import specs");
        assert!(kinds.contains(&SPEC_VALUE), "Should have value specs");
        assert!(kinds.contains(&SPEC_TYPEDEF), "Should have type specs");
    }

    #[test]
    fn test_simple_comments() {
        let db = extract_test_file("simple.go");
        let comments: Vec<_> = db.scan("comments").unwrap().collect();
        let texts: Vec<_> = comments.iter().map(|t| {
            db.strings.resolve(t[4].as_string().unwrap()).to_string()
        }).collect();
        eprintln!("Comments: {:?}", texts);
        assert!(!comments.is_empty(), "Should have at least one comment");
    }

    #[test]
    fn test_simple_locations() {
        let db = extract_test_file("simple.go");
        let locs: Vec<_> = db.scan("locations_default").unwrap().collect();
        eprintln!("Locations: {} entries", locs.len());
        assert!(locs.len() >= 10, "Should have at least 10 location entries");

        let has_locs: Vec<_> = db.scan("has_location").unwrap().collect();
        eprintln!("has_location: {} entries", has_locs.len());
        assert!(has_locs.len() >= 10, "Should have at least 10 has_location entries");
    }

    #[test]
    fn test_simple_switch_select() {
        let db = extract_test_file("simple.go");
        let stmts: Vec<_> = db.scan("stmts").unwrap().collect();
        let kinds: Vec<i64> = stmts.iter().map(|t| t[1].as_int().unwrap()).collect();
        assert!(kinds.contains(&STMT_EXPRSWITCH), "Should have switch statements");
        assert!(kinds.contains(&STMT_DEFINE), "Should have short var declarations");
    }

    #[test]
    fn test_simple_go_defer() {
        let db = extract_test_file("simple.go");
        let stmts: Vec<_> = db.scan("stmts").unwrap().collect();
        let kinds: Vec<i64> = stmts.iter().map(|t| t[1].as_int().unwrap()).collect();
        assert!(kinds.contains(&STMT_GO), "Should have go statements");
        assert!(kinds.contains(&STMT_DEFER), "Should have defer statements");
    }

    #[test]
    fn test_simple_assign() {
        let db = extract_test_file("simple.go");
        let stmts: Vec<_> = db.scan("stmts").unwrap().collect();
        let kinds: Vec<i64> = stmts.iter().map(|t| t[1].as_int().unwrap()).collect();
        assert!(kinds.contains(&STMT_ASSIGN), "Should have assignment statements");
    }

    #[test]
    fn test_simple_binary_ops() {
        let db = extract_test_file("simple.go");
        let exprs: Vec<_> = db.scan("exprs").unwrap().collect();
        let kinds: Vec<i64> = exprs.iter().map(|t| t[1].as_int().unwrap()).collect();
        assert!(kinds.contains(&EXPR_LSSEXPR), "Should have less-than comparisons");
    }

    #[test]
    fn test_simple_composite_lit() {
        let db = extract_test_file("simple.go");
        let exprs: Vec<_> = db.scan("exprs").unwrap().collect();
        let kinds: Vec<i64> = exprs.iter().map(|t| t[1].as_int().unwrap()).collect();
        assert!(kinds.contains(&EXPR_COMPOSITELIT), "Should have composite literals");
    }
}
