use ocql_database::{EntityId, Value};
use ocql_extractor_common::{Extractor, FactEmitter, LocationEmitter, NodeExt};
use ocql_extractor_common::tree_sitter::{Language, Node, Tree};

/// Java extractor using tree-sitter.
///
/// Extracts:
/// - Packages, imports
/// - Classes, interfaces, enums, records, annotation types
/// - Methods, constructors, fields
/// - Parameters, local variables
/// - Statements and expressions
/// - Annotations, modifiers
/// - Type parameters (generics)
/// - Comments
pub struct JavaExtractor;

impl JavaExtractor {
    pub fn new() -> Self {
        Self
    }
}

// Statement kind constants (matches CodeQL Java semmlecode.dbscheme)
const STMT_BLOCK: i64 = 0;
const STMT_IF: i64 = 1;
const STMT_FOR: i64 = 2;
const STMT_ENHANCED_FOR: i64 = 3;
const STMT_WHILE: i64 = 4;
const STMT_DO: i64 = 5;
const STMT_TRY: i64 = 6;
const STMT_SWITCH: i64 = 7;
const STMT_RETURN: i64 = 9;
const STMT_THROW: i64 = 10;
const STMT_BREAK: i64 = 11;
const STMT_CONTINUE: i64 = 12;
const STMT_EMPTY: i64 = 13;
const STMT_EXPR: i64 = 14;
const STMT_LABELED: i64 = 15;
const STMT_ASSERT: i64 = 16;
const STMT_LOCAL_VAR_DECL: i64 = 17;
const STMT_CATCH: i64 = 22;

// Expression kind constants (matches CodeQL Java semmlecode.dbscheme)
const EXPR_ASSIGN: i64 = 4;
const EXPR_ASSIGNADD: i64 = 5;
const EXPR_ASSIGNSUB: i64 = 6;
const EXPR_ASSIGNMUL: i64 = 7;
const EXPR_ASSIGNDIV: i64 = 8;
const EXPR_ASSIGNREM: i64 = 9;
const EXPR_ASSIGNAND: i64 = 10;
const EXPR_ASSIGNOR: i64 = 11;
const EXPR_ASSIGNXOR: i64 = 12;
const EXPR_ASSIGNLSHIFT: i64 = 13;
const EXPR_ASSIGNRSHIFT: i64 = 14;
const EXPR_ASSIGNURSHIFT: i64 = 15;
const EXPR_BOOLEAN_LIT: i64 = 16;
const EXPR_INT_LIT: i64 = 17;
const EXPR_LONG_LIT: i64 = 18;
const EXPR_FLOAT_LIT: i64 = 19;
const EXPR_DOUBLE_LIT: i64 = 20;
const EXPR_CHAR_LIT: i64 = 21;
const EXPR_STRING_LIT: i64 = 22;
const EXPR_NULL_LIT: i64 = 23;
const EXPR_MUL: i64 = 24;
const EXPR_DIV: i64 = 25;
const EXPR_REM: i64 = 26;
const EXPR_ADD: i64 = 27;
const EXPR_SUB: i64 = 28;
const EXPR_LSHIFT: i64 = 29;
const EXPR_RSHIFT: i64 = 30;
const EXPR_URSHIFT: i64 = 31;
const EXPR_ANDBIT: i64 = 32;
const EXPR_ORBIT: i64 = 33;
const EXPR_XORBIT: i64 = 34;
const EXPR_ANDLOG: i64 = 35;
const EXPR_ORLOG: i64 = 36;
const EXPR_LT: i64 = 37;
const EXPR_GT: i64 = 38;
const EXPR_LE: i64 = 39;
const EXPR_GE: i64 = 40;
const EXPR_EQ: i64 = 41;
const EXPR_NE: i64 = 42;
const EXPR_POSTINC: i64 = 43;
const EXPR_POSTDEC: i64 = 44;
const EXPR_PREINC: i64 = 45;
const EXPR_PREDEC: i64 = 46;
const EXPR_MINUS: i64 = 47;
const EXPR_PLUS: i64 = 48;
const EXPR_BITNOT: i64 = 49;
const EXPR_LOGNOT: i64 = 50;
const EXPR_CAST: i64 = 51;
const EXPR_NEW: i64 = 52;
const EXPR_CONDITIONAL: i64 = 53;
const EXPR_INSTANCEOF: i64 = 55;
const EXPR_LOCALVARDECL: i64 = 56;
const EXPR_THIS: i64 = 58;
const EXPR_SUPER: i64 = 59;
const EXPR_VARACCESS: i64 = 60;
const EXPR_METHODACCESS: i64 = 61;
const EXPR_ARRAY_ACCESS: i64 = 1;
const EXPR_ARRAY_CREATION: i64 = 2;
const EXPR_ARRAY_INIT: i64 = 3;
const EXPR_LAMBDA: i64 = 68;

// Import kind constants
const IMPORT_SINGLE: i64 = 1;
const IMPORT_ON_DEMAND: i64 = 2;
const IMPORT_STATIC_SINGLE: i64 = 3;
const IMPORT_STATIC_ON_DEMAND: i64 = 4;

impl Extractor for JavaExtractor {
    fn language(&self) -> Language {
        tree_sitter_java::LANGUAGE.into()
    }

    fn extensions(&self) -> &[&str] {
        &["java"]
    }

    fn extract_file(
        &self,
        emitter: &mut FactEmitter<'_>,
        file_id: EntityId,
        tree: &Tree,
        source: &[u8],
    ) {
        let root = tree.root_node();
        extract_program(emitter, file_id, &root, source);
    }
}

