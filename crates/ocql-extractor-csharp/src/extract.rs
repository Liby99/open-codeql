use ocql_database::{EntityId, Value};
use ocql_extractor_common::{Extractor, FactEmitter, LocationEmitter, NodeExt};
use ocql_extractor_common::tree_sitter::{Language, Node, Tree};

/// C# extractor using tree-sitter.
///
/// Extracts:
/// - Namespaces, using directives
/// - Classes, structs, interfaces, enums, records, delegates
/// - Methods, constructors, properties, fields, events
/// - Parameters, local variables
/// - Statements and expressions
/// - Attributes, modifiers
/// - Type parameters (generics)
/// - Inheritance (extends/implements)
/// - Comments
pub struct CSharpExtractor;

impl CSharpExtractor {
    pub fn new() -> Self {
        Self
    }
}

// Statement kind constants
const STMT_BLOCK: i64 = 0;
const STMT_IF: i64 = 1;
const STMT_FOR: i64 = 2;
const STMT_FOREACH: i64 = 3;
const STMT_WHILE: i64 = 4;
const STMT_DO: i64 = 5;
const STMT_SWITCH: i64 = 6;
const STMT_RETURN: i64 = 7;
const STMT_THROW: i64 = 8;
const STMT_BREAK: i64 = 9;
const STMT_CONTINUE: i64 = 10;
const STMT_TRY: i64 = 11;
const STMT_USING: i64 = 12;
const STMT_LOCK: i64 = 13;
const STMT_YIELD: i64 = 14;
const STMT_EXPR: i64 = 15;
const STMT_LOCAL_DECL: i64 = 16;
const STMT_GOTO: i64 = 17;
const STMT_CHECKED: i64 = 18;
const STMT_UNCHECKED: i64 = 19;
const STMT_FIXED: i64 = 20;
const STMT_UNSAFE: i64 = 21;
const STMT_CATCH: i64 = 22;
const STMT_FINALLY: i64 = 23;

// Expression kind constants
const EXPR_INT_LIT: i64 = 1;
const EXPR_REAL_LIT: i64 = 2;
const EXPR_STRING_LIT: i64 = 3;
const EXPR_CHAR_LIT: i64 = 4;
const EXPR_BOOL_LIT: i64 = 5;
const EXPR_NULL_LIT: i64 = 6;
const EXPR_BINARY: i64 = 7;
const EXPR_PREFIX_UNARY: i64 = 8;
const EXPR_POSTFIX_UNARY: i64 = 9;
const EXPR_ASSIGN: i64 = 10;
const EXPR_CALL: i64 = 11;
const EXPR_MEMBER_ACCESS: i64 = 12;
const EXPR_ELEMENT_ACCESS: i64 = 13;
const EXPR_CAST: i64 = 14;
const EXPR_NEW: i64 = 15;
const EXPR_TYPEOF: i64 = 16;
const EXPR_SIZEOF: i64 = 17;
const EXPR_NAMEOF: i64 = 18;
const EXPR_IS: i64 = 19;
const EXPR_AS: i64 = 20;
const EXPR_NULL_COALESCING: i64 = 21;
const EXPR_CONDITIONAL: i64 = 22;
const EXPR_AWAIT: i64 = 23;
const EXPR_THROW: i64 = 24;
const EXPR_LAMBDA: i64 = 25;
const EXPR_INTERPOLATED_STRING: i64 = 26;
const EXPR_IDENTIFIER: i64 = 27;
const EXPR_THIS: i64 = 28;
const EXPR_BASE: i64 = 29;
const EXPR_INITIALIZER: i64 = 30;
const EXPR_DEFAULT: i64 = 31;
const EXPR_CHECKED: i64 = 32;
const EXPR_UNCHECKED: i64 = 33;
const EXPR_STACKALLOC: i64 = 34;
const EXPR_SWITCH: i64 = 35;
const EXPR_WITH: i64 = 36;
const EXPR_RANGE: i64 = 37;
const EXPR_TUPLE: i64 = 38;
const EXPR_PATTERN: i64 = 39;

// Type kind constants (matches CodeQL C# dbscheme)
const TYPE_CLASS: i64 = 17;
const TYPE_STRUCT: i64 = 15;
const TYPE_INTERFACE: i64 = 19;
const TYPE_ENUM: i64 = 14;
const TYPE_DELEGATE: i64 = 20;
const TYPE_RECORD: i64 = 17; // records are class-like

// Using directive kind constants
const USING_NAMESPACE: i64 = 0;
const USING_STATIC: i64 = 1;
const USING_ALIAS: i64 = 2;

impl Extractor for CSharpExtractor {
    fn language(&self) -> Language {
        tree_sitter_c_sharp::LANGUAGE.into()
    }

    fn extensions(&self) -> &[&str] {
        &["cs"]
    }

    fn extract_file(
        &self,
        emitter: &mut FactEmitter<'_>,
        file_id: EntityId,
        tree: &Tree,
        source: &[u8],
    ) {
        let root = tree.root_node();
        extract_compilation_unit(emitter, file_id, &root, source);
    }
}

