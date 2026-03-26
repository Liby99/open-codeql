use ocql_database::{EntityId, Value};
use ocql_extractor_common::{Extractor, FactEmitter, LocationEmitter, NodeExt};
use ocql_extractor_common::tree_sitter::{Language, Node, Tree};

/// Swift extractor using tree-sitter.
///
/// Extracts:
/// - Import declarations
/// - Classes, structs, enums, protocols, extensions
/// - Functions, initializers, subscripts
/// - Properties, parameters, local variables
/// - Enum cases
/// - Statements and expressions
/// - Type inheritance
/// - Generic parameters and where clauses
/// - Access modifiers and attributes
/// - Comments
pub struct SwiftExtractor;

impl SwiftExtractor {
    pub fn new() -> Self {
        Self
    }
}

// Statement kind constants
const STMT_IF: i64 = 0;
const STMT_GUARD: i64 = 1;
const STMT_SWITCH: i64 = 2;
const STMT_FOR_IN: i64 = 3;
const STMT_WHILE: i64 = 4;
const STMT_REPEAT_WHILE: i64 = 5;
const STMT_RETURN: i64 = 6;
const STMT_THROW: i64 = 7;
const STMT_BREAK: i64 = 8;
const STMT_CONTINUE: i64 = 9;
const STMT_DEFER: i64 = 10;
const STMT_DO: i64 = 11;
const STMT_EXPRESSION: i64 = 12;

// Expression kind constants
const EXPR_CALL: i64 = 0;
const EXPR_MEMBER_ACCESS: i64 = 1;
const EXPR_SUBSCRIPT: i64 = 2;
const EXPR_IDENTIFIER: i64 = 3;
const EXPR_INTEGER: i64 = 4;
const EXPR_FLOAT: i64 = 5;
const EXPR_STRING: i64 = 6;
const EXPR_BOOL: i64 = 7;
const EXPR_NIL: i64 = 8;
const EXPR_ARRAY: i64 = 9;
const EXPR_DICTIONARY: i64 = 10;
const EXPR_TUPLE: i64 = 11;
const EXPR_BINARY: i64 = 12;
const EXPR_PREFIX_UNARY: i64 = 13;
const EXPR_POSTFIX_UNARY: i64 = 14;
const EXPR_ASSIGNMENT: i64 = 15;
const EXPR_TERNARY: i64 = 16;
const EXPR_TRY: i64 = 17;
const EXPR_AWAIT: i64 = 18;
const EXPR_CLOSURE: i64 = 19;
const EXPR_IS: i64 = 20;
const EXPR_AS: i64 = 21;
const EXPR_OPTIONAL_CHAIN: i64 = 22;
const EXPR_FORCE_UNWRAP: i64 = 23;
const EXPR_INTERPOLATED_STRING: i64 = 24;
const EXPR_KEY_PATH: i64 = 25;

// Import kind constants
const IMPORT_MODULE: i64 = 0;
const IMPORT_TYPE: i64 = 1;

impl Extractor for SwiftExtractor {
    fn language(&self) -> Language {
        tree_sitter_swift::LANGUAGE.into()
    }