/// Extract the top-level program node.
fn extract_program(
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
/// `enclosing_class` is Some when extracting nested types inside a class body.
fn extract_top_level(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    enclosing_class: Option<EntityId>,
) {
    match node.kind() {
        "package_declaration" => {
            extract_package(emitter, file_id, node, source);
        }
        "import_declaration" => {
            extract_import(emitter, file_id, node, source);
        }
        "class_declaration" => {
            extract_class(emitter, file_id, node, source, false, enclosing_class);
        }
        "interface_declaration" => {
            extract_class(emitter, file_id, node, source, true, enclosing_class);
        }
        "enum_declaration" => {
            extract_enum(emitter, file_id, node, source, enclosing_class);
        }
        "record_declaration" => {
            extract_class(emitter, file_id, node, source, false, enclosing_class);
        }
        "annotation_type_declaration" => {
            extract_annotation_type(emitter, file_id, node, source, enclosing_class);
        }
        "line_comment" | "block_comment" => {
            extract_comment(emitter, file_id, node, source);
        }
        _ => {}
    }
}

/// Extract a package declaration.
fn extract_package(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    // Find the scoped_identifier or identifier child
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "scoped_identifier" || child.kind() == "identifier" {
                let pkg_name = child.text(source);
                let pkg_id = emitter.alloc();
                let name_val = emitter.string(pkg_name);
                emitter.emit("packages", vec![
                    Value::Entity(pkg_id),
                    name_val,
                ]);
                emitter.emit("cupackage", vec![
                    Value::Entity(file_id),
                    Value::Entity(pkg_id),
                ]);
                let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
                emitter.emit("hasLocation", vec![
                    Value::Entity(pkg_id),
                    Value::Entity(loc_id),
                ]);
                return;
            }
            if !cursor.goto_next_sibling() { break; }
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
    let import_text = node.text(source);
    let is_static = import_text.contains("static ");
    let is_star = import_text.contains(".*");

    let kind = match (is_static, is_star) {
        (false, false) => IMPORT_SINGLE,
        (false, true) => IMPORT_ON_DEMAND,
        (true, false) => IMPORT_STATIC_SINGLE,
        (true, true) => IMPORT_STATIC_ON_DEMAND,
    };

    // Find the imported name
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "scoped_identifier" || child.kind() == "identifier"
                || child.kind() == "asterisk"
            {
                let import_id = emitter.alloc();
                // Get the full import path (scoped_identifier text)
                let name = if child.kind() == "asterisk" {
                    // For wildcard import, the scoped_identifier is a sibling
                    import_text
                        .trim_start_matches("import ")
                        .trim_start_matches("static ")
                        .trim_end_matches(';')
                        .trim()
                        .to_string()
                } else {
                    child.text(source).to_string()
                };
                let name_val = emitter.string(&name);
                emitter.emit("imports", vec![
                    Value::Entity(import_id),
                    Value::Entity(file_id),
                    name_val,
                    Value::Int(kind),
                ]);
                let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
                emitter.emit("hasLocation", vec![
                    Value::Entity(import_id),
                    Value::Entity(loc_id),
                ]);
                return;
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract a class or interface declaration.
fn extract_class(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    is_interface: bool,
    enclosing_class: Option<EntityId>,
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    if name.is_empty() { return; }

    let class_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    // Use a dummy package for now (0-entity)
    let pkg_id = emitter.alloc();
    let name_val = emitter.string(&name);
    emitter.emit("classes_or_interfaces", vec![
        Value::Entity(class_id),
        name_val,
        Value::Entity(pkg_id),
        Value::Entity(class_id), // sourceid = self
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(class_id),
        Value::Entity(loc_id),
    ]);

    if is_interface {
        emitter.emit("isInterface", vec![
            Value::Entity(class_id),
        ]);
    }

    if node.kind() == "record_declaration" {
        emitter.emit("isRecord", vec![
            Value::Entity(class_id),
        ]);
    }

    // Enclosing type
    if let Some(parent_id) = enclosing_class {
        emitter.emit("enclInReftype", vec![
            Value::Entity(class_id),
            Value::Entity(parent_id),
        ]);
    }

    // Extract modifiers
    extract_modifiers(emitter, file_id, node, source, class_id);

    // Extract superclass
    if let Some(superclass) = node.child_by_field("superclass") {
        extract_extends(emitter, file_id, class_id, &superclass, source);
    }

    // Extract interfaces
    if let Some(interfaces) = node.child_by_field("interfaces") {
        extract_implements(emitter, file_id, class_id, &interfaces, source);
    }

    // Also check super_interfaces for interface declarations
    if let Some(super_interfaces) = node.child_by_field("super_interfaces") {
        extract_implements(emitter, file_id, class_id, &super_interfaces, source);
    }

    // Extract type parameters
    if let Some(type_params) = node.child_by_field("type_parameters") {
        extract_type_parameters(emitter, file_id, class_id, &type_params, source);
    }

    // Extract body
    if let Some(body) = node.child_by_field("body") {
        extract_class_body(emitter, file_id, class_id, &body, source);
    }
}

/// Extract superclass from a `superclass` field node.
fn extract_extends(
    emitter: &mut FactEmitter<'_>,
    _file_id: EntityId,
    class_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "type_identifier" || child.kind() == "scoped_type_identifier"
                || child.kind() == "generic_type"
            {
                let type_name = extract_type_name(&child, source);
                let super_id = emitter.alloc();
                let dummy_pkg = emitter.alloc();
                let name_val = emitter.string(&type_name);
                emitter.emit("classes_or_interfaces", vec![
                    Value::Entity(super_id),
                    name_val,
                    Value::Entity(dummy_pkg),
                    Value::Entity(super_id),
                ]);
                emitter.emit("extendsReftype", vec![
                    Value::Entity(class_id),
                    Value::Entity(super_id),
                ]);
                return;
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract implemented interfaces from an `interfaces` or `super_interfaces` field.
fn extract_implements(
    emitter: &mut FactEmitter<'_>,
    _file_id: EntityId,
    class_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    // The node is a `type_list` containing type identifiers
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "type_identifier" || child.kind() == "scoped_type_identifier"
                || child.kind() == "generic_type"
            {
                let type_name = extract_type_name(&child, source);
                let iface_id = emitter.alloc();
                let dummy_pkg = emitter.alloc();
                let name_val = emitter.string(&type_name);
                emitter.emit("classes_or_interfaces", vec![
                    Value::Entity(iface_id),
                    name_val,
                    Value::Entity(dummy_pkg),
                    Value::Entity(iface_id),
                ]);
                emitter.emit("isInterface", vec![
                    Value::Entity(iface_id),
                ]);
                emitter.emit("implInterface", vec![
                    Value::Entity(class_id),
                    Value::Entity(iface_id),
                ]);
            }
            // Also check type_list children
            if child.kind() == "type_list" {
                extract_implements(emitter, _file_id, class_id, &child, source);
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract type name from various type nodes.
fn extract_type_name(node: &Node<'_>, source: &[u8]) -> String {
    match node.kind() {
        "generic_type" => {
            // Just return the base type name, not the type arguments
            if let Some(name) = node.child(0) {
                name.text(source).to_string()
            } else {
                node.text(source).to_string()
            }
        }
        _ => node.text(source).to_string(),
    }
}

/// Extract full type text (including generics, arrays, etc.)
fn extract_full_type(node: &Node<'_>, source: &[u8]) -> String {
    node.text(source).to_string()
}

/// Extract an enum declaration.
fn extract_enum(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    enclosing_class: Option<EntityId>,
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    if name.is_empty() { return; }

    let class_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let pkg_id = emitter.alloc();
    let name_val = emitter.string(&name);
    emitter.emit("classes_or_interfaces", vec![
        Value::Entity(class_id),
        name_val,
        Value::Entity(pkg_id),
        Value::Entity(class_id),
    ]);
    emitter.emit("isEnumType", vec![
        Value::Entity(class_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(class_id),
        Value::Entity(loc_id),
    ]);

    if let Some(parent_id) = enclosing_class {
        emitter.emit("enclInReftype", vec![
            Value::Entity(class_id),
            Value::Entity(parent_id),
        ]);
    }

    extract_modifiers(emitter, file_id, node, source, class_id);

    // Extract interfaces
    if let Some(interfaces) = node.child_by_field("interfaces") {
        extract_implements(emitter, file_id, class_id, &interfaces, source);
    }

    // Extract enum body
    if let Some(body) = node.child_by_field("body") {
        extract_enum_body(emitter, file_id, class_id, &body, source);
    }
}

/// Extract enum body (constants and members).
fn extract_enum_body(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    class_id: EntityId,
    body: &Node<'_>,
    source: &[u8],
) {
    let mut cursor = body.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "enum_constant" => {
                    let name = child.child_by_field("name")
                        .map(|n| n.text(source).to_string())
                        .unwrap_or_default();
                    if !name.is_empty() {
                        let field_id = emitter.alloc();
                        let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                        let name_val = emitter.string(&name);
                        let type_val = emitter.string("<enum>");
                        emitter.emit("fields", vec![
                            Value::Entity(field_id),
                            name_val,
                            type_val,
                            Value::Entity(class_id),
                        ]);
                        emitter.emit("isEnumConst", vec![
                            Value::Entity(field_id),
                        ]);
                        emitter.emit("hasLocation", vec![
                            Value::Entity(field_id),
                            Value::Entity(loc_id),
                        ]);
                    }
                }
                "enum_body_declarations" => {
                    // Regular class body inside enum
                    extract_class_body_children(emitter, file_id, class_id, &child, source);
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract an annotation type declaration.
fn extract_annotation_type(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    enclosing_class: Option<EntityId>,
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    if name.is_empty() { return; }

    let class_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let pkg_id = emitter.alloc();
    let name_val = emitter.string(&name);
    emitter.emit("classes_or_interfaces", vec![
        Value::Entity(class_id),
        name_val,
        Value::Entity(pkg_id),
        Value::Entity(class_id),
    ]);
    emitter.emit("isInterface", vec![
        Value::Entity(class_id),
    ]);
    emitter.emit("isAnnotType", vec![
        Value::Entity(class_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(class_id),
        Value::Entity(loc_id),
    ]);

    if let Some(parent_id) = enclosing_class {
        emitter.emit("enclInReftype", vec![
            Value::Entity(class_id),
            Value::Entity(parent_id),
        ]);
    }
}

/// Extract the body of a class/interface.
fn extract_class_body(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    class_id: EntityId,
    body: &Node<'_>,
    source: &[u8],
) {
    extract_class_body_children(emitter, file_id, class_id, body, source);
}

fn extract_class_body_children(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    class_id: EntityId,
    body: &Node<'_>,
    source: &[u8],
) {
    let mut cursor = body.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "field_declaration" => {
                    extract_field(emitter, file_id, class_id, &child, source);
                }
                "method_declaration" => {
                    extract_method(emitter, file_id, class_id, &child, source);
                }
                "constructor_declaration" => {
                    extract_constructor(emitter, file_id, class_id, &child, source);
                }
                "class_declaration" => {
                    extract_class(emitter, file_id, &child, source, false, Some(class_id));
                }
                "interface_declaration" => {
                    extract_class(emitter, file_id, &child, source, true, Some(class_id));
                }
                "enum_declaration" => {
                    extract_enum(emitter, file_id, &child, source, Some(class_id));
                }
                "annotation_type_declaration" => {
                    extract_annotation_type(emitter, file_id, &child, source, Some(class_id));
                }
                "record_declaration" => {
                    extract_class(emitter, file_id, &child, source, false, Some(class_id));
                }
                "line_comment" | "block_comment" => {
                    extract_comment(emitter, file_id, &child, source);
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract a field declaration.
fn extract_field(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    class_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    // Get the type
    let type_text = node.child_by_field("type")
        .map(|t| extract_full_type(&t, source))
        .unwrap_or_else(|| "unknown".to_string());

    // Get declarators (there can be multiple: `int x, y;`)
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "variable_declarator" {
                let name = child.child_by_field("name")
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
                        Value::Entity(class_id),
                    ]);
                    emitter.emit("hasLocation", vec![
                        Value::Entity(field_id),
                        Value::Entity(loc_id),
                    ]);
                    extract_modifiers(emitter, file_id, node, source, field_id);
                    extract_annotations_on(emitter, file_id, node, source, field_id);
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract a method declaration.
fn extract_method(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    class_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    if name.is_empty() { return; }

    let return_type = node.child_by_field("type")
        .map(|t| extract_full_type(&t, source))
        .unwrap_or_else(|| "void".to_string());

    let method_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    // Build signature from parameter types
    let sig = build_method_signature(&name, node, source);

    let name_val = emitter.string(&name);
    let sig_val = emitter.string(&sig);
    let type_val = emitter.string(&return_type);
    emitter.emit("methods", vec![
        Value::Entity(method_id),
        name_val,
        sig_val,
        type_val,
        Value::Entity(class_id),
        Value::Entity(method_id), // sourceid = self
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(method_id),
        Value::Entity(loc_id),
    ]);

    extract_modifiers(emitter, file_id, node, source, method_id);

    // Extract annotations on the method
    extract_annotations_on(emitter, file_id, node, source, method_id);

    // Type parameters
    if let Some(type_params) = node.child_by_field("type_parameters") {
        extract_type_parameters(emitter, file_id, method_id, &type_params, source);
    }

    // Parameters
    if let Some(params) = node.child_by_field("parameters") {
        extract_parameters(emitter, file_id, method_id, &params, source);
    }

    // Body
    if let Some(body) = node.child_by_field("body") {
        extract_method_body(emitter, file_id, method_id, &body, source);
    }
}

/// Extract a constructor declaration.
fn extract_constructor(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    class_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    if name.is_empty() { return; }

    let constr_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let sig = build_method_signature(&name, node, source);

    let name_val = emitter.string(&name);
    let sig_val = emitter.string(&sig);
    let type_val = emitter.string(&name); // constructor "returns" its type
    emitter.emit("constrs", vec![
        Value::Entity(constr_id),
        name_val,
        sig_val,
        type_val,
        Value::Entity(class_id),
        Value::Entity(constr_id), // sourceid = self
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(constr_id),
        Value::Entity(loc_id),
    ]);

    extract_modifiers(emitter, file_id, node, source, constr_id);

    if let Some(params) = node.child_by_field("parameters") {
        extract_parameters(emitter, file_id, constr_id, &params, source);
    }

    if let Some(body) = node.child_by_field("body") {
        extract_method_body(emitter, file_id, constr_id, &body, source);
    }
}

/// Build a method signature string like "methodName(int, String)".
fn build_method_signature(name: &str, node: &Node<'_>, source: &[u8]) -> String {
    let mut param_types = Vec::new();
    if let Some(params) = node.child_by_field("parameters") {
        let mut cursor = params.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "formal_parameter" || child.kind() == "spread_parameter" {
                    if let Some(type_node) = child.child_by_field("type") {
                        param_types.push(extract_full_type(&type_node, source));
                    }
                }
                if !cursor.goto_next_sibling() { break; }
            }
        }
    }
    format!("{}({})", name, param_types.join(", "))
}

/// Extract parameters from a formal_parameters node.
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
            if child.kind() == "formal_parameter" || child.kind() == "spread_parameter" {
                let type_text = child.child_by_field("type")
                    .map(|t| extract_full_type(&t, source))
                    .unwrap_or_else(|| "unknown".to_string());
                let name = child.child_by_field("name")
                    .map(|n| n.text(source).to_string())
                    .unwrap_or_default();

                let param_id = emitter.alloc();
                let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                let type_val = emitter.string(&type_text);
                emitter.emit("params", vec![
                    Value::Entity(param_id),
                    type_val,
                    Value::Int(index),
                    Value::Entity(callable_id),
                    Value::Entity(param_id), // sourceid = self
                ]);

                if !name.is_empty() {
                    let name_val = emitter.string(&name);
                    emitter.emit("paramName", vec![
                        Value::Entity(param_id),
                        name_val,
                    ]);
                }

                emitter.emit("hasLocation", vec![
                    Value::Entity(param_id),
                    Value::Entity(loc_id),
                ]);

                index += 1;
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract modifiers from a declaration node.
fn extract_modifiers(
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
                // Recurse into the modifiers node
                let mut inner = child.walk();
                if inner.goto_first_child() {
                    loop {
                        let mod_child = inner.node();
                        let mod_text = mod_child.text(source);
                        // Check both named and unnamed children for modifier keywords
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
                        if !inner.goto_next_sibling() { break; }
                    }
                }
                return;
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

fn is_modifier_text(text: &str) -> bool {
    matches!(text,
        "public" | "private" | "protected" | "static" | "final"
        | "abstract" | "synchronized" | "native" | "transient"
        | "volatile" | "strictfp" | "default" | "sealed"
    )
}

/// Extract annotations on a declaration.
fn extract_annotations_on(
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
            if child.kind() == "modifiers" {
                let mut inner = child.walk();
                if inner.goto_first_child() {
                    loop {
                        let mod_child = inner.node();
                        if mod_child.kind() == "marker_annotation"
                            || mod_child.kind() == "annotation"
                        {
                            let ann_name = mod_child.child_by_field("name")
                                .map(|n| n.text(source).to_string())
                                .unwrap_or_else(|| mod_child.text(source).to_string());
                            let ann_id = emitter.alloc();
                            let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &mod_child);
                            let name_val = emitter.string(&ann_name);
                            emitter.emit("annotations", vec![
                                Value::Entity(ann_id),
                                Value::Entity(parent_id),
                                name_val,
                            ]);
                            emitter.emit("hasLocation", vec![
                                Value::Entity(ann_id),
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

/// Extract type parameters from a type_parameters node.
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
                let name = child.child_by_field("name")
                    .or_else(|| child.child(0).filter(|c| c.kind() == "type_identifier"))
                    .map(|n| n.text(source).to_string())
                    .unwrap_or_default();
                if !name.is_empty() {
                    let tvar_id = emitter.alloc();
                    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                    let name_val = emitter.string(&name);
                    emitter.emit("typeVars", vec![
                        Value::Entity(tvar_id),
                        name_val,
                        Value::Int(index),
                        Value::Entity(parent_id),
                    ]);
                    emitter.emit("hasLocation", vec![
                        Value::Entity(tvar_id),
                        Value::Entity(loc_id),
                    ]);

                    // Extract bounds — type_bound is a direct child, not a field
                    let mut inner = child.walk();
                    if inner.goto_first_child() {
                        loop {
                            let tc = inner.node();
                            if tc.kind() == "type_bound" {
                                extract_type_bounds(emitter, file_id, tvar_id, &tc, source);
                            }
                            if !inner.goto_next_sibling() { break; }
                        }
                    }

                    index += 1;
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract type bounds from a type_bound node.
fn extract_type_bounds(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    tvar_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let mut index = 0i64;
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "type_identifier" || child.kind() == "scoped_type_identifier"
                || child.kind() == "generic_type"
            {
                let bound_id = emitter.alloc();
                let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                let type_name = extract_full_type(&child, source);
                let name_val = emitter.string(&type_name);
                emitter.emit("typeBounds", vec![
                    Value::Entity(bound_id),
                    name_val,
                    Value::Int(index),
                    Value::Entity(tvar_id),
                ]);
                emitter.emit("hasLocation", vec![
                    Value::Entity(bound_id),
                    Value::Entity(loc_id),
                ]);
                index += 1;
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
    if body.kind() == "block" || body.kind() == "constructor_body" {
        extract_stmt(emitter, file_id, body, source, callable_id, callable_id, 0);
    }
}

/// Extract a statement, returning the entity ID.
fn extract_stmt(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    callable_id: EntityId,
    parent_id: EntityId,
    index: i64,
) -> Option<EntityId> {
    let kind = match node.kind() {
        "block" | "constructor_body" => Some(STMT_BLOCK),
        "if_statement" => Some(STMT_IF),
        "for_statement" => Some(STMT_FOR),
        "enhanced_for_statement" => Some(STMT_ENHANCED_FOR),
        "while_statement" => Some(STMT_WHILE),
        "do_statement" => Some(STMT_DO),
        "try_statement" | "try_with_resources_statement" => Some(STMT_TRY),
        "switch_expression" => Some(STMT_SWITCH),
        "return_statement" => Some(STMT_RETURN),
        "throw_statement" => Some(STMT_THROW),
        "break_statement" => Some(STMT_BREAK),
        "continue_statement" => Some(STMT_CONTINUE),
        "empty_statement" => Some(STMT_EMPTY),
        "expression_statement" => Some(STMT_EXPR),
        "labeled_statement" => Some(STMT_LABELED),
        "assert_statement" => Some(STMT_ASSERT),
        "local_variable_declaration" => Some(STMT_LOCAL_VAR_DECL),
        "catch_clause" => Some(STMT_CATCH),
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
        Value::Entity(callable_id),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(stmt_id),
        Value::Entity(loc_id),
    ]);

    // Process children
    match node.kind() {
        "block" | "constructor_body" => {
            let mut child_index = 0i64;
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        if let Some(_) = extract_stmt(
                            emitter, file_id, &child, source, callable_id, stmt_id, child_index,
                        ) {
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
                        extract_expr(emitter, file_id, &child, source, callable_id, stmt_id, 0);
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "if_statement" => {
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, callable_id, stmt_id, 0);
            }
            if let Some(consequence) = node.child_by_field("consequence") {
                extract_stmt(emitter, file_id, &consequence, source, callable_id, stmt_id, 0);
            }
            if let Some(alternative) = node.child_by_field("alternative") {
                extract_stmt(emitter, file_id, &alternative, source, callable_id, stmt_id, 1);
            }
        }
        "for_statement" => {
            if let Some(body) = node.child_by_field("body") {
                extract_stmt(emitter, file_id, &body, source, callable_id, stmt_id, 0);
            }
        }
        "enhanced_for_statement" => {
            // Extract the local variable
            if let Some(name) = node.child_by_field("name") {
                let type_text = node.child_by_field("type")
                    .map(|t| extract_full_type(&t, source))
                    .unwrap_or_else(|| "unknown".to_string());
                let var_id = emitter.alloc();
                let name_val = emitter.string(name.text(source));
                let type_val = emitter.string(&type_text);
                emitter.emit("localvars", vec![
                    Value::Entity(var_id),
                    name_val,
                    type_val,
                    Value::Entity(stmt_id),
                ]);
            }
            if let Some(body) = node.child_by_field("body") {
                extract_stmt(emitter, file_id, &body, source, callable_id, stmt_id, 0);
            }
        }
        "while_statement" | "do_statement" => {
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, callable_id, stmt_id, 0);
            }
            if let Some(body) = node.child_by_field("body") {
                extract_stmt(emitter, file_id, &body, source, callable_id, stmt_id, 0);
            }
        }
        "return_statement" => {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, &child, source, callable_id, stmt_id, 0);
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "throw_statement" => {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, &child, source, callable_id, stmt_id, 0);
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "local_variable_declaration" => {
            // Extract local variable declarations
            let type_text = node.child_by_field("type")
                .map(|t| extract_full_type(&t, source))
                .unwrap_or_else(|| "unknown".to_string());
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "variable_declarator" {
                        let name = child.child_by_field("name")
                            .map(|n| n.text(source).to_string())
                            .unwrap_or_default();
                        if !name.is_empty() {
                            let var_id = emitter.alloc();
                            let name_val = emitter.string(&name);
                            let type_val = emitter.string(&type_text);
                            emitter.emit("localvars", vec![
                                Value::Entity(var_id),
                                name_val,
                                type_val,
                                Value::Entity(stmt_id),
                            ]);
                            let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                            emitter.emit("hasLocation", vec![
                                Value::Entity(var_id),
                                Value::Entity(loc_id),
                            ]);

                            // Extract initializer expression
                            if let Some(value) = child.child_by_field("value") {
                                extract_expr(emitter, file_id, &value, source, callable_id, stmt_id, 0);
                            }
                        }
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "try_statement" | "try_with_resources_statement" => {
            if let Some(body) = node.child_by_field("body") {
                extract_stmt(emitter, file_id, &body, source, callable_id, stmt_id, 0);
            }
            // Catch clauses
            let mut catch_idx = 1i64;
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "catch_clause" {
                        extract_stmt(emitter, file_id, &child, source, callable_id, stmt_id, catch_idx);
                        catch_idx += 1;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "catch_clause" => {
            if let Some(body) = node.child_by_field("body") {
                extract_stmt(emitter, file_id, &body, source, callable_id, stmt_id, 0);
            }
        }
        "labeled_statement" => {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() && child.kind() != "identifier" {
                        extract_stmt(emitter, file_id, &child, source, callable_id, stmt_id, 0);
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
    callable_id: EntityId,
    parent_id: EntityId,
    index: i64,
) -> Option<EntityId> {
    let kind = match node.kind() {
        "decimal_integer_literal" | "hex_integer_literal" | "octal_integer_literal"
        | "binary_integer_literal" => Some(EXPR_INT_LIT),
        "decimal_floating_point_literal" | "hex_floating_point_literal" => Some(EXPR_DOUBLE_LIT),
        "character_literal" => Some(EXPR_CHAR_LIT),
        "string_literal" => Some(EXPR_STRING_LIT),
        "true" | "false" => Some(EXPR_BOOLEAN_LIT),
        "null_literal" => Some(EXPR_NULL_LIT),
        "identifier" => Some(EXPR_VARACCESS),
        "this" => Some(EXPR_THIS),
        "super" => Some(EXPR_SUPER),
        "method_invocation" => Some(EXPR_METHODACCESS),
        "object_creation_expression" => Some(EXPR_NEW),
        "field_access" => Some(EXPR_VARACCESS),
        "array_access" => Some(EXPR_ARRAY_ACCESS),
        "array_creation_expression" => Some(EXPR_ARRAY_CREATION),
        "array_initializer" => Some(EXPR_ARRAY_INIT),
        "cast_expression" => Some(EXPR_CAST),
        "instanceof_expression" => Some(EXPR_INSTANCEOF),
        "ternary_expression" => Some(EXPR_CONDITIONAL),
        "parenthesized_expression" => {
            // Extract inner expression directly
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        return extract_expr(emitter, file_id, &child, source, callable_id, parent_id, index);
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
            return None;
        }
        "binary_expression" => {
            let op = find_java_operator(node, source);
            Some(java_binary_op_kind(&op))
        }
        "unary_expression" => {
            let op = find_java_operator(node, source);
            match op.as_str() {
                "!" => Some(EXPR_LOGNOT),
                "~" => Some(EXPR_BITNOT),
                "-" => Some(EXPR_MINUS),
                "+" => Some(EXPR_PLUS),
                _ => Some(EXPR_MINUS),
            }
        }
        "update_expression" => {
            let op = find_java_operator(node, source);
            let is_prefix = node.child(0)
                .map(|c| !c.is_named())
                .unwrap_or(false);
            match (op.as_str(), is_prefix) {
                ("++", true) => Some(EXPR_PREINC),
                ("++", false) => Some(EXPR_POSTINC),
                ("--", true) => Some(EXPR_PREDEC),
                ("--", false) => Some(EXPR_POSTDEC),
                _ => Some(EXPR_PREINC),
            }
        }
        "assignment_expression" => {
            let op = find_java_operator(node, source);
            Some(java_assign_op_kind(&op))
        }
        "lambda_expression" => Some(EXPR_LAMBDA),
        _ => None,
    };

    let expr_kind = match kind {
        Some(k) => k,
        None => return None,
    };

    let expr_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let type_val = emitter.string("unknown"); // We don't resolve types yet
    emitter.emit("exprs", vec![
        Value::Entity(expr_id),
        Value::Int(expr_kind),
        type_val,
        Value::Entity(parent_id),
        Value::Int(index),
    ]);

    emitter.emit("callableEnclosingExpr", vec![
        Value::Entity(expr_id),
        Value::Entity(callable_id),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(expr_id),
        Value::Entity(loc_id),
    ]);

    // Recurse into children
    match node.kind() {
        "method_invocation" => {
            if let Some(obj) = node.child_by_field("object") {
                extract_expr(emitter, file_id, &obj, source, callable_id, expr_id, 0);
            }
            if let Some(args) = node.child_by_field("arguments") {
                let mut idx = 0i64;
                let mut cursor = args.walk();
                if cursor.goto_first_child() {
                    loop {
                        let child = cursor.node();
                        if child.is_named() {
                            extract_expr(emitter, file_id, &child, source, callable_id, expr_id, idx);
                            idx += 1;
                        }
                        if !cursor.goto_next_sibling() { break; }
                    }
                }
            }
        }
        "object_creation_expression" => {
            if let Some(args) = node.child_by_field("arguments") {
                let mut idx = 0i64;
                let mut cursor = args.walk();
                if cursor.goto_first_child() {
                    loop {
                        let child = cursor.node();
                        if child.is_named() {
                            extract_expr(emitter, file_id, &child, source, callable_id, expr_id, idx);
                            idx += 1;
                        }
                        if !cursor.goto_next_sibling() { break; }
                    }
                }
            }
        }
        "binary_expression" => {
            if let Some(left) = node.child_by_field("left") {
                extract_expr(emitter, file_id, &left, source, callable_id, expr_id, 0);
            }
            if let Some(right) = node.child_by_field("right") {
                extract_expr(emitter, file_id, &right, source, callable_id, expr_id, 1);
            }
        }
        "assignment_expression" => {
            if let Some(left) = node.child_by_field("left") {
                extract_expr(emitter, file_id, &left, source, callable_id, expr_id, 0);
            }
            if let Some(right) = node.child_by_field("right") {
                extract_expr(emitter, file_id, &right, source, callable_id, expr_id, 1);
            }
        }
        "unary_expression" | "update_expression" => {
            if let Some(operand) = node.child_by_field("operand") {
                extract_expr(emitter, file_id, &operand, source, callable_id, expr_id, 0);
            }
        }
        "ternary_expression" => {
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, callable_id, expr_id, 0);
            }
            if let Some(consequence) = node.child_by_field("consequence") {
                extract_expr(emitter, file_id, &consequence, source, callable_id, expr_id, 1);
            }
            if let Some(alternative) = node.child_by_field("alternative") {
                extract_expr(emitter, file_id, &alternative, source, callable_id, expr_id, 2);
            }
        }
        "field_access" => {
            if let Some(obj) = node.child_by_field("object") {
                extract_expr(emitter, file_id, &obj, source, callable_id, expr_id, 0);
            }
        }
        "cast_expression" => {
            if let Some(value) = node.child_by_field("value") {
                extract_expr(emitter, file_id, &value, source, callable_id, expr_id, 0);
            }
        }
        _ => {}
    }

    Some(expr_id)
}

fn find_java_operator(node: &Node<'_>, source: &[u8]) -> String {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if !child.is_named() {
                let text = child.text(source);
                match text {
                    "+" | "-" | "*" | "/" | "%" | "&" | "|" | "^" | "<<" | ">>" | ">>>"
                    | "==" | "!=" | "<" | ">" | "<=" | ">=" | "&&" | "||"
                    | "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^="
                    | "<<=" | ">>=" | ">>>=" | "!" | "~" | "++" | "--" => {
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

fn java_binary_op_kind(op: &str) -> i64 {
    match op {
        "*" => EXPR_MUL,
        "/" => EXPR_DIV,
        "%" => EXPR_REM,
        "+" => EXPR_ADD,
        "-" => EXPR_SUB,
        "<<" => EXPR_LSHIFT,
        ">>" => EXPR_RSHIFT,
        ">>>" => EXPR_URSHIFT,
        "&" => EXPR_ANDBIT,
        "|" => EXPR_ORBIT,
        "^" => EXPR_XORBIT,
        "&&" => EXPR_ANDLOG,
        "||" => EXPR_ORLOG,
        "<" => EXPR_LT,
        ">" => EXPR_GT,
        "<=" => EXPR_LE,
        ">=" => EXPR_GE,
        "==" => EXPR_EQ,
        "!=" => EXPR_NE,
        _ => EXPR_ADD,
    }
}

fn java_assign_op_kind(op: &str) -> i64 {
    match op {
        "=" => EXPR_ASSIGN,
        "+=" => EXPR_ASSIGNADD,
        "-=" => EXPR_ASSIGNSUB,
        "*=" => EXPR_ASSIGNMUL,
        "/=" => EXPR_ASSIGNDIV,
        "%=" => EXPR_ASSIGNREM,
        "&=" => EXPR_ASSIGNAND,
        "|=" => EXPR_ASSIGNOR,
        "^=" => EXPR_ASSIGNXOR,
        "<<=" => EXPR_ASSIGNLSHIFT,
        ">>=" => EXPR_ASSIGNRSHIFT,
        ">>>=" => EXPR_ASSIGNURSHIFT,
        _ => EXPR_ASSIGN,
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

#[cfg(test)]
mod tests {
    use super::*;
    use ocql_database::Database;
    use crate::schema::java_schema;

    fn extract_test_file(filename: &str) -> Database {
        let source = std::fs::read(
            format!("tests/fixtures/{}", filename)
        ).unwrap();
        let schema = java_schema();
        let mut db = Database::from_schema(schema);
        let extractor = JavaExtractor::new();
        let result = extractor.extract_source(&mut db, filename, &source);
        assert!(result.success, "Extraction failed for {}: {:?}", filename, result.error);
        db
    }

    #[test]
    fn test_simple_classes() {
        let db = extract_test_file("Simple.java");
        let classes: Vec<_> = db.scan("classes_or_interfaces").unwrap().collect();
        let names: Vec<_> = classes.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Classes: {:?}", names);
        assert!(names.contains(&"Simple"), "Should find 'Simple'");
    }

    #[test]
    fn test_simple_methods() {
        let db = extract_test_file("Simple.java");
        let methods: Vec<_> = db.scan("methods").unwrap().collect();
        let names: Vec<_> = methods.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Methods: {:?}", names);
        assert!(names.contains(&"getCount"), "Should find 'getCount'");
        assert!(names.contains(&"increment"), "Should find 'increment'");
        assert!(names.contains(&"factorial"), "Should find 'factorial'");
        assert!(names.contains(&"main"), "Should find 'main'");
    }

    #[test]
    fn test_simple_constructors() {
        let db = extract_test_file("Simple.java");
        let constrs: Vec<_> = db.scan("constrs").unwrap().collect();
        let names: Vec<_> = constrs.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Constructors: {:?}", names);
        assert!(names.contains(&"Simple"), "Should find 'Simple' constructor");
    }

    #[test]
    fn test_simple_fields() {
        let db = extract_test_file("Simple.java");
        let fields: Vec<_> = db.scan("fields").unwrap().collect();
        let names: Vec<_> = fields.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Fields: {:?}", names);
        assert!(names.contains(&"count"), "Should find 'count'");
        assert!(names.contains(&"name"), "Should find 'name'");
    }

    #[test]
    fn test_simple_params() {
        let db = extract_test_file("Simple.java");
        let params: Vec<_> = db.scan("params").unwrap().collect();
        let param_names: Vec<_> = db.scan("paramName").unwrap().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap()).to_string()
        }).collect();
        eprintln!("Params: {} total, names: {:?}", params.len(), param_names);
        assert!(param_names.contains(&"n".to_string()), "Should find param 'n'");
        assert!(param_names.contains(&"args".to_string()), "Should find param 'args'");
    }

    #[test]
    fn test_simple_imports() {
        let db = extract_test_file("Simple.java");
        let imports: Vec<_> = db.scan("imports").unwrap().collect();
        let names: Vec<_> = imports.iter().map(|t| {
            db.strings.resolve(t[2].as_string().unwrap())
        }).collect();
        eprintln!("Imports: {:?}", names);
        assert!(names.iter().any(|n| n.contains("List")), "Should find List import");
        assert!(names.iter().any(|n| n.contains("ArrayList")), "Should find ArrayList import");
    }

    #[test]
    fn test_simple_package() {
        let db = extract_test_file("Simple.java");
        let packages: Vec<_> = db.scan("packages").unwrap().collect();
        let names: Vec<_> = packages.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Packages: {:?}", names);
        assert!(names.iter().any(|n| n.contains("com.example")), "Should find com.example package");
    }

    #[test]
    fn test_simple_statements() {
        let db = extract_test_file("Simple.java");
        let stmts: Vec<_> = db.scan("stmts").unwrap().collect();
        let kinds: Vec<i64> = stmts.iter().map(|t| t[1].as_int().unwrap()).collect();
        eprintln!("Statement kinds: {:?}", kinds);
        assert!(kinds.contains(&STMT_BLOCK), "Should have block statements");
        assert!(kinds.contains(&STMT_RETURN), "Should have return statements");
        assert!(kinds.contains(&STMT_IF), "Should have if statements");
    }

    #[test]
    fn test_simple_expressions() {
        let db = extract_test_file("Simple.java");
        let exprs: Vec<_> = db.scan("exprs").unwrap().collect();
        let kinds: Vec<i64> = exprs.iter().map(|t| t[1].as_int().unwrap()).collect();
        eprintln!("Expression kinds: {:?}", kinds);
        assert!(kinds.contains(&EXPR_METHODACCESS), "Should have method calls");
        assert!(kinds.contains(&EXPR_NEW), "Should have new expressions");
    }

    #[test]
    fn test_simple_local_vars() {
        let db = extract_test_file("Simple.java");
        let locals: Vec<_> = db.scan("localvars").unwrap().collect();
        let names: Vec<_> = locals.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Local vars: {:?}", names);
        assert!(names.contains(&"s"), "Should find local 's'");
        assert!(names.contains(&"result"), "Should find local 'result'");
    }

    #[test]
    fn test_inheritance_classes() {
        let db = extract_test_file("Inheritance.java");
        let classes: Vec<_> = db.scan("classes_or_interfaces").unwrap().collect();
        let names: Vec<_> = classes.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Classes: {:?}", names);
        assert!(names.contains(&"Shape"), "Should find 'Shape'");
        assert!(names.contains(&"Circle"), "Should find 'Circle'");
        assert!(names.contains(&"Rectangle"), "Should find 'Rectangle'");
        assert!(names.contains(&"Drawable"), "Should find 'Drawable'");
        assert!(names.contains(&"Resizable"), "Should find 'Resizable'");
    }

    #[test]
    fn test_inheritance_extends() {
        let db = extract_test_file("Inheritance.java");
        let extends: Vec<_> = db.scan("extendsReftype").unwrap().collect();
        eprintln!("Extends: {} entries", extends.len());
        assert!(extends.len() >= 2, "Should have >= 2 extends entries (Circle, Rectangle extend Shape)");
    }

    #[test]
    fn test_inheritance_implements() {
        let db = extract_test_file("Inheritance.java");
        let impls: Vec<_> = db.scan("implInterface").unwrap().collect();
        eprintln!("implInterface: {} entries", impls.len());
        assert!(impls.len() >= 2, "Should have >= 2 implements entries");
    }

    #[test]
    fn test_enum_extraction() {
        let db = extract_test_file("Enums.java");
        let classes: Vec<_> = db.scan("classes_or_interfaces").unwrap().collect();
        let names: Vec<_> = classes.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        assert!(names.contains(&"Enums"), "Should find 'Enums' enum");

        let enum_types: Vec<_> = db.scan("isEnumType").unwrap().collect();
        assert!(enum_types.len() >= 1, "Should have at least 1 enum type");

        let fields: Vec<_> = db.scan("fields").unwrap().collect();
        let field_names: Vec<_> = fields.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Enum fields: {:?}", field_names);
        assert!(field_names.contains(&"RED"), "Should find enum constant RED");
        assert!(field_names.contains(&"GREEN"), "Should find enum constant GREEN");
        assert!(field_names.contains(&"BLUE"), "Should find enum constant BLUE");
    }

    #[test]
    fn test_generics_type_params() {
        let db = extract_test_file("Generics.java");
        let type_vars: Vec<_> = db.scan("typeVars").unwrap().collect();
        let names: Vec<_> = type_vars.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Type vars: {:?}", names);
        assert!(names.contains(&"T"), "Should find type parameter T");
    }

    #[test]
    fn test_annotations() {
        let db = extract_test_file("Annotations.java");
        let annotations: Vec<_> = db.scan("annotations").unwrap().collect();
        let names: Vec<_> = annotations.iter().map(|t| {
            db.strings.resolve(t[2].as_string().unwrap())
        }).collect();
        eprintln!("Annotations: {:?}", names);
        assert!(names.iter().any(|n| n.contains("Override")), "Should find @Override");
        assert!(names.iter().any(|n| n.contains("Deprecated")), "Should find @Deprecated");
    }
}