/// Extract the top-level compilation_unit node.
fn extract_compilation_unit(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    root: &Node<'_>,
    source: &[u8],
) {
    let mut cursor = root.walk();
    if cursor.goto_first_child() {
        loop {
            let node = cursor.node();
            if node.is_named() {
                extract_top_level(emitter, file_id, &node, source, None);
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// Extract a top-level declaration.
/// `enclosing_type` is Some when extracting nested types inside a type body.
fn extract_top_level(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    enclosing_type: Option<EntityId>,
) {
    match node.kind() {
        "namespace_declaration" | "file_scoped_namespace_declaration" => {
            extract_namespace(emitter, file_id, node, source);
        }
        "using_directive" => {
            extract_using_directive(emitter, file_id, node, source);
        }
        "class_declaration" => {
            extract_type_decl(emitter, file_id, node, source, TYPE_CLASS, enclosing_type);
        }
        "struct_declaration" => {
            extract_type_decl(emitter, file_id, node, source, TYPE_STRUCT, enclosing_type);
        }
        "interface_declaration" => {
            extract_type_decl(emitter, file_id, node, source, TYPE_INTERFACE, enclosing_type);
        }
        "enum_declaration" => {
            extract_enum(emitter, file_id, node, source, enclosing_type);
        }
        "record_declaration" | "record_struct_declaration" => {
            extract_type_decl(emitter, file_id, node, source, TYPE_RECORD, enclosing_type);
        }
        "delegate_declaration" => {
            extract_delegate(emitter, file_id, node, source, enclosing_type);
        }
        "comment" => {
            extract_comment(emitter, file_id, node, source);
        }
        "global_statement" => {
            // Top-level statements (C# 9+): extract child statements
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        let dummy_parent = emitter.alloc();
                        extract_stmt(emitter, file_id, &child, source, dummy_parent, 0);
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        _ => {}
    }
}

/// Extract a namespace declaration.
fn extract_namespace(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    if name.is_empty() { return; }

    let ns_id = emitter.alloc();
    let ns_decl_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let name_val = emitter.string(&name);
    emitter.emit("namespaces", vec![
        Value::Entity(ns_id),
        name_val,
    ]);
    emitter.emit("namespace_declarations", vec![
        Value::Entity(ns_decl_id),
        Value::Entity(ns_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(ns_decl_id),
        Value::Entity(loc_id),
    ]);

    // Extract body members
    if let Some(body) = node.child_by_field("body") {
        let mut cursor = body.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.is_named() {
                    extract_top_level(emitter, file_id, &child, source, None);
                }
                if !cursor.goto_next_sibling() { break; }
            }
        }
    }

    // For file-scoped namespaces, members are siblings, not inside a body
    if node.kind() == "file_scoped_namespace_declaration" {
        // Members after the semicolon are handled by the parent compilation_unit traversal
    }
}

/// Extract a using directive.
fn extract_using_directive(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let text = node.text(source);

    let (name, kind) = if text.contains(" static ") {
        // using static SomeType;
        let name = extract_using_name(node, source);
        (name, USING_STATIC)
    } else if text.contains(" = ") || text.contains("=") {
        // using Alias = SomeType;
        let name = node.text(source)
            .trim_start_matches("using ")
            .trim_start_matches("global ")
            .trim_end_matches(';')
            .trim()
            .to_string();
        (name, USING_ALIAS)
    } else {
        // using SomeNamespace;
        let name = extract_using_name(node, source);
        (name, USING_NAMESPACE)
    };

    let using_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    let name_val = emitter.string(&name);
    emitter.emit("using_directives", vec![
        Value::Entity(using_id),
        name_val,
        Value::Int(kind),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(using_id),
        Value::Entity(loc_id),
    ]);
}

/// Extract the name from a using directive node.
fn extract_using_name(node: &Node<'_>, source: &[u8]) -> String {
    // Look for qualified_name, identifier, or name field
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "qualified_name" | "identifier" | "name" => {
                    return child.text(source).to_string();
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
    // Fallback: parse from text
    node.text(source)
        .trim_start_matches("using ")
        .trim_start_matches("global ")
        .trim_start_matches("static ")
        .trim_end_matches(';')
        .trim()
        .to_string()
}

/// Extract a type declaration (class, struct, interface, record).
fn extract_type_decl(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    type_kind: i64,
    enclosing_type: Option<EntityId>,
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    if name.is_empty() { return; }

    let type_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let parent_id = enclosing_type.unwrap_or_else(|| emitter.alloc());
    let name_val = emitter.string(&name);
    emitter.emit("types", vec![
        Value::Entity(type_id),
        Value::Int(type_kind),
        name_val,
        Value::Entity(parent_id),
    ]);
    emitter.emit("type_location", vec![
        Value::Entity(type_id),
        Value::Entity(loc_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(type_id),
        Value::Entity(loc_id),
    ]);

    // Extract modifiers
    extract_modifiers(emitter, node, source, type_id);

    // Extract attributes
    extract_attributes(emitter, file_id, node, source, type_id);

    // Extract base types (inheritance) — base_list is a direct child, not a field
    if let Some(bases) = find_child_by_kind(node, "base_list") {
        extract_base_list(emitter, file_id, type_id, &bases, source, type_kind);
    }

    // Extract type parameters — type_parameter_list is a direct child, not a field
    if let Some(type_params) = find_child_by_kind(node, "type_parameter_list") {
        extract_type_parameters(emitter, file_id, type_id, &type_params, source);
    }

    // Extract body
    if let Some(body) = node.child_by_field("body") {
        extract_type_body(emitter, file_id, type_id, &body, source);
    }
}

/// Extract an enum declaration.
fn extract_enum(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    enclosing_type: Option<EntityId>,
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    if name.is_empty() { return; }

    let type_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let parent_id = enclosing_type.unwrap_or_else(|| emitter.alloc());
    let name_val = emitter.string(&name);
    emitter.emit("types", vec![
        Value::Entity(type_id),
        Value::Int(TYPE_ENUM),
        name_val,
        Value::Entity(parent_id),
    ]);
    emitter.emit("type_location", vec![
        Value::Entity(type_id),
        Value::Entity(loc_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(type_id),
        Value::Entity(loc_id),
    ]);

    extract_modifiers(emitter, node, source, type_id);
    extract_attributes(emitter, file_id, node, source, type_id);

    // Extract enum members as fields
    if let Some(body) = node.child_by_field("body") {
        let mut cursor = body.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "enum_member_declaration" {
                    let member_name = child.child_by_field("name")
                        .map(|n| n.text(source).to_string())
                        .unwrap_or_default();
                    if !member_name.is_empty() {
                        let field_id = emitter.alloc();
                        let field_loc = LocationEmitter::emit_for_node(emitter, file_id, &child);
                        let member_name_val = emitter.string(&member_name);
                        let type_val = emitter.string(&name);
                        emitter.emit("fields", vec![
                            Value::Entity(field_id),
                            member_name_val,
                            type_val,
                            Value::Entity(type_id),
                        ]);
                        emitter.emit("hasLocation", vec![
                            Value::Entity(field_id),
                            Value::Entity(field_loc),
                        ]);
                    }
                }
                if !cursor.goto_next_sibling() { break; }
            }
        }
    }
}

/// Extract a delegate declaration.
fn extract_delegate(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    enclosing_type: Option<EntityId>,
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    if name.is_empty() { return; }

    let type_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let parent_id = enclosing_type.unwrap_or_else(|| emitter.alloc());
    let name_val = emitter.string(&name);
    emitter.emit("types", vec![
        Value::Entity(type_id),
        Value::Int(TYPE_DELEGATE),
        name_val,
        Value::Entity(parent_id),
    ]);
    emitter.emit("type_location", vec![
        Value::Entity(type_id),
        Value::Entity(loc_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(type_id),
        Value::Entity(loc_id),
    ]);

    extract_modifiers(emitter, node, source, type_id);
    extract_attributes(emitter, file_id, node, source, type_id);

    // Extract type parameters — type_parameter_list is a direct child, not a field
    if let Some(type_params) = find_child_by_kind(node, "type_parameter_list") {
        extract_type_parameters(emitter, file_id, type_id, &type_params, source);
    }
}

/// Extract base list (extends/implements).
fn extract_base_list(
    emitter: &mut FactEmitter<'_>,
    _file_id: EntityId,
    type_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    type_kind: i64,
) {
    let mut first = true;
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            // Base types are identifier, qualified_name, generic_name nodes inside base_list
            if child.is_named() && child.kind() != ":" && child.kind() != "," {
                let base_name = child.text(source).to_string();
                if !base_name.is_empty() && base_name != ":" && base_name != "," {
                    let base_id = emitter.alloc();
                    let dummy_parent = emitter.alloc();
                    let name_val = emitter.string(&base_name);
                    // Create a type entry for the base type
                    emitter.emit("types", vec![
                        Value::Entity(base_id),
                        Value::Int(TYPE_INTERFACE), // assume interface by default
                        name_val,
                        Value::Entity(dummy_parent),
                    ]);

                    // For classes, first base type could be a class (extends), rest are interfaces
                    if first && (type_kind == TYPE_CLASS || type_kind == TYPE_RECORD) {
                        emitter.emit("extends", vec![
                            Value::Entity(type_id),
                            Value::Entity(base_id),
                        ]);
                        first = false;
                    } else {
                        emitter.emit("implements", vec![
                            Value::Entity(type_id),
                            Value::Entity(base_id),
                        ]);
                    }
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract the body of a type (class, struct, interface).
fn extract_type_body(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    type_id: EntityId,
    body: &Node<'_>,
    source: &[u8],
) {
    let mut cursor = body.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "method_declaration" => {
                    extract_method(emitter, file_id, type_id, &child, source);
                }
                "constructor_declaration" => {
                    extract_constructor(emitter, file_id, type_id, &child, source);
                }
                "property_declaration" => {
                    extract_property(emitter, file_id, type_id, &child, source);
                }
                "field_declaration" => {
                    extract_field(emitter, file_id, type_id, &child, source);
                }
                "event_declaration" | "event_field_declaration" => {
                    extract_event(emitter, file_id, type_id, &child, source);
                }
                "indexer_declaration" => {
                    extract_property(emitter, file_id, type_id, &child, source);
                }
                "operator_declaration" | "conversion_operator_declaration" => {
                    extract_method(emitter, file_id, type_id, &child, source);
                }
                "destructor_declaration" => {
                    extract_method(emitter, file_id, type_id, &child, source);
                }
                // Nested types
                "class_declaration" => {
                    extract_type_decl(emitter, file_id, &child, source, TYPE_CLASS, Some(type_id));
                }
                "struct_declaration" => {
                    extract_type_decl(emitter, file_id, &child, source, TYPE_STRUCT, Some(type_id));
                }
                "interface_declaration" => {
                    extract_type_decl(emitter, file_id, &child, source, TYPE_INTERFACE, Some(type_id));
                }
                "enum_declaration" => {
                    extract_enum(emitter, file_id, &child, source, Some(type_id));
                }
                "record_declaration" | "record_struct_declaration" => {
                    extract_type_decl(emitter, file_id, &child, source, TYPE_RECORD, Some(type_id));
                }
                "delegate_declaration" => {
                    extract_delegate(emitter, file_id, &child, source, Some(type_id));
                }
                "comment" => {
                    extract_comment(emitter, file_id, &child, source);
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract a method declaration.
fn extract_method(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    type_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_else(|| {
            // Operators and destructors may not have a "name" field
            // For operators, try to get the operator token
            if node.kind() == "operator_declaration" || node.kind() == "conversion_operator_declaration" {
                "operator".to_string()
            } else if node.kind() == "destructor_declaration" {
                "~destructor".to_string()
            } else {
                String::new()
            }
        });
    if name.is_empty() { return; }

    let return_type = node.child_by_field("type")
        .or_else(|| node.child_by_field("returns"))
        .map(|t| t.text(source).to_string())
        .unwrap_or_else(|| "void".to_string());

    let method_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let sig = build_method_signature(&name, node, source);

    let name_val = emitter.string(&name);
    let sig_val = emitter.string(&sig);
    let ret_val = emitter.string(&return_type);
    emitter.emit("methods", vec![
        Value::Entity(method_id),
        name_val,
        sig_val,
        ret_val,
        Value::Entity(type_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(method_id),
        Value::Entity(loc_id),
    ]);

    extract_modifiers(emitter, node, source, method_id);
    extract_attributes(emitter, file_id, node, source, method_id);

    // Type parameters — type_parameter_list is a direct child, not a field
    if let Some(type_params) = find_child_by_kind(node, "type_parameter_list") {
        extract_type_parameters(emitter, file_id, method_id, &type_params, source);
    }

    // Parameters
    if let Some(params) = node.child_by_field("parameters") {
        extract_parameters(emitter, file_id, method_id, &params, source);
    }

    // Body (block or expression body)
    if let Some(body) = node.child_by_field("body") {
        extract_method_body(emitter, file_id, method_id, &body, source);
    }
}

/// Extract a constructor declaration.
fn extract_constructor(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    type_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    if name.is_empty() { return; }

    let constr_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let name_val = emitter.string(&name);
    emitter.emit("constructors", vec![
        Value::Entity(constr_id),
        name_val,
        Value::Entity(type_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(constr_id),
        Value::Entity(loc_id),
    ]);

    extract_modifiers(emitter, node, source, constr_id);
    extract_attributes(emitter, file_id, node, source, constr_id);

    if let Some(params) = node.child_by_field("parameters") {
        extract_parameters(emitter, file_id, constr_id, &params, source);
    }

    if let Some(body) = node.child_by_field("body") {
        extract_method_body(emitter, file_id, constr_id, &body, source);
    }
}

/// Extract a property declaration.
fn extract_property(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    type_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_else(|| {
            // Indexer: use "this[]"
            if node.kind() == "indexer_declaration" {
                "this[]".to_string()
            } else {
                String::new()
            }
        });
    if name.is_empty() { return; }

    let type_text = node.child_by_field("type")
        .map(|t| t.text(source).to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let prop_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let name_val = emitter.string(&name);
    let type_val = emitter.string(&type_text);
    emitter.emit("properties", vec![
        Value::Entity(prop_id),
        name_val,
        type_val,
        Value::Entity(type_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(prop_id),
        Value::Entity(loc_id),
    ]);

    extract_modifiers(emitter, node, source, prop_id);
    extract_attributes(emitter, file_id, node, source, prop_id);
}

/// Extract a field declaration.
fn extract_field(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    type_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    // Get the type from variable_declaration child
    let var_decl = find_child_by_kind(node, "variable_declaration");
    let type_text = var_decl.as_ref()
        .and_then(|vd| vd.child_by_field("type"))
        .map(|t| t.text(source).to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Get declarators
    if let Some(vd) = var_decl {
        let mut cursor = vd.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "variable_declarator" {
                    let name = child.child_by_field("name")
                        .or_else(|| child.child(0).filter(|c| c.kind() == "identifier"))
                        .map(|n| n.text(source).to_string())
                        .unwrap_or_default();
                    if !name.is_empty() {
                        let field_id = emitter.alloc();
                        let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                        let name_val = emitter.string(&name);
                        let type_val = emitter.string(&type_text);
                        emitter.emit("fields", vec![
                            Value::Entity(field_id),
                            name_val,
                            type_val,
                            Value::Entity(type_id),
                        ]);
                        emitter.emit("hasLocation", vec![
                            Value::Entity(field_id),
                            Value::Entity(loc_id),
                        ]);
                        extract_modifiers(emitter, node, source, field_id);
                        extract_attributes(emitter, file_id, node, source, field_id);
                    }
                }
                if !cursor.goto_next_sibling() { break; }
            }
        }
    }
}

/// Extract an event declaration.
fn extract_event(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    type_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    // For event_field_declaration, the structure is similar to field_declaration
    // For event_declaration, there's a name and type
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();

    let type_text = node.child_by_field("type")
        .map(|t| t.text(source).to_string())
        .unwrap_or_else(|| "unknown".to_string());

    if !name.is_empty() {
        let event_id = emitter.alloc();
        let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
        let name_val = emitter.string(&name);
        let type_val = emitter.string(&type_text);
        emitter.emit("events", vec![
            Value::Entity(event_id),
            name_val,
            type_val,
            Value::Entity(type_id),
        ]);
        emitter.emit("hasLocation", vec![
            Value::Entity(event_id),
            Value::Entity(loc_id),
        ]);
        extract_modifiers(emitter, node, source, event_id);
        return;
    }

    // For event_field_declaration, look for variable_declaration
    if let Some(vd) = find_child_by_kind(node, "variable_declaration") {
        let evt_type = vd.child_by_field("type")
            .map(|t| t.text(source).to_string())
            .unwrap_or_else(|| type_text.clone());
        let mut cursor = vd.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "variable_declarator" {
                    let decl_name = child.child_by_field("name")
                        .or_else(|| child.child(0).filter(|c| c.kind() == "identifier"))
                        .map(|n| n.text(source).to_string())
                        .unwrap_or_default();
                    if !decl_name.is_empty() {
                        let event_id = emitter.alloc();
                        let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                        let name_val = emitter.string(&decl_name);
                        let type_val = emitter.string(&evt_type);
                        emitter.emit("events", vec![
                            Value::Entity(event_id),
                            name_val,
                            type_val,
                            Value::Entity(type_id),
                        ]);
                        emitter.emit("hasLocation", vec![
                            Value::Entity(event_id),
                            Value::Entity(loc_id),
                        ]);
                        extract_modifiers(emitter, node, source, event_id);
                    }
                }
                if !cursor.goto_next_sibling() { break; }
            }
        }
    }
}

/// Build a method signature string like "MethodName(int, string)".
fn build_method_signature(name: &str, node: &Node<'_>, source: &[u8]) -> String {
    let mut param_types = Vec::new();
    if let Some(params) = node.child_by_field("parameters") {
        let mut cursor = params.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "parameter" {
                    if let Some(type_node) = child.child_by_field("type") {
                        param_types.push(type_node.text(source).to_string());
                    }
                }
                if !cursor.goto_next_sibling() { break; }
            }
        }
    }
    format!("{}({})", name, param_types.join(", "))
}

/// Extract parameters from a parameter_list node.
fn extract_parameters(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    callable_id: EntityId,
    params: &Node<'_>,
    source: &[u8],
) {
    let mut index = 0i64;
    let mut cursor = params.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "parameter" {
                let type_text = child.child_by_field("type")
                    .map(|t| t.text(source).to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                let name = child.child_by_field("name")
                    .map(|n| n.text(source).to_string())
                    .unwrap_or_default();

                if !name.is_empty() {
                    let param_id = emitter.alloc();
                    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                    let name_val = emitter.string(&name);
                    let type_val = emitter.string(&type_text);
                    emitter.emit("params", vec![
                        Value::Entity(param_id),
                        name_val,
                        type_val,
                        Value::Int(index),
                        Value::Entity(callable_id),
                    ]);
                    emitter.emit("hasLocation", vec![
                        Value::Entity(param_id),
                        Value::Entity(loc_id),
                    ]);

                    index += 1;
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract modifiers from a declaration node.
fn extract_modifiers(
    emitter: &mut FactEmitter<'_>,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "modifier" {
                let mod_text = child.text(source);
                if is_modifier_text(mod_text) {
                    let mod_id = emitter.alloc();
                    let name_val = emitter.string(mod_text);
                    emitter.emit("modifiers", vec![
                        Value::Entity(mod_id),
                        name_val,
                    ]);
                    emitter.emit("hasModifier", vec![
                        Value::Entity(parent_id),
                        Value::Entity(mod_id),
                    ]);
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

fn is_modifier_text(text: &str) -> bool {
    matches!(text,
        "public" | "private" | "protected" | "internal"
        | "static" | "virtual" | "override" | "abstract"
        | "sealed" | "async" | "readonly" | "volatile"
        | "extern" | "unsafe" | "new" | "partial"
        | "const" | "ref" | "out" | "in"
        | "required" | "file" | "scoped"
    )
}

/// Extract attributes from a declaration node.
fn extract_attributes(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "attribute_list" {
                let mut inner = child.walk();
                if inner.goto_first_child() {
                    loop {
                        let attr = inner.node();
                        if attr.kind() == "attribute" {
                            let attr_name = attr.child_by_field("name")
                                .map(|n| n.text(source).to_string())
                                .unwrap_or_else(|| attr.text(source).to_string());
                            let attr_id = emitter.alloc();
                            let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &attr);
                            let name_val = emitter.string(&attr_name);
                            emitter.emit("attributes", vec![
                                Value::Entity(attr_id),
                                Value::Entity(parent_id),
                                name_val,
                            ]);
                            emitter.emit("hasLocation", vec![
                                Value::Entity(attr_id),
                                Value::Entity(loc_id),
                            ]);
                        }
                        if !inner.goto_next_sibling() { break; }
                    }
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract type parameters from a type_parameter_list node.
fn extract_type_parameters(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    parent_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let mut index = 0i64;
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "type_parameter" {
                let name = child.text(source).to_string();
                if !name.is_empty() && name != "<" && name != ">" && name != "," {
                    let tparam_id = emitter.alloc();
                    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                    let name_val = emitter.string(&name);
                    emitter.emit("type_parameters", vec![
                        Value::Entity(tparam_id),
                        name_val,
                        Value::Int(index),
                        Value::Entity(parent_id),
                    ]);
                    emitter.emit("hasLocation", vec![
                        Value::Entity(tparam_id),
                        Value::Entity(loc_id),
                    ]);
                    index += 1;
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract a method/constructor body (block of statements).
fn extract_method_body(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    callable_id: EntityId,
    body: &Node<'_>,
    source: &[u8],
) {
    if body.kind() == "block" {
        extract_stmt(emitter, file_id, body, source, callable_id, 0);
    } else if body.kind() == "arrow_expression_clause" {
        // Expression body: => expr;
        let mut cursor = body.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.is_named() {
                    extract_expr(emitter, file_id, &child, source, callable_id, 0);
                }
                if !cursor.goto_next_sibling() { break; }
            }
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
    let kind = match node.kind() {
        "block" => Some(STMT_BLOCK),
        "if_statement" => Some(STMT_IF),
        "for_statement" => Some(STMT_FOR),
        "for_each_statement" => Some(STMT_FOREACH),
        "while_statement" => Some(STMT_WHILE),
        "do_statement" => Some(STMT_DO),
        "switch_statement" => Some(STMT_SWITCH),
        "return_statement" => Some(STMT_RETURN),
        "throw_statement" => Some(STMT_THROW),
        "break_statement" => Some(STMT_BREAK),
        "continue_statement" => Some(STMT_CONTINUE),
        "try_statement" => Some(STMT_TRY),
        "using_statement" => Some(STMT_USING),
        "lock_statement" => Some(STMT_LOCK),
        "yield_statement" => Some(STMT_YIELD),
        "expression_statement" => Some(STMT_EXPR),
        "local_declaration_statement" => Some(STMT_LOCAL_DECL),
        "goto_statement" => Some(STMT_GOTO),
        "checked_statement" => Some(STMT_CHECKED),
        "unchecked_statement" => Some(STMT_UNCHECKED),
        "fixed_statement" => Some(STMT_FIXED),
        "unsafe_statement" => Some(STMT_UNSAFE),
        "catch_clause" => Some(STMT_CATCH),
        "finally_clause" => Some(STMT_FINALLY),
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
    emitter.emit("hasLocation", vec![
        Value::Entity(stmt_id),
        Value::Entity(loc_id),
    ]);

    // Process children based on statement type
    match node.kind() {
        "block" => {
            let mut child_index = 0i64;
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        if extract_stmt(emitter, file_id, &child, source, stmt_id, child_index).is_some() {
                            child_index += 1;
                        }
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
        "if_statement" => {
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, stmt_id, 0);
            }
            if let Some(consequence) = node.child_by_field("consequence") {
                extract_stmt(emitter, file_id, &consequence, source, stmt_id, 0);
            }
            if let Some(alternative) = node.child_by_field("alternative") {
                extract_stmt(emitter, file_id, &alternative, source, stmt_id, 1);
            }
        }
        "for_statement" => {
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, stmt_id, 0);
            }
            if let Some(body) = node.child_by_field("body") {
                extract_stmt(emitter, file_id, &body, source, stmt_id, 0);
            }
        }
        "for_each_statement" => {
            // Extract foreach variable
            if let Some(left) = node.child_by_field("left") {
                let type_text = node.child_by_field("type")
                    .map(|t| t.text(source).to_string())
                    .unwrap_or_else(|| "var".to_string());
                let name = left.text(source);
                if !name.is_empty() {
                    let var_id = emitter.alloc();
                    let name_val = emitter.string(name);
                    let type_val = emitter.string(&type_text);
                    emitter.emit("local_vars", vec![
                        Value::Entity(var_id),
                        name_val,
                        type_val,
                        Value::Entity(stmt_id),
                    ]);
                }
            }
            if let Some(right) = node.child_by_field("right") {
                extract_expr(emitter, file_id, &right, source, stmt_id, 0);
            }
            if let Some(body) = node.child_by_field("body") {
                extract_stmt(emitter, file_id, &body, source, stmt_id, 0);
            }
        }
        "while_statement" | "do_statement" => {
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, stmt_id, 0);
            }
            if let Some(body) = node.child_by_field("body") {
                extract_stmt(emitter, file_id, &body, source, stmt_id, 0);
            }
        }
        "return_statement" | "throw_statement" | "yield_statement" => {
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
        "local_declaration_statement" => {
            // Extract local variable declarations
            if let Some(vd) = find_child_by_kind(node, "variable_declaration") {
                let type_text = vd.child_by_field("type")
                    .map(|t| t.text(source).to_string())
                    .unwrap_or_else(|| "var".to_string());
                let mut cursor = vd.walk();
                if cursor.goto_first_child() {
                    loop {
                        let child = cursor.node();
                        if child.kind() == "variable_declarator" {
                            let name = child.child_by_field("name")
                                .or_else(|| child.child(0).filter(|c| c.kind() == "identifier"))
                                .map(|n| n.text(source).to_string())
                                .unwrap_or_default();
                            if !name.is_empty() {
                                let var_id = emitter.alloc();
                                let name_val = emitter.string(&name);
                                let type_val = emitter.string(&type_text);
                                emitter.emit("local_vars", vec![
                                    Value::Entity(var_id),
                                    name_val,
                                    type_val,
                                    Value::Entity(stmt_id),
                                ]);
                                let var_loc = LocationEmitter::emit_for_node(emitter, file_id, &child);
                                emitter.emit("hasLocation", vec![
                                    Value::Entity(var_id),
                                    Value::Entity(var_loc),
                                ]);

                                // Extract initializer expression
                                // In tree-sitter-c-sharp, the variable_declarator children are:
                                //   identifier (name), "=", <expression>
                                // We look for any named child that is not the name identifier.
                                extract_variable_initializer(emitter, file_id, &child, source, stmt_id);
                            }
                        }
                        if !cursor.goto_next_sibling() { break; }
                    }
                }
            }
        }
        "try_statement" => {
            if let Some(body) = node.child_by_field("body") {
                extract_stmt(emitter, file_id, &body, source, stmt_id, 0);
            }
            let mut catch_idx = 1i64;
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "catch_clause" {
                        extract_stmt(emitter, file_id, &child, source, stmt_id, catch_idx);
                        catch_idx += 1;
                    } else if child.kind() == "finally_clause" {
                        extract_stmt(emitter, file_id, &child, source, stmt_id, catch_idx);
                        catch_idx += 1;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "catch_clause" | "finally_clause" => {
            if let Some(body) = node.child_by_field("body") {
                extract_stmt(emitter, file_id, &body, source, stmt_id, 0);
            }
        }
        "using_statement" | "lock_statement" | "fixed_statement" | "unsafe_statement" => {
            if let Some(body) = node.child_by_field("body") {
                extract_stmt(emitter, file_id, &body, source, stmt_id, 0);
            }
        }
        "switch_statement" => {
            if let Some(value) = node.child_by_field("value") {
                extract_expr(emitter, file_id, &value, source, stmt_id, 0);
            }
            if let Some(body) = node.child_by_field("body") {
                let mut section_idx = 0i64;
                let mut cursor = body.walk();
                if cursor.goto_first_child() {
                    loop {
                        let child = cursor.node();
                        if child.kind() == "switch_section" {
                            // Extract statements inside each section
                            let mut inner = child.walk();
                            let mut stmt_idx = 0i64;
                            if inner.goto_first_child() {
                                loop {
                                    let sc = inner.node();
                                    if sc.is_named() && sc.kind() != "case_switch_label" && sc.kind() != "default_switch_label" {
                                        if extract_stmt(emitter, file_id, &sc, source, stmt_id, section_idx + stmt_idx).is_some() {
                                            stmt_idx += 1;
                                        }
                                    }
                                    if !inner.goto_next_sibling() { break; }
                                }
                            }
                            section_idx += 1;
                        }
                        if !cursor.goto_next_sibling() { break; }
                    }
                }
            }
        }
        "checked_statement" | "unchecked_statement" => {
            if let Some(body) = node.child_by_field("body") {
                extract_stmt(emitter, file_id, &body, source, stmt_id, 0);
            }
            // Also try block child
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "block" {
                        extract_stmt(emitter, file_id, &child, source, stmt_id, 0);
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        _ => {}
    }

    Some(stmt_id)
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
        "integer_literal" => Some(EXPR_INT_LIT),
        "real_literal" => Some(EXPR_REAL_LIT),
        "string_literal" | "verbatim_string_literal" | "raw_string_literal" => Some(EXPR_STRING_LIT),
        "character_literal" => Some(EXPR_CHAR_LIT),
        "boolean_literal" => Some(EXPR_BOOL_LIT),
        "null_literal" => Some(EXPR_NULL_LIT),
        "identifier" | "simple_name" => Some(EXPR_IDENTIFIER),
        "this_expression" => Some(EXPR_THIS),
        "base_expression" => Some(EXPR_BASE),
        "invocation_expression" => Some(EXPR_CALL),
        "member_access_expression" | "conditional_access_expression" => Some(EXPR_MEMBER_ACCESS),
        "element_access_expression" | "element_binding_expression" => Some(EXPR_ELEMENT_ACCESS),
        "object_creation_expression" | "implicit_object_creation_expression" => Some(EXPR_NEW),
        "cast_expression" => Some(EXPR_CAST),
        "binary_expression" => Some(EXPR_BINARY),
        "prefix_unary_expression" => Some(EXPR_PREFIX_UNARY),
        "postfix_unary_expression" => Some(EXPR_POSTFIX_UNARY),
        "assignment_expression" => Some(EXPR_ASSIGN),
        "conditional_expression" => Some(EXPR_CONDITIONAL),
        "is_expression" | "is_pattern_expression" => Some(EXPR_IS),
        "as_expression" => Some(EXPR_AS),
        "null_coalescing_expression" => Some(EXPR_NULL_COALESCING),
        "await_expression" => Some(EXPR_AWAIT),
        "throw_expression" => Some(EXPR_THROW),
        "lambda_expression" | "anonymous_method_expression" => Some(EXPR_LAMBDA),
        "interpolated_string_expression" => Some(EXPR_INTERPOLATED_STRING),
        "typeof_expression" => Some(EXPR_TYPEOF),
        "sizeof_expression" => Some(EXPR_SIZEOF),
        "nameof_expression" => Some(EXPR_NAMEOF),
        "initializer_expression" => Some(EXPR_INITIALIZER),
        "default_expression" => Some(EXPR_DEFAULT),
        "checked_expression" => Some(EXPR_CHECKED),
        "unchecked_expression" => Some(EXPR_UNCHECKED),
        "stackalloc_expression" => Some(EXPR_STACKALLOC),
        "switch_expression" => Some(EXPR_SWITCH),
        "with_expression" => Some(EXPR_WITH),
        "range_expression" => Some(EXPR_RANGE),
        "tuple_expression" => Some(EXPR_TUPLE),
        "declaration_expression" | "recursive_pattern" | "constant_pattern"
        | "relational_pattern" | "negation_pattern" | "and_pattern" | "or_pattern"
        | "type_pattern" | "var_pattern" | "discard" | "list_pattern" => Some(EXPR_PATTERN),
        "parenthesized_expression" => {
            // Unwrap parenthesized expression
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        return extract_expr(emitter, file_id, &child, source, parent_id, index);
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
            return None;
        }
        _ => None,
    };

    let expr_kind = match kind {
        Some(k) => k,
        None => return None,
    };

    let expr_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let type_val = emitter.string("unknown");
    emitter.emit("exprs", vec![
        Value::Entity(expr_id),
        Value::Int(expr_kind),
        type_val,
        Value::Entity(parent_id),
        Value::Int(index),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(expr_id),
        Value::Entity(loc_id),
    ]);

    // Recurse into children
    match node.kind() {
        "invocation_expression" => {
            if let Some(func) = node.child_by_field("function") {
                extract_expr(emitter, file_id, &func, source, expr_id, 0);
            }
            if let Some(args) = node.child_by_field("arguments") {
                extract_argument_list(emitter, file_id, &args, source, expr_id);
            }
        }
        "object_creation_expression" | "implicit_object_creation_expression" => {
            if let Some(args) = node.child_by_field("arguments") {
                extract_argument_list(emitter, file_id, &args, source, expr_id);
            }
            if let Some(init) = node.child_by_field("initializer") {
                extract_expr(emitter, file_id, &init, source, expr_id, 0);
            }
        }
        "member_access_expression" | "conditional_access_expression" => {
            if let Some(expr) = node.child_by_field("expression") {
                extract_expr(emitter, file_id, &expr, source, expr_id, 0);
            }
            if let Some(name) = node.child_by_field("name") {
                extract_expr(emitter, file_id, &name, source, expr_id, 1);
            }
        }
        "element_access_expression" => {
            if let Some(expr) = node.child_by_field("expression") {
                extract_expr(emitter, file_id, &expr, source, expr_id, 0);
            }
            if let Some(args) = node.child_by_field("subscript") {
                extract_argument_list(emitter, file_id, &args, source, expr_id);
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
        "assignment_expression" => {
            if let Some(left) = node.child_by_field("left") {
                extract_expr(emitter, file_id, &left, source, expr_id, 0);
            }
            if let Some(right) = node.child_by_field("right") {
                extract_expr(emitter, file_id, &right, source, expr_id, 1);
            }
        }
        "prefix_unary_expression" | "postfix_unary_expression" => {
            if let Some(operand) = node.child_by_field("operand") {
                extract_expr(emitter, file_id, &operand, source, expr_id, 0);
            }
        }
        "conditional_expression" => {
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, expr_id, 0);
            }
            if let Some(consequence) = node.child_by_field("consequence") {
                extract_expr(emitter, file_id, &consequence, source, expr_id, 1);
            }
            if let Some(alternative) = node.child_by_field("alternative") {
                extract_expr(emitter, file_id, &alternative, source, expr_id, 2);
            }
        }
        "cast_expression" => {
            if let Some(value) = node.child_by_field("value") {
                extract_expr(emitter, file_id, &value, source, expr_id, 0);
            }
        }
        "is_expression" | "is_pattern_expression" | "as_expression" => {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                let mut idx = 0i64;
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, &child, source, expr_id, idx);
                        idx += 1;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "null_coalescing_expression" => {
            if let Some(left) = node.child_by_field("left") {
                extract_expr(emitter, file_id, &left, source, expr_id, 0);
            }
            if let Some(right) = node.child_by_field("right") {
                extract_expr(emitter, file_id, &right, source, expr_id, 1);
            }
        }
        "await_expression" | "throw_expression" => {
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
        "lambda_expression" | "anonymous_method_expression" => {
            if let Some(body) = node.child_by_field("body") {
                if body.kind() == "block" {
                    let dummy_callable = emitter.alloc();
                    extract_stmt(emitter, file_id, &body, source, dummy_callable, 0);
                } else {
                    extract_expr(emitter, file_id, &body, source, expr_id, 0);
                }
            }
        }
        "initializer_expression" | "tuple_expression" => {
            let mut cursor = node.walk();
            let mut idx = 0i64;
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, &child, source, expr_id, idx);
                        idx += 1;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "switch_expression" => {
            if let Some(value) = node.child(0) {
                if value.is_named() {
                    extract_expr(emitter, file_id, &value, source, expr_id, 0);
                }
            }
        }
        "with_expression" => {
            let mut cursor = node.walk();
            let mut idx = 0i64;
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, &child, source, expr_id, idx);
                        idx += 1;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        _ => {}
    }

    Some(expr_id)
}

/// Extract arguments from an argument_list node.
fn extract_argument_list(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    args: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let mut idx = 0i64;
    let mut cursor = args.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "argument" {
                // Extract the expression inside the argument
                if let Some(expr) = child.child_by_field("expression") {
                    extract_expr(emitter, file_id, &expr, source, parent_id, idx + 1);
                } else {
                    // Try first named child
                    let mut inner = child.walk();
                    if inner.goto_first_child() {
                        loop {
                            let ic = inner.node();
                            if ic.is_named() {
                                extract_expr(emitter, file_id, &ic, source, parent_id, idx + 1);
                                break;
                            }
                            if !inner.goto_next_sibling() { break; }
                        }
                    }
                }
                idx += 1;
            } else if child.is_named() && child.kind() != "(" && child.kind() != ")" {
                // Direct expression children in some argument list forms
                extract_expr(emitter, file_id, &child, source, parent_id, idx + 1);
                idx += 1;
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract the initializer expression from a variable_declarator node.
/// In tree-sitter-c-sharp, the structure is: identifier, "=", <expression>
/// or: identifier, equals_value_clause(<expression>)
fn extract_variable_initializer(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    declarator: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let name_node = declarator.child_by_field("name");
    let mut cursor = declarator.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.is_named() {
                // Skip the name identifier
                let is_name = name_node.map_or(false, |n| n.id() == child.id());
                if !is_name {
                    if child.kind() == "equals_value_clause" {
                        // Extract the expression inside equals_value_clause
                        let mut inner = child.walk();
                        if inner.goto_first_child() {
                            loop {
                                let ic = inner.node();
                                if ic.is_named() {
                                    extract_expr(emitter, file_id, &ic, source, parent_id, 0);
                                }
                                if !inner.goto_next_sibling() { break; }
                            }
                        }
                    } else {
                        extract_expr(emitter, file_id, &child, source, parent_id, 0);
                    }
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract a comment node.
fn extract_comment(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let comment_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    let text = node.text(source);
    let text_val = emitter.string(text);
    emitter.emit("comments", vec![
        Value::Entity(comment_id),
        text_val,
        Value::Entity(loc_id),
    ]);
}

/// Find a child node by kind name.
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

#[cfg(test)]
mod tests {
    use super::*;
    use ocql_database::Database;
    use crate::schema::csharp_schema;

    fn extract_test_file(filename: &str) -> Database {
        let source = std::fs::read(
            format!("tests/fixtures/{}", filename)
        ).unwrap();
        let schema = csharp_schema();
        let mut db = Database::from_schema(schema);
        let extractor = CSharpExtractor::new();
        let result = extractor.extract_source(&mut db, filename, &source);
        assert!(result.success, "Extraction failed for {}: {:?}", filename, result.error);
        db
    }

    #[test]
    fn test_simple_types() {
        let db = extract_test_file("Simple.cs");
        let types: Vec<_> = db.scan("types").unwrap().collect();
        let names: Vec<_> = types.iter().map(|t| {
            db.strings.resolve(t[2].as_string().unwrap())
        }).collect();
        eprintln!("Types: {:?}", names);
        assert!(names.contains(&"Calculator"), "Should find 'Calculator'");
        assert!(names.contains(&"IShape"), "Should find 'IShape'");
        assert!(names.contains(&"Circle"), "Should find 'Circle'");
        assert!(names.contains(&"Color"), "Should find 'Color'");
    }

    #[test]
    fn test_simple_methods() {
        let db = extract_test_file("Simple.cs");
        let methods: Vec<_> = db.scan("methods").unwrap().collect();
        let names: Vec<_> = methods.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Methods: {:?}", names);
        assert!(names.contains(&"Add"), "Should find 'Add'");
        assert!(names.contains(&"GetArea"), "Should find 'GetArea'");
        assert!(names.contains(&"Main"), "Should find 'Main'");
    }

    #[test]
    fn test_simple_constructors() {
        let db = extract_test_file("Simple.cs");
        let constrs: Vec<_> = db.scan("constructors").unwrap().collect();
        let names: Vec<_> = constrs.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Constructors: {:?}", names);
        assert!(names.contains(&"Circle"), "Should find 'Circle' constructor");
    }

    #[test]
    fn test_simple_fields() {
        let db = extract_test_file("Simple.cs");
        let fields: Vec<_> = db.scan("fields").unwrap().collect();
        let names: Vec<_> = fields.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Fields: {:?}", names);
        assert!(names.contains(&"_radius"), "Should find '_radius'");
        // Enum members
        assert!(names.contains(&"Red"), "Should find enum member 'Red'");
        assert!(names.contains(&"Green"), "Should find enum member 'Green'");
        assert!(names.contains(&"Blue"), "Should find enum member 'Blue'");
    }

    #[test]
    fn test_simple_properties() {
        let db = extract_test_file("Simple.cs");
        let props: Vec<_> = db.scan("properties").unwrap().collect();
        let names: Vec<_> = props.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Properties: {:?}", names);
        assert!(names.contains(&"Radius"), "Should find 'Radius' property");
    }

    #[test]
    fn test_simple_params() {
        let db = extract_test_file("Simple.cs");
        let params: Vec<_> = db.scan("params").unwrap().collect();
        let param_names: Vec<_> = params.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap()).to_string()
        }).collect();
        eprintln!("Params: {:?}", param_names);
        assert!(param_names.contains(&"a".to_string()), "Should find param 'a'");
        assert!(param_names.contains(&"b".to_string()), "Should find param 'b'");
        assert!(param_names.contains(&"radius".to_string()), "Should find param 'radius'");
    }

    #[test]
    fn test_simple_namespace() {
        let db = extract_test_file("Simple.cs");
        let namespaces: Vec<_> = db.scan("namespaces").unwrap().collect();
        let names: Vec<_> = namespaces.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Namespaces: {:?}", names);
        assert!(names.iter().any(|n| n.contains("MyApp")), "Should find MyApp namespace");
    }

    #[test]
    fn test_simple_using_directives() {
        let db = extract_test_file("Simple.cs");
        let usings: Vec<_> = db.scan("using_directives").unwrap().collect();
        let names: Vec<_> = usings.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Using directives: {:?}", names);
        assert!(names.iter().any(|n| n.contains("System")), "Should find System using");
    }

    #[test]
    fn test_simple_statements() {
        let db = extract_test_file("Simple.cs");
        let stmts: Vec<_> = db.scan("stmts").unwrap().collect();
        let kinds: Vec<i64> = stmts.iter().map(|t| t[1].as_int().unwrap()).collect();
        eprintln!("Statement kinds: {:?}", kinds);
        assert!(kinds.contains(&STMT_BLOCK), "Should have block statements");
        assert!(kinds.contains(&STMT_RETURN), "Should have return statements");
        assert!(kinds.contains(&STMT_IF), "Should have if statements");
    }

    #[test]
    fn test_simple_expressions() {
        let db = extract_test_file("Simple.cs");
        let exprs: Vec<_> = db.scan("exprs").unwrap().collect();
        let kinds: Vec<i64> = exprs.iter().map(|t| t[1].as_int().unwrap()).collect();
        eprintln!("Expression kinds: {:?}", kinds);
        assert!(kinds.contains(&EXPR_CALL), "Should have call expressions");
        assert!(kinds.contains(&EXPR_NEW), "Should have new expressions");
    }

    #[test]
    fn test_simple_modifiers() {
        let db = extract_test_file("Simple.cs");
        let modifiers: Vec<_> = db.scan("modifiers").unwrap().collect();
        let names: Vec<_> = modifiers.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Modifiers: {:?}", names);
        assert!(names.contains(&"public"), "Should find 'public' modifier");
        assert!(names.contains(&"static"), "Should find 'static' modifier");
        assert!(names.contains(&"private"), "Should find 'private' modifier");
    }

    #[test]
    fn test_simple_attributes() {
        let db = extract_test_file("Simple.cs");
        let attrs: Vec<_> = db.scan("attributes").unwrap().collect();
        let names: Vec<_> = attrs.iter().map(|t| {
            db.strings.resolve(t[2].as_string().unwrap())
        }).collect();
        eprintln!("Attributes: {:?}", names);
        assert!(names.iter().any(|n| n.contains("Serializable")), "Should find [Serializable]");
    }

    #[test]
    fn test_simple_local_vars() {
        let db = extract_test_file("Simple.cs");
        let locals: Vec<_> = db.scan("local_vars").unwrap().collect();
        let names: Vec<_> = locals.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Local vars: {:?}", names);
        assert!(names.contains(&"result"), "Should find local 'result'");
    }

    #[test]
    fn test_simple_inheritance() {
        let db = extract_test_file("Simple.cs");
        let impls: Vec<_> = db.scan("implements").unwrap().collect();
        let extends: Vec<_> = db.scan("extends").unwrap().collect();
        eprintln!("Implements: {} entries, Extends: {} entries", impls.len(), extends.len());
        // Circle implements IShape
        assert!(impls.len() >= 1 || extends.len() >= 1, "Should have inheritance entries");
    }

    #[test]
    fn test_simple_type_parameters() {
        let db = extract_test_file("Simple.cs");
        let type_params: Vec<_> = db.scan("type_parameters").unwrap().collect();
        let names: Vec<_> = type_params.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Type parameters: {:?}", names);
        assert!(names.contains(&"T"), "Should find type parameter T");
    }

    #[test]
    fn test_simple_comments() {
        let db = extract_test_file("Simple.cs");
        let comments: Vec<_> = db.scan("comments").unwrap().collect();
        eprintln!("Comments: {} entries", comments.len());
        assert!(comments.len() >= 1, "Should have at least one comment");
    }

}