    fn extensions(&self) -> &[&str] {
        &["swift"]
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
    let mut cursor = root.walk();
    if cursor.goto_first_child() {
        loop {
            let node = cursor.node();
            if node.is_named() {
                extract_top_level(emitter, file_id, &node, source, file_id);
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// Extract a top-level declaration.
fn extract_top_level(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    match node.kind() {
        "import_declaration" => {
            extract_import(emitter, file_id, node, source);
        }
        "class_declaration" => {
            // tree-sitter-swift uses class_declaration for class, struct, enum,
            // protocol, extension, and actor. Dispatch based on keyword child.
            match find_type_keyword(node, source).as_str() {
                "struct" => extract_struct(emitter, file_id, node, source, parent_id),
                "enum" => extract_enum(emitter, file_id, node, source, parent_id),
                "protocol" => extract_protocol(emitter, file_id, node, source, parent_id),
                "extension" => extract_extension(emitter, file_id, node, source, parent_id),
                "actor" => extract_class(emitter, file_id, node, source, parent_id),
                _ => extract_class(emitter, file_id, node, source, parent_id),
            }
        }
        "struct_declaration" => {
            extract_struct(emitter, file_id, node, source, parent_id);
        }
        "enum_declaration" => {
            extract_enum(emitter, file_id, node, source, parent_id);
        }
        "protocol_declaration" => {
            extract_protocol(emitter, file_id, node, source, parent_id);
        }
        "extension_declaration" => {
            extract_extension(emitter, file_id, node, source, parent_id);
        }
        "actor_declaration" => {
            // Treat actors like classes
            extract_class(emitter, file_id, node, source, parent_id);
        }
        "function_declaration" => {
            extract_function(emitter, file_id, node, source, parent_id);
        }
        "protocol_function_declaration" => {
            extract_function(emitter, file_id, node, source, parent_id);
        }
        "init_declaration" => {
            extract_initializer(emitter, file_id, node, source, parent_id);
        }
        "deinit_declaration" => {
            // deinit is a special function, skip for now
        }
        "subscript_declaration" => {
            extract_subscript(emitter, file_id, node, source, parent_id);
        }
        "property_declaration" | "protocol_property_declaration" => {
            extract_property(emitter, file_id, node, source, parent_id);
        }
        "typealias_declaration" => {
            // Skip typealias for now
        }
        "comment" | "multiline_comment" => {
            extract_comment(emitter, file_id, node, source);
        }
        _ => {
            // Try to extract as statement or expression
            extract_stmt_or_expr_top_level(emitter, file_id, node, source, parent_id);
        }
    }
}

/// Extract an import declaration.
fn extract_import(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let import_text = node.text(source).to_string();

    // Determine kind: "import class Foo" vs "import Foo"
    let kind = if import_text.contains("import class ")
        || import_text.contains("import struct ")
        || import_text.contains("import enum ")
        || import_text.contains("import protocol ")
        || import_text.contains("import func ")
        || import_text.contains("import var ")
        || import_text.contains("import let ")
        || import_text.contains("import typealias ")
    {
        IMPORT_TYPE
    } else {
        IMPORT_MODULE
    };

    // Extract the import path: everything after "import" (and optional kind keyword)
    let path = import_text
        .trim_start_matches("import ")
        .trim_start_matches("class ")
        .trim_start_matches("struct ")
        .trim_start_matches("enum ")
        .trim_start_matches("protocol ")
        .trim_start_matches("func ")
        .trim_start_matches("var ")
        .trim_start_matches("let ")
        .trim_start_matches("typealias ")
        .trim()
        .to_string();

    let import_id = emitter.alloc();
    let path_val = emitter.string(&path);
    emitter.emit("swift_imports", vec![
        Value::Entity(import_id),
        path_val,
        Value::Int(kind),
    ]);

    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    emitter.emit("hasLocation", vec![
        Value::Entity(import_id),
        Value::Entity(loc_id),
    ]);
}

/// Extract a class declaration.
fn extract_class(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let name = find_declaration_name(node, source);
    if name.is_empty() { return; }

    let class_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let name_val = emitter.string(&name);
    emitter.emit("swift_classes", vec![
        Value::Entity(class_id),
        name_val,
        Value::Entity(parent_id),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(class_id),
        Value::Entity(loc_id),
    ]);

    extract_modifiers_on(emitter, file_id, node, source, class_id);
    extract_attributes_on(emitter, file_id, node, source, class_id);
    extract_inheritance(emitter, node, source, class_id);
    extract_generic_parameters(emitter, file_id, node, source, class_id);
    extract_where_clause(emitter, file_id, node, source, class_id);
    extract_type_body(emitter, file_id, node, source, class_id);
}

/// Extract a struct declaration.
fn extract_struct(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let name = find_declaration_name(node, source);
    if name.is_empty() { return; }

    let struct_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let name_val = emitter.string(&name);
    emitter.emit("swift_structs", vec![
        Value::Entity(struct_id),
        name_val,
        Value::Entity(parent_id),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(struct_id),
        Value::Entity(loc_id),
    ]);

    extract_modifiers_on(emitter, file_id, node, source, struct_id);
    extract_attributes_on(emitter, file_id, node, source, struct_id);
    extract_inheritance(emitter, node, source, struct_id);
    extract_generic_parameters(emitter, file_id, node, source, struct_id);
    extract_where_clause(emitter, file_id, node, source, struct_id);
    extract_type_body(emitter, file_id, node, source, struct_id);
}

/// Extract an enum declaration.
fn extract_enum(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let name = find_declaration_name(node, source);
    if name.is_empty() { return; }

    let enum_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let name_val = emitter.string(&name);
    emitter.emit("swift_enums", vec![
        Value::Entity(enum_id),
        name_val,
        Value::Entity(parent_id),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(enum_id),
        Value::Entity(loc_id),
    ]);

    extract_modifiers_on(emitter, file_id, node, source, enum_id);
    extract_attributes_on(emitter, file_id, node, source, enum_id);
    extract_inheritance(emitter, node, source, enum_id);
    extract_generic_parameters(emitter, file_id, node, source, enum_id);
    extract_where_clause(emitter, file_id, node, source, enum_id);

    // Extract enum body including cases
    extract_enum_body(emitter, file_id, node, source, enum_id);
}

/// Extract a protocol declaration.
fn extract_protocol(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let name = find_declaration_name(node, source);
    if name.is_empty() { return; }

    let protocol_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let name_val = emitter.string(&name);
    emitter.emit("swift_protocols", vec![
        Value::Entity(protocol_id),
        name_val,
        Value::Entity(parent_id),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(protocol_id),
        Value::Entity(loc_id),
    ]);

    extract_modifiers_on(emitter, file_id, node, source, protocol_id);
    extract_attributes_on(emitter, file_id, node, source, protocol_id);
    extract_inheritance(emitter, node, source, protocol_id);
    extract_type_body(emitter, file_id, node, source, protocol_id);
}

/// Extract an extension declaration.
fn extract_extension(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    // Extensions extend an existing type; find the type name
    let type_name = find_extension_type_name(node, source);
    if type_name.is_empty() { return; }

    let ext_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let name_val = emitter.string(&type_name);
    emitter.emit("swift_extensions", vec![
        Value::Entity(ext_id),
        name_val,
        Value::Entity(parent_id),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(ext_id),
        Value::Entity(loc_id),
    ]);

    extract_modifiers_on(emitter, file_id, node, source, ext_id);
    extract_attributes_on(emitter, file_id, node, source, ext_id);
    extract_inheritance(emitter, node, source, ext_id);
    extract_where_clause(emitter, file_id, node, source, ext_id);
    extract_type_body(emitter, file_id, node, source, ext_id);
}

/// Extract a function declaration.
fn extract_function(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let name = find_declaration_name(node, source);
    if name.is_empty() { return; }

    let func_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let name_val = emitter.string(&name);
    emitter.emit("swift_functions", vec![
        Value::Entity(func_id),
        name_val,
        Value::Entity(parent_id),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(func_id),
        Value::Entity(loc_id),
    ]);

    extract_modifiers_on(emitter, file_id, node, source, func_id);
    extract_attributes_on(emitter, file_id, node, source, func_id);
    extract_generic_parameters(emitter, file_id, node, source, func_id);
    extract_where_clause(emitter, file_id, node, source, func_id);
    extract_parameters(emitter, file_id, node, source, func_id);
    extract_function_body(emitter, file_id, node, source, func_id);
}

/// Extract an init declaration.
fn extract_initializer(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let init_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    emitter.emit("swift_initializers", vec![
        Value::Entity(init_id),
        Value::Entity(parent_id),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(init_id),
        Value::Entity(loc_id),
    ]);

    extract_modifiers_on(emitter, file_id, node, source, init_id);
    extract_attributes_on(emitter, file_id, node, source, init_id);
    extract_parameters(emitter, file_id, node, source, init_id);
    extract_function_body(emitter, file_id, node, source, init_id);
}

/// Extract a subscript declaration.
fn extract_subscript(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let sub_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    emitter.emit("swift_subscripts", vec![
        Value::Entity(sub_id),
        Value::Entity(parent_id),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(sub_id),
        Value::Entity(loc_id),
    ]);

    extract_modifiers_on(emitter, file_id, node, source, sub_id);
    extract_parameters(emitter, file_id, node, source, sub_id);
    extract_function_body(emitter, file_id, node, source, sub_id);
}

/// Extract a property declaration (let/var).
fn extract_property(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    // Find pattern bindings within the property declaration
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "pattern" || child.kind() == "simple_identifier" {
                let name = child.text(source).to_string();
                if !name.is_empty() && name != "let" && name != "var" {
                    emit_property(emitter, file_id, node, source, parent_id, &name);
                    return;
                }
            }
            // Check for value_binding_pattern or typed_pattern
            if child.kind() == "value_binding_pattern"
                || child.kind() == "typed_pattern"
                || child.kind() == "pattern"
            {
                if let Some(name) = find_pattern_name(&child, source) {
                    emit_property(emitter, file_id, node, source, parent_id, &name);
                    return;
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }

    // Fallback: try to get the name from child_by_field
    if let Some(name_node) = node.child_by_field("name") {
        let name = name_node.text(source).to_string();
        if !name.is_empty() {
            emit_property(emitter, file_id, node, source, parent_id, &name);
        }
    }
}

fn emit_property(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
    name: &str,
) {
    let prop_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let type_name = find_type_annotation(node, source)
        .unwrap_or_else(|| "unknown".to_string());

    let name_val = emitter.string(name);
    let type_val = emitter.string(&type_name);
    emitter.emit("swift_properties", vec![
        Value::Entity(prop_id),
        name_val,
        type_val,
        Value::Entity(parent_id),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(prop_id),
        Value::Entity(loc_id),
    ]);

    extract_modifiers_on(emitter, file_id, node, source, prop_id);
    extract_attributes_on(emitter, file_id, node, source, prop_id);
}

/// Extract a local variable declaration inside a function body.
fn extract_local_var(
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
            if child.kind() == "pattern" || child.kind() == "simple_identifier" {
                let name = child.text(source).to_string();
                if !name.is_empty() && name != "let" && name != "var" {
                    emit_local_var(emitter, file_id, node, source, parent_id, &name);
                    return;
                }
            }
            if child.kind() == "value_binding_pattern"
                || child.kind() == "typed_pattern"
                || child.kind() == "pattern"
            {
                if let Some(name) = find_pattern_name(&child, source) {
                    emit_local_var(emitter, file_id, node, source, parent_id, &name);
                    return;
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }

    if let Some(name_node) = node.child_by_field("name") {
        let name = name_node.text(source).to_string();
        if !name.is_empty() {
            emit_local_var(emitter, file_id, node, source, parent_id, &name);
        }
    }
}

fn emit_local_var(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
    name: &str,
) {
    let var_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let type_name = find_type_annotation(node, source)
        .unwrap_or_else(|| "unknown".to_string());

    let name_val = emitter.string(name);
    let type_val = emitter.string(&type_name);
    emitter.emit("swift_local_vars", vec![
        Value::Entity(var_id),
        name_val,
        type_val,
        Value::Entity(parent_id),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(var_id),
        Value::Entity(loc_id),
    ]);
}

/// Extract enum body members, including enum cases.
fn extract_enum_body(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    enum_id: EntityId,
) {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "enum_class_body" | "class_body" => {
                    extract_enum_body(emitter, file_id, &child, source, enum_id);
                }
                "enum_entry" => {
                    extract_enum_entry(emitter, file_id, &child, source, enum_id);
                }
                _ => {
                    if child.is_named() {
                        extract_top_level(emitter, file_id, &child, source, enum_id);
                    }
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract a single enum entry (which may contain multiple cases).
fn extract_enum_entry(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    enum_id: EntityId,
) {
    // An enum_entry may have one or more simple_identifier children as case names
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "simple_identifier" {
                let name = child.text(source).to_string();
                if !name.is_empty() && name != "case" {
                    let case_id = emitter.alloc();
                    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                    let name_val = emitter.string(&name);
                    emitter.emit("swift_enum_cases", vec![
                        Value::Entity(case_id),
                        name_val,
                        Value::Entity(enum_id),
                    ]);
                    emitter.emit("hasLocation", vec![
                        Value::Entity(case_id),
                        Value::Entity(loc_id),
                    ]);
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract the body of a type declaration (class, struct, protocol, extension).
fn extract_type_body(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    type_id: EntityId,
) {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "class_body" | "enum_class_body" | "protocol_body" => {
                    extract_type_body_children(emitter, file_id, &child, source, type_id);
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

fn extract_type_body_children(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    body: &Node<'_>,
    source: &[u8],
    type_id: EntityId,
) {
    let mut cursor = body.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.is_named() {
                extract_top_level(emitter, file_id, &child, source, type_id);
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract parameters from a declaration node containing parameter_clause.
fn extract_parameters(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    callable_id: EntityId,
) {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "parameter" {
                extract_single_param(emitter, file_id, &child, source, callable_id);
            } else if child.kind() == "parameter_clause"
                || child.kind() == "function_declaration"
            {
                // Recurse into parameter_clause
                extract_parameters(emitter, file_id, &child, source, callable_id);
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

fn extract_single_param(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    callable_id: EntityId,
) {
    // Parameter can have external_name and name (internal name)
    let mut name = String::new();
    let mut type_name = "unknown".to_string();

    let mut cursor = node.walk();
    let mut found_name = false;
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "simple_identifier" => {
                    // First identifier could be external name, second is internal
                    if !found_name {
                        name = child.text(source).to_string();
                        found_name = true;
                    } else {
                        // This is the internal name
                        name = child.text(source).to_string();
                    }
                }
                "type_annotation" => {
                    type_name = extract_type_annotation_text(&child, source);
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }

    if name.is_empty() || name == "_" {
        // Use a placeholder for unnamed parameters
        if name != "_" {
            return;
        }
    }

    let param_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    // Count existing params for this callable to determine position
    // We use a simple approach: position is set externally. For now, use 0.
    // Actually, we track via static counter within the parent.
    // Since we process params in order, we use a simple heuristic.
    let pos = count_preceding_siblings_of_kind(node, "parameter") as i64;

    let name_val = emitter.string(&name);
    let type_val = emitter.string(&type_name);
    emitter.emit("swift_params", vec![
        Value::Entity(param_id),
        name_val,
        type_val,
        Value::Int(pos),
        Value::Entity(callable_id),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(param_id),
        Value::Entity(loc_id),
    ]);
}

/// Count how many preceding siblings of the same kind exist.
fn count_preceding_siblings_of_kind(node: &Node<'_>, kind: &str) -> usize {
    let mut count = 0;
    if let Some(parent) = node.parent() {
        let mut cursor = parent.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.id() == node.id() {
                    break;
                }
                if child.kind() == kind {
                    count += 1;
                }
                if !cursor.goto_next_sibling() { break; }
            }
        }
    }
    count
}

/// Extract type inheritance (: Protocol, Class).
fn extract_inheritance(
    emitter: &mut FactEmitter<'_>,
    node: &Node<'_>,
    source: &[u8],
    type_id: EntityId,
) {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "inheritance_specifier"
                || child.kind() == "type_identifier"
                || child.kind() == "user_type"
            {
                // Check if this is within an inheritance clause context
                if let Some(parent) = child.parent() {
                    if parent.kind() == "inheritance_specifier" || node.kind().ends_with("_declaration") {
                        extract_inheritance_specifier(emitter, &child, source, type_id);
                    }
                }
            }
            if child.kind() == "inheritance_specifier" {
                extract_inheritance_specifier(emitter, &child, source, type_id);
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

fn extract_inheritance_specifier(
    emitter: &mut FactEmitter<'_>,
    node: &Node<'_>,
    source: &[u8],
    type_id: EntityId,
) {
    // An inheritance_specifier contains type identifiers
    let mut idx = 0i64;
    let mut cursor = node.walk();
    if node.kind() == "inheritance_specifier" {
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.is_named() && child.kind() != "," {
                    let type_name = child.text(source).to_string();
                    if !type_name.is_empty() && type_name != ":" {
                        let name_val = emitter.string(&type_name);
                        emitter.emit("swift_type_inheritance", vec![
                            Value::Entity(type_id),
                            name_val,
                            Value::Int(idx),
                        ]);
                        idx += 1;
                    }
                }
                if !cursor.goto_next_sibling() { break; }
            }
        }
    } else {
        // Direct type identifier
        let type_name = node.text(source).to_string();
        if !type_name.is_empty() {
            let name_val = emitter.string(&type_name);
            emitter.emit("swift_type_inheritance", vec![
                Value::Entity(type_id),
                name_val,
                Value::Int(0),
            ]);
        }
    }
}

/// Extract generic type parameters.
fn extract_generic_parameters(
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
            if child.kind() == "type_parameters" {
                extract_type_params_inner(emitter, file_id, &child, source, parent_id);
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

fn extract_type_params_inner(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let mut index = 0i64;
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "type_parameter" || child.kind() == "simple_identifier" {
                let name = if child.kind() == "type_parameter" {
                    // The type_parameter may contain a simple_identifier
                    child.child(0)
                        .map(|c| c.text(source).to_string())
                        .unwrap_or_else(|| child.text(source).to_string())
                } else {
                    child.text(source).to_string()
                };

                if !name.is_empty() {
                    let gen_id = emitter.alloc();
                    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                    let name_val = emitter.string(&name);
                    emitter.emit("swift_generics", vec![
                        Value::Entity(gen_id),
                        name_val,
                        Value::Int(index),
                        Value::Entity(parent_id),
                    ]);
                    emitter.emit("hasLocation", vec![
                        Value::Entity(gen_id),
                        Value::Entity(loc_id),
                    ]);
                    index += 1;
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract where clause.
fn extract_where_clause(
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
            if child.kind() == "type_constraint" || child.kind() == "where_clause"
                || child.kind() == "generic_where_clause"
            {
                let wc_id = emitter.alloc();
                let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                let req = child.text(source).to_string();
                let req_val = emitter.string(&req);
                emitter.emit("swift_where_clauses", vec![
                    Value::Entity(wc_id),
                    req_val,
                    Value::Entity(parent_id),
                ]);
                emitter.emit("hasLocation", vec![
                    Value::Entity(wc_id),
                    Value::Entity(loc_id),
                ]);
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract modifiers (public, private, etc.) from a declaration.
fn extract_modifiers_on(
    emitter: &mut FactEmitter<'_>,
    _file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "modifiers" {
                extract_modifiers_children(emitter, &child, source, parent_id);
                return;
            }
            // Some grammars put modifiers directly as children
            if child.kind() == "modifier" {
                emit_modifier(emitter, &child, source, parent_id);
            }
            // Access modifiers may appear as direct keywords
            if is_swift_modifier(child.text(source)) && !child.is_named() {
                // Anonymous keyword node
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

fn extract_modifiers_children(
    emitter: &mut FactEmitter<'_>,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "modifier" || child.kind() == "property_modifier"
                || child.kind() == "member_modifier" || child.kind() == "function_modifier"
                || child.kind() == "visibility_modifier" || child.kind() == "ownership_modifier"
                || child.kind() == "inheritance_modifier"
            {
                emit_modifier(emitter, &child, source, parent_id);
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

fn emit_modifier(
    emitter: &mut FactEmitter<'_>,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let mod_text = node.text(source).trim().to_string();
    if !mod_text.is_empty() && is_swift_modifier(&mod_text) {
        let mod_id = emitter.alloc();
        let name_val = emitter.string(&mod_text);
        emitter.emit("swift_modifiers", vec![
            Value::Entity(mod_id),
            name_val,
        ]);
        emitter.emit("swift_hasModifier", vec![
            Value::Entity(parent_id),
            Value::Entity(mod_id),
        ]);
    }
}

fn is_swift_modifier(text: &str) -> bool {
    matches!(text,
        "public" | "private" | "fileprivate" | "internal" | "open"
        | "static" | "class" | "final" | "override" | "required"
        | "convenience" | "lazy" | "weak" | "unowned" | "mutating"
        | "nonmutating" | "optional" | "indirect" | "dynamic"
        | "prefix" | "postfix" | "infix" | "nonisolated"
        | "async" | "rethrows" | "throws"
    )
}

/// Extract attributes (@objc, @discardableResult, etc.).
fn extract_attributes_on(
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
            if child.kind() == "attribute" {
                let attr_text = child.text(source).to_string();
                // Strip the leading @
                let attr_name = attr_text.trim_start_matches('@')
                    .split('(').next()
                    .unwrap_or(&attr_text)
                    .trim()
                    .to_string();
                if !attr_name.is_empty() {
                    let attr_id = emitter.alloc();
                    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                    let name_val = emitter.string(&attr_name);
                    emitter.emit("swift_attributes", vec![
                        Value::Entity(attr_id),
                        name_val,
                        Value::Entity(parent_id),
                    ]);
                    emitter.emit("hasLocation", vec![
                        Value::Entity(attr_id),
                        Value::Entity(loc_id),
                    ]);
                }
            }
            // Also check modifiers node for attributes
            if child.kind() == "modifiers" {
                extract_attributes_in_modifiers(emitter, file_id, &child, source, parent_id);
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

fn extract_attributes_in_modifiers(
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
            if child.kind() == "attribute" {
                let attr_text = child.text(source).to_string();
                let attr_name = attr_text.trim_start_matches('@')
                    .split('(').next()
                    .unwrap_or(&attr_text)
                    .trim()
                    .to_string();
                if !attr_name.is_empty() {
                    let attr_id = emitter.alloc();
                    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                    let name_val = emitter.string(&attr_name);
                    emitter.emit("swift_attributes", vec![
                        Value::Entity(attr_id),
                        name_val,
                        Value::Entity(parent_id),
                    ]);
                    emitter.emit("hasLocation", vec![
                        Value::Entity(attr_id),
                        Value::Entity(loc_id),
                    ]);
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract the function body (statements).
fn extract_function_body(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    func_id: EntityId,
) {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "function_body" || child.kind() == "statements" {
                extract_statements(emitter, file_id, &child, source, func_id);
            }
            // Some function bodies are direct code blocks
            if child.kind() == "{" || child.kind() == "}" {
                // skip braces
            } else if child.kind() == "statements" {
                extract_statements(emitter, file_id, &child, source, func_id);
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract statements from a statements/body node.
fn extract_statements(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let mut idx = 0i64;
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.is_named() {
                // If child is a "statements" node, recurse into it
                if child.kind() == "statements" {
                    extract_statements(emitter, file_id, &child, source, parent_id);
                } else if extract_stmt(emitter, file_id, &child, source, parent_id, idx).is_some() {
                    idx += 1;
                } else {
                    // Not a statement; try as top-level declaration
                    extract_top_level(emitter, file_id, &child, source, parent_id);
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Try to extract non-declaration items as statements or expressions at top level.
fn extract_stmt_or_expr_top_level(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    // Try statement first
    if extract_stmt(emitter, file_id, node, source, parent_id, 0).is_some() {
        return;
    }
    // Try expression
    extract_expr(emitter, file_id, node, source, parent_id, 0);
}

/// Extract a statement, returning the entity ID if recognized.
fn extract_stmt(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
    index: i64,
) -> Option<EntityId> {
    let kind = match node.kind() {
        "if_statement" => Some(STMT_IF),
        "guard_statement" => Some(STMT_GUARD),
        "switch_statement" => Some(STMT_SWITCH),
        "for_statement" | "for_in_statement" => Some(STMT_FOR_IN),
        "while_statement" => Some(STMT_WHILE),
        "repeat_while_statement" => Some(STMT_REPEAT_WHILE),
        "return_statement" => Some(STMT_RETURN),
        // tree-sitter-swift uses control_transfer_statement for return/throw/break/continue
        "control_transfer_statement" => {
            let text = node.text(source).trim_start();
            if text.starts_with("return") {
                Some(STMT_RETURN)
            } else if text.starts_with("throw") {
                Some(STMT_THROW)
            } else if text.starts_with("break") {
                Some(STMT_BREAK)
            } else if text.starts_with("continue") {
                Some(STMT_CONTINUE)
            } else {
                Some(STMT_RETURN) // fallback
            }
        }
        "throw_statement" => Some(STMT_THROW),
        "break_statement" => Some(STMT_BREAK),
        "continue_statement" => Some(STMT_CONTINUE),
        "defer_statement" => Some(STMT_DEFER),
        "do_statement" => Some(STMT_DO),
        // Property declarations inside function bodies are local vars
        "property_declaration" => {
            extract_local_var(emitter, file_id, node, source, parent_id);
            return None;
        }
        _ => {
            // Check if it could be an expression statement
            if is_expression_node(node) {
                Some(STMT_EXPRESSION)
            } else {
                None
            }
        }
    };

    let stmt_kind = match kind {
        Some(k) => k,
        None => return None,
    };

    let stmt_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    emitter.emit("swift_stmts", vec![
        Value::Entity(stmt_id),
        Value::Int(stmt_kind),
        Value::Entity(parent_id),
        Value::Int(index),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(stmt_id),
        Value::Entity(loc_id),
    ]);

    // Recurse into sub-statements and expressions
    match node.kind() {
        "if_statement" => {
            extract_stmt_children(emitter, file_id, node, source, stmt_id);
        }
        "guard_statement" => {
            extract_stmt_children(emitter, file_id, node, source, stmt_id);
        }
        "switch_statement" => {
            extract_switch_entries(emitter, file_id, node, source, stmt_id);
        }
        "for_statement" | "for_in_statement" => {
            extract_stmt_children(emitter, file_id, node, source, stmt_id);
        }
        "while_statement" | "repeat_while_statement" => {
            extract_stmt_children(emitter, file_id, node, source, stmt_id);
        }
        "return_statement" | "throw_statement" | "control_transfer_statement" => {
            // Extract the return/throw expression
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
        "do_statement" => {
            extract_stmt_children(emitter, file_id, node, source, stmt_id);
        }
        "defer_statement" => {
            extract_stmt_children(emitter, file_id, node, source, stmt_id);
        }
        _ => {
            // Expression statement: extract the expression
            if stmt_kind == STMT_EXPRESSION {
                extract_expr(emitter, file_id, node, source, stmt_id, 0);
            }
        }
    }

    Some(stmt_id)
}

fn extract_stmt_children(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let mut idx = 0i64;
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.is_named() {
                if child.kind() == "statements" || child.kind() == "function_body" {
                    extract_statements(emitter, file_id, &child, source, parent_id);
                } else if child.kind() == "catch_clause" {
                    // Extract catch body
                    extract_stmt_children(emitter, file_id, &child, source, parent_id);
                } else if let Some(_) = extract_stmt(emitter, file_id, &child, source, parent_id, idx) {
                    idx += 1;
                } else {
                    extract_expr(emitter, file_id, &child, source, parent_id, idx);
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

fn extract_switch_entries(
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
            if child.kind() == "switch_entry" {
                extract_stmt_children(emitter, file_id, &child, source, parent_id);
            }
            if !cursor.goto_next_sibling() { break; }
        }
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
        "call_expression" => Some(EXPR_CALL),
        "navigation_expression" => Some(EXPR_MEMBER_ACCESS),
        "subscript_expression" => Some(EXPR_SUBSCRIPT),
        "simple_identifier" => Some(EXPR_IDENTIFIER),
        "integer_literal" => Some(EXPR_INTEGER),
        "real_literal" => Some(EXPR_FLOAT),
        "line_string_literal" | "multiline_string_literal" | "raw_string_literal" => Some(EXPR_STRING),
        "boolean_literal" => Some(EXPR_BOOL),
        "nil" => Some(EXPR_NIL),
        "array_literal" => Some(EXPR_ARRAY),
        "dictionary_literal" => Some(EXPR_DICTIONARY),
        "tuple_expression" => Some(EXPR_TUPLE),
        "infix_expression" => {
            // Check if this is an assignment
            let text = node.text(source);
            if text.contains(" = ") && !text.contains("==") && !text.contains("!=")
                && !text.contains("<=") && !text.contains(">=")
            {
                Some(EXPR_ASSIGNMENT)
            } else {
                Some(EXPR_BINARY)
            }
        }
        "prefix_expression" => Some(EXPR_PREFIX_UNARY),
        "postfix_expression" => Some(EXPR_POSTFIX_UNARY),
        "directly_assignable_expression" | "assignment" => Some(EXPR_ASSIGNMENT),
        "ternary_expression" => Some(EXPR_TERNARY),
        "try_expression" => Some(EXPR_TRY),
        "await_expression" => Some(EXPR_AWAIT),
        "lambda_literal" | "closure_expression" => Some(EXPR_CLOSURE),
        "is_expression" | "check_expression" => Some(EXPR_IS),
        "as_expression" => Some(EXPR_AS),
        "optional_chaining_expression" => Some(EXPR_OPTIONAL_CHAIN),
        "forced_value_expression" | "force_unwrap_expression" => Some(EXPR_FORCE_UNWRAP),
        "interpolated_expression" => Some(EXPR_INTERPOLATED_STRING),
        "key_path_expression" => Some(EXPR_KEY_PATH),
        "parenthesized_expression" => {
            // Extract inner expression directly
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
        // Handle string interpolation at the top level
        "interpolation" => Some(EXPR_INTERPOLATED_STRING),
        _ => None,
    };

    let expr_kind = match kind {
        Some(k) => k,
        None => return None,
    };

    let expr_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    emitter.emit("swift_exprs", vec![
        Value::Entity(expr_id),
        Value::Int(expr_kind),
        Value::Entity(parent_id),
        Value::Int(index),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(expr_id),
        Value::Entity(loc_id),
    ]);

    // Recurse into child expressions
    match node.kind() {
        "call_expression" => {
            // Extract function reference and arguments
            let mut idx = 0i64;
            let mut cursor = node.walk();
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
        "navigation_expression" => {
            let mut idx = 0i64;
            let mut cursor = node.walk();
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
        "infix_expression" => {
            let mut idx = 0i64;
            let mut cursor = node.walk();
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
        "prefix_expression" | "postfix_expression" => {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, &child, source, expr_id, 0);
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "ternary_expression" => {
            let mut idx = 0i64;
            let mut cursor = node.walk();
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
        "try_expression" | "await_expression" => {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, &child, source, expr_id, 0);
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "is_expression" | "check_expression" | "as_expression" => {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, &child, source, expr_id, 0);
                        break; // Only the expression operand, not the type
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "lambda_literal" | "closure_expression" => {
            // Extract closure body
            extract_stmt_children(emitter, file_id, node, source, expr_id);
        }
        "subscript_expression" => {
            let mut idx = 0i64;
            let mut cursor = node.walk();
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
        "array_literal" | "dictionary_literal" | "tuple_expression" => {
            let mut idx = 0i64;
            let mut cursor = node.walk();
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
        _ => {}
    }

    Some(expr_id)
}

/// Extract a comment.
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
    emitter.emit("swift_comments", vec![
        Value::Entity(comment_id),
        text_val,
        Value::Entity(loc_id),
    ]);
}

// ========== Helper functions ==========

/// Find the type keyword (class, struct, enum, protocol, extension, actor)
/// inside a class_declaration node. tree-sitter-swift 0.7 uses class_declaration
/// for all type declarations.
fn find_type_keyword(node: &Node<'_>, source: &[u8]) -> String {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            // Skip modifiers node
            if child.kind() == "modifiers" {
                if !cursor.goto_next_sibling() { break; }
                continue;
            }
            let text = child.text(source);
            if matches!(text, "class" | "struct" | "enum" | "protocol" | "extension" | "actor") {
                return text.to_string();
            }
            // Once we hit a type_identifier or body, stop looking
            if child.kind() == "type_identifier" || child.kind() == "class_body"
                || child.kind() == "enum_class_body" || child.kind() == "protocol_body"
            {
                break;
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
    String::new()
}

/// Find the name of a declaration (class, struct, enum, protocol, function).
fn find_declaration_name(node: &Node<'_>, source: &[u8]) -> String {
    // Try field "name" first
    if let Some(name_node) = node.child_by_field("name") {
        return name_node.text(source).to_string();
    }

    // Walk children looking for simple_identifier after the keyword
    let mut cursor = node.walk();
    let mut found_keyword = false;
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            let text = child.text(source);
            if !found_keyword {
                if matches!(text, "class" | "struct" | "enum" | "protocol" | "func" | "actor"
                    | "extension" | "init" | "subscript")
                {
                    found_keyword = true;
                }
            } else if child.kind() == "simple_identifier" || child.kind() == "type_identifier" {
                return text.to_string();
            } else if child.kind() == "type_parameters" || child.kind() == "parameter_clause"
                || child.kind() == "inheritance_specifier" || child.kind() == "class_body"
                || child.kind() == "enum_class_body"
            {
                // Went past where the name should be
                break;
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }

    String::new()
}

/// Find the type name for an extension declaration.
fn find_extension_type_name(node: &Node<'_>, source: &[u8]) -> String {
    let mut cursor = node.walk();
    let mut found_extension = false;
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            let text = child.text(source);
            if !found_extension {
                if text == "extension" {
                    found_extension = true;
                }
            } else if child.kind() == "user_type" || child.kind() == "type_identifier"
                || child.kind() == "simple_identifier"
            {
                return text.to_string();
            } else if child.kind() == "inheritance_specifier" || child.kind() == "class_body"
                || child.kind() == "type_parameters" || child.kind() == "where_clause"
            {
                break;
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
    String::new()
}

/// Find a name in a pattern binding.
fn find_pattern_name(node: &Node<'_>, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "simple_identifier" {
                let name = child.text(source).to_string();
                if !name.is_empty() && name != "let" && name != "var" {
                    return Some(name);
                }
            }
            // Recurse into nested patterns
            if child.kind() == "pattern" || child.kind() == "value_binding_pattern"
                || child.kind() == "typed_pattern" || child.kind() == "binding_pattern"
            {
                if let Some(name) = find_pattern_name(&child, source) {
                    return Some(name);
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
    None
}

/// Find a type annotation on a node.
fn find_type_annotation(node: &Node<'_>, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "type_annotation" {
                return Some(extract_type_annotation_text(&child, source));
            }
            // Look inside patterns
            if child.kind() == "typed_pattern" || child.kind() == "pattern"
                || child.kind() == "value_binding_pattern"
            {
                if let Some(t) = find_type_annotation(&child, source) {
                    return Some(t);
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
    None
}

/// Extract the text of a type annotation node (strip the leading `:` and whitespace).
fn extract_type_annotation_text(node: &Node<'_>, source: &[u8]) -> String {
    let text = node.text(source).to_string();
    text.trim_start_matches(':').trim().to_string()
}

/// Check if a node is an expression.
fn is_expression_node(node: &Node<'_>) -> bool {
    matches!(node.kind(),
        "call_expression" | "navigation_expression" | "subscript_expression"
        | "simple_identifier" | "integer_literal" | "real_literal"
        | "line_string_literal" | "multiline_string_literal" | "raw_string_literal"
        | "boolean_literal" | "nil"
        | "array_literal" | "dictionary_literal" | "tuple_expression"
        | "infix_expression" | "prefix_expression" | "postfix_expression"
        | "assignment" | "directly_assignable_expression"
        | "ternary_expression" | "try_expression" | "await_expression"
        | "lambda_literal" | "closure_expression"
        | "is_expression" | "check_expression" | "as_expression"
        | "optional_chaining_expression" | "forced_value_expression" | "force_unwrap_expression"
        | "interpolated_expression" | "key_path_expression"
        | "parenthesized_expression" | "interpolation"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocql_database::Database;
    use crate::schema::swift_schema;

    fn extract_test_file(filename: &str) -> Database {
        let source = std::fs::read(
            format!("tests/fixtures/{}", filename)
        ).unwrap();
        let schema = swift_schema();
        let mut db = Database::from_schema(schema);
        let extractor = SwiftExtractor::new();
        let result = extractor.extract_source(&mut db, filename, &source);
        assert!(result.success, "Extraction failed for {}: {:?}", filename, result.error);
        db
    }

    #[test]
    fn test_swift_imports() {
        let db = extract_test_file("simple.swift");
        let imports: Vec<_> = db.scan("swift_imports").unwrap().collect();
        let paths: Vec<_> = imports.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Imports: {:?}", paths);
        assert!(paths.iter().any(|p| p.contains("Foundation")), "Should find Foundation import");
        assert!(paths.iter().any(|p| p.contains("UIKit")), "Should find UIKit import");
    }

    #[test]
    fn test_swift_protocols() {
        let db = extract_test_file("simple.swift");
        let protocols: Vec<_> = db.scan("swift_protocols").unwrap().collect();
        let names: Vec<_> = protocols.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Protocols: {:?}", names);
        assert!(names.contains(&"Drawable"), "Should find Drawable protocol");
    }

    #[test]
    fn test_swift_classes() {
        let db = extract_test_file("simple.swift");
        let classes: Vec<_> = db.scan("swift_classes").unwrap().collect();
        let names: Vec<_> = classes.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Classes: {:?}", names);
        assert!(names.contains(&"Animal"), "Should find Animal class");
        assert!(names.contains(&"Dog"), "Should find Dog class");
    }

    #[test]
    fn test_swift_structs() {
        let db = extract_test_file("simple.swift");
        let structs: Vec<_> = db.scan("swift_structs").unwrap().collect();
        let names: Vec<_> = structs.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Structs: {:?}", names);
        assert!(names.contains(&"Point"), "Should find Point struct");
    }

    #[test]
    fn test_swift_enums() {
        let db = extract_test_file("simple.swift");
        let enums: Vec<_> = db.scan("swift_enums").unwrap().collect();
        let names: Vec<_> = enums.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Enums: {:?}", names);
        assert!(names.contains(&"Direction"), "Should find Direction enum");
    }

    #[test]
    fn test_swift_enum_cases() {
        let db = extract_test_file("simple.swift");
        let cases: Vec<_> = db.scan("swift_enum_cases").unwrap().collect();
        let names: Vec<_> = cases.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Enum cases: {:?}", names);
        assert!(names.contains(&"north"), "Should find 'north' case");
        assert!(names.contains(&"south"), "Should find 'south' case");
    }

    #[test]
    fn test_swift_functions() {
        let db = extract_test_file("simple.swift");
        let functions: Vec<_> = db.scan("swift_functions").unwrap().collect();
        let names: Vec<_> = functions.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Functions: {:?}", names);
        assert!(names.iter().any(|n| *n == "greet" || n.contains("greet")), "Should find greet function");
        assert!(names.iter().any(|n| *n == "swap" || n.contains("swap")), "Should find swap function");
    }

    #[test]
    fn test_swift_properties() {
        let db = extract_test_file("simple.swift");
        let props: Vec<_> = db.scan("swift_properties").unwrap().collect();
        let names: Vec<_> = props.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Properties: {:?}", names);
        assert!(names.contains(&"name"), "Should find 'name' property");
    }

    #[test]
    fn test_swift_extensions() {
        let db = extract_test_file("simple.swift");
        let extensions: Vec<_> = db.scan("swift_extensions").unwrap().collect();
        let names: Vec<_> = extensions.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Extensions: {:?}", names);
        assert!(names.iter().any(|n| n.contains("Point")), "Should find Point extension");
    }

    #[test]
    fn test_swift_comments() {
        let db = extract_test_file("simple.swift");
        let comments: Vec<_> = db.scan("swift_comments").unwrap().collect();
        eprintln!("Comments: {} total", comments.len());
        assert!(comments.len() >= 1, "Should have at least one comment");
    }

    #[test]
    fn test_swift_stmts() {
        let db = extract_test_file("simple.swift");
        let stmts: Vec<_> = db.scan("swift_stmts").unwrap().collect();
        let kinds: Vec<i64> = stmts.iter().map(|t| t[1].as_int().unwrap()).collect();
        eprintln!("Statement kinds: {:?}", kinds);
        assert!(!stmts.is_empty(), "Should have some statements");
    }

    #[test]
    fn test_swift_exprs() {
        let db = extract_test_file("simple.swift");
        let exprs: Vec<_> = db.scan("swift_exprs").unwrap().collect();
        let kinds: Vec<i64> = exprs.iter().map(|t| t[1].as_int().unwrap()).collect();
        eprintln!("Expression kinds: {:?}", kinds);
        assert!(!exprs.is_empty(), "Should have some expressions");
    }

    #[test]
    fn test_swift_locations() {
        let db = extract_test_file("simple.swift");
        let locs: Vec<_> = db.scan("locations_default").unwrap().collect();
        eprintln!("Locations: {} total", locs.len());
        assert!(locs.len() >= 10, "Should have many locations");
    }

    #[test]
    fn test_swift_has_location() {
        let db = extract_test_file("simple.swift");
        let has_locs: Vec<_> = db.scan("hasLocation").unwrap().collect();
        eprintln!("hasLocation: {} total", has_locs.len());
        assert!(has_locs.len() >= 5, "Should have hasLocation entries");
    }

    #[test]
    fn test_debug_tree() {
        let source = b"struct Point { var x: Int }\nenum Direction { case north }\nextension Point { func draw() {} }\nlet x = 1\nif true { print(x) }\n";
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_swift::LANGUAGE.into()).unwrap();
        let tree = parser.parse(&source[..], None).unwrap();
        fn pt(node: &tree_sitter::Node, source: &[u8], depth: usize) {
            if depth > 4 { return; }
            let indent = "  ".repeat(depth);
            let text = node.utf8_text(source).unwrap_or("").replace('\n', "\\n");
            let short = if text.len() > 60 { format!("{}...", &text[..60]) } else { text };
            eprintln!("{}{} [named={}] {:?}", indent, node.kind(), node.is_named(), short);
            let mut c = node.walk();
            if c.goto_first_child() { loop { pt(&c.node(), source, depth+1); if !c.goto_next_sibling() { break; } } }
        }
        pt(&tree.root_node(), &source[..], 0);
    }
}
