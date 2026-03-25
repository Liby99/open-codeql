use ocql_database::{EntityId, Value};
use ocql_extractor_common::{Extractor, FactEmitter, LocationEmitter, NodeExt};
use ocql_extractor_common::tree_sitter::{Language, Node, Tree};

/// Rust extractor using tree-sitter.
///
/// Extracts:
/// - Modules, use declarations
/// - Functions, structs, enums, traits, impls
/// - Type aliases, consts, statics
/// - Fields, parameters, local variables
/// - Statements and expressions
/// - Macro definitions, attributes
/// - Generic type parameters with trait bounds
/// - Comments
pub struct RustExtractor;

impl RustExtractor {
    pub fn new() -> Self {
        Self
    }
}

// Statement kind constants
const STMT_LET: i64 = 0;
const STMT_EXPR: i64 = 1;
const STMT_RETURN: i64 = 2;
const STMT_IF: i64 = 3;
const STMT_MATCH: i64 = 4;
const STMT_LOOP: i64 = 5;
const STMT_WHILE: i64 = 6;
const STMT_FOR: i64 = 7;
const STMT_BLOCK: i64 = 8;

// Expression kind constants
const EXPR_CALL: i64 = 0;
#[allow(dead_code)]
const EXPR_METHOD_CALL: i64 = 1;
const EXPR_FIELD: i64 = 2;
const EXPR_INDEX: i64 = 3;
const EXPR_PATH: i64 = 4;
const EXPR_LITERAL_INT: i64 = 5;
const EXPR_LITERAL_FLOAT: i64 = 6;
const EXPR_LITERAL_STRING: i64 = 7;
const EXPR_LITERAL_CHAR: i64 = 8;
const EXPR_LITERAL_BOOL: i64 = 9;
const EXPR_BINARY: i64 = 10;
const EXPR_UNARY: i64 = 11;
const EXPR_REFERENCE: i64 = 12;
#[allow(dead_code)]
const EXPR_DEREFERENCE: i64 = 13;
const EXPR_ASSIGNMENT: i64 = 14;
const EXPR_COMPOUND_ASSIGNMENT: i64 = 15;
const EXPR_RANGE: i64 = 16;
const EXPR_CLOSURE: i64 = 17;
const EXPR_IF: i64 = 18;
const EXPR_MATCH: i64 = 19;
const EXPR_BLOCK: i64 = 20;
const EXPR_TUPLE: i64 = 21;
const EXPR_ARRAY: i64 = 22;
const EXPR_STRUCT: i64 = 23;
const EXPR_TRY: i64 = 24;
const EXPR_AWAIT: i64 = 25;
const EXPR_RETURN: i64 = 26;
const EXPR_BREAK: i64 = 27;
const EXPR_CONTINUE: i64 = 28;
const EXPR_MACRO_INVOCATION: i64 = 29;

impl Extractor for RustExtractor {
    fn language(&self) -> Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn extensions(&self) -> &[&str] {
        &["rs"]
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
                extract_item(emitter, file_id, file_id, &node, source);
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// Extract an item (top-level or nested).
fn extract_item(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    parent_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    match node.kind() {
        "mod_item" => extract_module(emitter, file_id, parent_id, node, source),
        "function_item" => extract_function(emitter, file_id, parent_id, node, source),
        "struct_item" => extract_struct(emitter, file_id, parent_id, node, source),
        "enum_item" => extract_enum(emitter, file_id, parent_id, node, source),
        "trait_item" => extract_trait(emitter, file_id, parent_id, node, source),
        "impl_item" => extract_impl(emitter, file_id, parent_id, node, source),
        "type_item" => extract_type_alias(emitter, file_id, parent_id, node, source),
        "const_item" => extract_const(emitter, file_id, parent_id, node, source, false),
        "static_item" => extract_const(emitter, file_id, parent_id, node, source, true),
        "use_declaration" => extract_use(emitter, file_id, parent_id, node, source),
        "macro_definition" => extract_macro_def(emitter, file_id, parent_id, node, source),
        "attribute_item" | "inner_attribute_item" => {
            extract_attribute(emitter, file_id, parent_id, node, source);
        }
        "line_comment" | "block_comment" => {
            extract_comment(emitter, file_id, node, source);
        }
        "expression_statement" => {
            extract_expression_statement(emitter, file_id, parent_id, node, source);
        }
        "let_declaration" => {
            extract_let_declaration(emitter, file_id, parent_id, node, source, 0);
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Module definitions
// ---------------------------------------------------------------------------

fn extract_module(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    parent_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    if name.is_empty() { return; }

    let mod_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    let name_val = emitter.string(&name);
    emitter.emit("rs_modules", vec![
        Value::Entity(mod_id),
        name_val,
        Value::Entity(parent_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(mod_id),
        Value::Entity(loc_id),
    ]);

    // Extract attributes on this module
    extract_preceding_attributes(emitter, file_id, mod_id, node, source);

    // Extract body if present (inline module)
    if let Some(body) = node.child_by_field("body") {
        extract_block_items(emitter, file_id, mod_id, &body, source);
    }
}

// ---------------------------------------------------------------------------
// Function definitions
// ---------------------------------------------------------------------------

fn extract_function(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    parent_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    if name.is_empty() { return; }

    let fn_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    let name_val = emitter.string(&name);
    emitter.emit("rs_functions", vec![
        Value::Entity(fn_id),
        name_val,
        Value::Entity(parent_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(fn_id),
        Value::Entity(loc_id),
    ]);

    // Extract attributes
    extract_preceding_attributes(emitter, file_id, fn_id, node, source);

    // Extract type parameters
    if let Some(type_params) = node.child_by_field("type_parameters") {
        extract_type_parameters(emitter, file_id, fn_id, &type_params, source);
    }

    // Extract parameters
    if let Some(params) = node.child_by_field("parameters") {
        extract_parameters(emitter, file_id, fn_id, &params, source);
    }

    // Extract body
    if let Some(body) = node.child_by_field("body") {
        extract_function_body(emitter, file_id, fn_id, &body, source);
    }
}

// ---------------------------------------------------------------------------
// Struct definitions
// ---------------------------------------------------------------------------

fn extract_struct(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    parent_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    if name.is_empty() { return; }

    let struct_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    let name_val = emitter.string(&name);
    emitter.emit("rs_structs", vec![
        Value::Entity(struct_id),
        name_val,
        Value::Entity(parent_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(struct_id),
        Value::Entity(loc_id),
    ]);

    // Extract attributes
    extract_preceding_attributes(emitter, file_id, struct_id, node, source);

    // Extract type parameters
    if let Some(type_params) = node.child_by_field("type_parameters") {
        extract_type_parameters(emitter, file_id, struct_id, &type_params, source);
    }

    // Extract body (field declarations)
    if let Some(body) = node.child_by_field("body") {
        extract_struct_fields(emitter, file_id, struct_id, &body, source);
    }
}

fn extract_struct_fields(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    struct_id: EntityId,
    body: &Node<'_>,
    source: &[u8],
) {
    let mut cursor = body.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "field_declaration" {
                let name = child.child_by_field("name")
                    .map(|n| n.text(source).to_string())
                    .unwrap_or_default();
                let type_text = child.child_by_field("type")
                    .map(|t| t.text(source).to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                if !name.is_empty() {
                    let field_id = emitter.alloc();
                    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                    let name_val = emitter.string(&name);
                    let type_val = emitter.string(&type_text);
                    emitter.emit("rs_fields", vec![
                        Value::Entity(field_id),
                        name_val,
                        type_val,
                        Value::Entity(struct_id),
                    ]);
                    emitter.emit("hasLocation", vec![
                        Value::Entity(field_id),
                        Value::Entity(loc_id),
                    ]);
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

// ---------------------------------------------------------------------------
// Enum definitions
// ---------------------------------------------------------------------------

fn extract_enum(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    parent_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    if name.is_empty() { return; }

    let enum_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    let name_val = emitter.string(&name);
    emitter.emit("rs_enums", vec![
        Value::Entity(enum_id),
        name_val,
        Value::Entity(parent_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(enum_id),
        Value::Entity(loc_id),
    ]);

    // Extract attributes
    extract_preceding_attributes(emitter, file_id, enum_id, node, source);

    // Extract type parameters
    if let Some(type_params) = node.child_by_field("type_parameters") {
        extract_type_parameters(emitter, file_id, enum_id, &type_params, source);
    }

    // Extract body (variants)
    if let Some(body) = node.child_by_field("body") {
        extract_enum_variants(emitter, file_id, enum_id, &body, source);
    }
}

fn extract_enum_variants(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    enum_id: EntityId,
    body: &Node<'_>,
    source: &[u8],
) {
    let mut cursor = body.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "enum_variant" {
                let name = child.child_by_field("name")
                    .map(|n| n.text(source).to_string())
                    .unwrap_or_default();
                if !name.is_empty() {
                    let variant_id = emitter.alloc();
                    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                    let name_val = emitter.string(&name);
                    emitter.emit("rs_enum_variants", vec![
                        Value::Entity(variant_id),
                        name_val,
                        Value::Entity(enum_id),
                    ]);
                    emitter.emit("hasLocation", vec![
                        Value::Entity(variant_id),
                        Value::Entity(loc_id),
                    ]);

                    // Extract fields on tuple/struct variants
                    if let Some(field_body) = child.child_by_field("body") {
                        extract_variant_fields(emitter, file_id, variant_id, &field_body, source);
                    }
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

fn extract_variant_fields(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    variant_id: EntityId,
    body: &Node<'_>,
    source: &[u8],
) {
    let mut idx = 0;
    let mut cursor = body.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "field_declaration" {
                let name = child.child_by_field("name")
                    .map(|n| n.text(source).to_string())
                    .unwrap_or_else(|| idx.to_string());
                let type_text = child.child_by_field("type")
                    .map(|t| t.text(source).to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                let field_id = emitter.alloc();
                let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                let name_val = emitter.string(&name);
                let type_val = emitter.string(&type_text);
                emitter.emit("rs_fields", vec![
                    Value::Entity(field_id),
                    name_val,
                    type_val,
                    Value::Entity(variant_id),
                ]);
                emitter.emit("hasLocation", vec![
                    Value::Entity(field_id),
                    Value::Entity(loc_id),
                ]);
                idx += 1;
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

// ---------------------------------------------------------------------------
// Trait definitions
// ---------------------------------------------------------------------------

fn extract_trait(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    parent_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    if name.is_empty() { return; }

    let trait_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    let name_val = emitter.string(&name);
    emitter.emit("rs_traits", vec![
        Value::Entity(trait_id),
        name_val,
        Value::Entity(parent_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(trait_id),
        Value::Entity(loc_id),
    ]);

    // Extract attributes
    extract_preceding_attributes(emitter, file_id, trait_id, node, source);

    // Extract type parameters
    if let Some(type_params) = node.child_by_field("type_parameters") {
        extract_type_parameters(emitter, file_id, trait_id, &type_params, source);
    }

    // Extract body (associated items)
    if let Some(body) = node.child_by_field("body") {
        extract_trait_body(emitter, file_id, trait_id, &body, source);
    }
}

fn extract_trait_body(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    trait_id: EntityId,
    body: &Node<'_>,
    source: &[u8],
) {
    let mut cursor = body.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "function_item" | "function_signature_item" => {
                    extract_function(emitter, file_id, trait_id, &child, source);
                }
                "type_item" => {
                    extract_type_alias(emitter, file_id, trait_id, &child, source);
                }
                "const_item" => {
                    extract_const(emitter, file_id, trait_id, &child, source, false);
                }
                "attribute_item" => {
                    extract_attribute(emitter, file_id, trait_id, &child, source);
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

// ---------------------------------------------------------------------------
// Impl blocks
// ---------------------------------------------------------------------------

fn extract_impl(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    parent_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    // Get the type being implemented
    let type_name = node.child_by_field("type")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();

    // Get the trait being implemented (if any)
    let trait_name = node.child_by_field("trait")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();

    let impl_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    let type_val = emitter.string(&type_name);
    let trait_val = emitter.string(&trait_name);
    emitter.emit("rs_impls", vec![
        Value::Entity(impl_id),
        type_val,
        trait_val,
        Value::Entity(parent_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(impl_id),
        Value::Entity(loc_id),
    ]);

    // Extract attributes
    extract_preceding_attributes(emitter, file_id, impl_id, node, source);

    // Extract type parameters
    if let Some(type_params) = node.child_by_field("type_parameters") {
        extract_type_parameters(emitter, file_id, impl_id, &type_params, source);
    }

    // Extract body
    if let Some(body) = node.child_by_field("body") {
        extract_impl_body(emitter, file_id, impl_id, &body, source);
    }
}

fn extract_impl_body(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    impl_id: EntityId,
    body: &Node<'_>,
    source: &[u8],
) {
    let mut cursor = body.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "function_item" => {
                    extract_function(emitter, file_id, impl_id, &child, source);
                }
                "type_item" => {
                    extract_type_alias(emitter, file_id, impl_id, &child, source);
                }
                "const_item" => {
                    extract_const(emitter, file_id, impl_id, &child, source, false);
                }
                "macro_invocation" => {
                    // Macro invocations in impl bodies
                }
                "attribute_item" => {
                    extract_attribute(emitter, file_id, impl_id, &child, source);
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

// ---------------------------------------------------------------------------
// Type aliases
// ---------------------------------------------------------------------------

fn extract_type_alias(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    parent_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    if name.is_empty() { return; }

    let alias_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    let name_val = emitter.string(&name);
    emitter.emit("rs_type_aliases", vec![
        Value::Entity(alias_id),
        name_val,
        Value::Entity(parent_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(alias_id),
        Value::Entity(loc_id),
    ]);
}

// ---------------------------------------------------------------------------
// Const and static items
// ---------------------------------------------------------------------------

fn extract_const(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    parent_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    _is_static: bool,
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    if name.is_empty() { return; }

    let type_text = node.child_by_field("type")
        .map(|t| t.text(source).to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let const_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    let name_val = emitter.string(&name);
    let type_val = emitter.string(&type_text);
    emitter.emit("rs_consts", vec![
        Value::Entity(const_id),
        name_val,
        type_val,
        Value::Entity(parent_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(const_id),
        Value::Entity(loc_id),
    ]);
}

// ---------------------------------------------------------------------------
// Use declarations
// ---------------------------------------------------------------------------

fn extract_use(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    parent_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    // Get the argument child which contains the use path
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "use_as_clause" => {
                    // use path as alias;
                    let path = child.child_by_field("path")
                        .map(|n| n.text(source).to_string())
                        .unwrap_or_default();
                    let alias = child.child_by_field("alias")
                        .map(|n| n.text(source).to_string())
                        .unwrap_or_default();
                    emit_use_decl(emitter, file_id, parent_id, node, &path, &alias);
                    return;
                }
                "scoped_identifier" | "identifier" | "scoped_use_list" | "use_wildcard"
                | "use_list" => {
                    let path = child.text(source).to_string();
                    emit_use_decl(emitter, file_id, parent_id, node, &path, "");
                    return;
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }

    // Fallback: use the full text
    let text = node.text(source);
    let path = text.trim_start_matches("use ").trim_end_matches(';').trim();
    emit_use_decl(emitter, file_id, parent_id, node, path, "");
}

fn emit_use_decl(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    parent_id: EntityId,
    node: &Node<'_>,
    path: &str,
    alias: &str,
) {
    let use_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    let path_val = emitter.string(path);
    let alias_val = emitter.string(alias);
    emitter.emit("rs_use_decls", vec![
        Value::Entity(use_id),
        path_val,
        alias_val,
        Value::Entity(parent_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(use_id),
        Value::Entity(loc_id),
    ]);
}

// ---------------------------------------------------------------------------
// Macro definitions
// ---------------------------------------------------------------------------

fn extract_macro_def(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    parent_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    if name.is_empty() { return; }

    let macro_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    let name_val = emitter.string(&name);
    emitter.emit("rs_macros", vec![
        Value::Entity(macro_id),
        name_val,
        Value::Entity(parent_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(macro_id),
        Value::Entity(loc_id),
    ]);
}

// ---------------------------------------------------------------------------
// Attributes
// ---------------------------------------------------------------------------

fn extract_attribute(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    parent_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    // Extract the attribute name from inside the brackets
    let text = node.text(source);
    let name = extract_attribute_name(text);

    let attr_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    let name_val = emitter.string(&name);
    emitter.emit("rs_attributes", vec![
        Value::Entity(attr_id),
        name_val,
        Value::Entity(parent_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(attr_id),
        Value::Entity(loc_id),
    ]);
}

/// Extract the attribute name from an attribute text like `#[derive(Debug)]`.
fn extract_attribute_name(text: &str) -> String {
    let s = text.trim();
    // Strip #[ or #![
    let inner = if s.starts_with("#![") {
        &s[3..s.len().saturating_sub(1)]
    } else if s.starts_with("#[") {
        &s[2..s.len().saturating_sub(1)]
    } else {
        s
    };
    // Return just the identifier part before any parentheses
    if let Some(paren) = inner.find('(') {
        inner[..paren].trim().to_string()
    } else {
        inner.trim().to_string()
    }
}

/// Extract attributes from preceding siblings of a node.
fn extract_preceding_attributes(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    item_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    // Look at children of the node for attribute_item
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "attribute_item" || child.kind() == "inner_attribute_item" {
                extract_attribute(emitter, file_id, item_id, &child, source);
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

// ---------------------------------------------------------------------------
// Generic type parameters
// ---------------------------------------------------------------------------

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
            match child.kind() {
                "type_parameter" => {
                    // tree-sitter-rust: type_parameter has "name" field (type_identifier)
                    // and optional "bounds" field (trait_bounds)
                    let name = child.child_by_field("name")
                        .map(|n| n.text(source).to_string())
                        .unwrap_or_default();
                    if !name.is_empty() {
                        let gen_id = emitter.alloc();
                        let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                        let name_val = emitter.string(&name);
                        emitter.emit("rs_generics", vec![
                            Value::Entity(gen_id),
                            name_val,
                            Value::Int(index),
                            Value::Entity(parent_id),
                        ]);
                        emitter.emit("hasLocation", vec![
                            Value::Entity(gen_id),
                            Value::Entity(loc_id),
                        ]);

                        // Extract trait bounds if present
                        if let Some(bounds) = child.child_by_field("bounds") {
                            extract_trait_bounds(emitter, file_id, gen_id, &bounds, source);
                        }

                        index += 1;
                    }
                }
                "constrained_type_parameter" => {
                    // Fallback for older grammar versions
                    let name = child.child_by_field("left")
                        .or_else(|| child.child(0).filter(|c| c.kind() == "type_identifier"))
                        .map(|n| n.text(source).to_string())
                        .unwrap_or_default();
                    if !name.is_empty() {
                        let gen_id = emitter.alloc();
                        let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                        let name_val = emitter.string(&name);
                        emitter.emit("rs_generics", vec![
                            Value::Entity(gen_id),
                            name_val,
                            Value::Int(index),
                            Value::Entity(parent_id),
                        ]);
                        emitter.emit("hasLocation", vec![
                            Value::Entity(gen_id),
                            Value::Entity(loc_id),
                        ]);

                        extract_trait_bounds_for_param(emitter, file_id, gen_id, &child, source);
                        index += 1;
                    }
                }
                "lifetime" | "const_parameter" => {
                    // Lifetime parameters like 'a and const generics — skip for now
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

fn extract_trait_bounds_for_param(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    gen_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    // Try field access first (tree-sitter-rust uses "bounds" field)
    if let Some(bounds) = node.child_by_field("bounds") {
        extract_trait_bounds(emitter, file_id, gen_id, &bounds, source);
        return;
    }
    // Fallback: walk children looking for trait_bounds
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "trait_bounds" {
                extract_trait_bounds(emitter, file_id, gen_id, &child, source);
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

fn extract_trait_bounds(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    gen_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "type_identifier" | "scoped_type_identifier" | "generic_type"
                | "scoped_identifier" => {
                    let bound_name = child.text(source).to_string();
                    emit_trait_bound(emitter, file_id, gen_id, &child, &bound_name);
                }
                // tree-sitter-rust wraps bounds in `type` or other wrapper nodes
                _ if child.is_named() && child.kind() != "lifetime" => {
                    // Check if this is a wrapper containing a type identifier
                    let text = child.text(source);
                    if !text.is_empty() && text != "+" {
                        // Try to find a type identifier inside
                        let mut found = false;
                        let mut inner = child.walk();
                        if inner.goto_first_child() {
                            loop {
                                let ic = inner.node();
                                match ic.kind() {
                                    "type_identifier" | "scoped_type_identifier"
                                    | "scoped_identifier" | "generic_type" => {
                                        let bound_name = ic.text(source).to_string();
                                        emit_trait_bound(emitter, file_id, gen_id, &ic, &bound_name);
                                        found = true;
                                    }
                                    _ => {}
                                }
                                if !inner.goto_next_sibling() { break; }
                            }
                        }
                        if !found && child.kind() != "+" {
                            // Use the text as-is for opaque bound types
                            let bound_name = text.to_string();
                            if !bound_name.is_empty() {
                                emit_trait_bound(emitter, file_id, gen_id, &child, &bound_name);
                            }
                        }
                    }
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

fn emit_trait_bound(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    gen_id: EntityId,
    node: &Node<'_>,
    bound_name: &str,
) {
    let bound_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    let name_val = emitter.string(bound_name);
    emitter.emit("rs_trait_bounds", vec![
        Value::Entity(bound_id),
        name_val,
        Value::Entity(gen_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(bound_id),
        Value::Entity(loc_id),
    ]);
}

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

fn extract_parameters(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    fn_id: EntityId,
    params: &Node<'_>,
    source: &[u8],
) {
    let mut index = 0i64;
    let mut cursor = params.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "parameter" => {
                    let name = child.child_by_field("pattern")
                        .map(|n| n.text(source).to_string())
                        .unwrap_or_default();
                    let type_text = child.child_by_field("type")
                        .map(|t| t.text(source).to_string())
                        .unwrap_or_else(|| "unknown".to_string());

                    if !name.is_empty() {
                        let param_id = emitter.alloc();
                        let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                        let name_val = emitter.string(&name);
                        let type_val = emitter.string(&type_text);
                        emitter.emit("rs_params", vec![
                            Value::Entity(param_id),
                            name_val,
                            type_val,
                            Value::Int(index),
                            Value::Entity(fn_id),
                        ]);
                        emitter.emit("hasLocation", vec![
                            Value::Entity(param_id),
                            Value::Entity(loc_id),
                        ]);
                        index += 1;
                    }
                }
                "self_parameter" => {
                    let self_text = child.text(source).to_string();
                    let param_id = emitter.alloc();
                    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                    let name_val = emitter.string("self");
                    let type_val = emitter.string(&self_text);
                    emitter.emit("rs_params", vec![
                        Value::Entity(param_id),
                        name_val,
                        type_val,
                        Value::Int(index),
                        Value::Entity(fn_id),
                    ]);
                    emitter.emit("hasLocation", vec![
                        Value::Entity(param_id),
                        Value::Entity(loc_id),
                    ]);
                    index += 1;
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

// ---------------------------------------------------------------------------
// Function body
// ---------------------------------------------------------------------------

fn extract_function_body(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    fn_id: EntityId,
    body: &Node<'_>,
    source: &[u8],
) {
    if body.kind() == "block" {
        extract_block_stmts(emitter, file_id, fn_id, body, source);
    }
}

fn extract_block_stmts(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    parent_id: EntityId,
    block: &Node<'_>,
    source: &[u8],
) {
    let mut child_index = 0i64;
    let mut cursor = block.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.is_named() {
                if extract_stmt(emitter, file_id, &child, source, parent_id, child_index).is_some() {
                    child_index += 1;
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract items from a block (used for module bodies and other item-containing blocks).
fn extract_block_items(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    parent_id: EntityId,
    body: &Node<'_>,
    source: &[u8],
) {
    let mut cursor = body.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.is_named() {
                extract_item(emitter, file_id, parent_id, &child, source);
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

// ---------------------------------------------------------------------------
// Statements
// ---------------------------------------------------------------------------

fn extract_stmt(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
    index: i64,
) -> Option<EntityId> {
    let kind = match node.kind() {
        "let_declaration" => Some(STMT_LET),
        "expression_statement" => Some(STMT_EXPR),
        "return_expression" => Some(STMT_RETURN),
        "if_expression" => Some(STMT_IF),
        "match_expression" => Some(STMT_MATCH),
        "loop_expression" => Some(STMT_LOOP),
        "while_expression" => Some(STMT_WHILE),
        "for_expression" => Some(STMT_FOR),
        "block" => Some(STMT_BLOCK),
        _ => None,
    };

    // Also try extracting as an item (nested items in function bodies)
    if kind.is_none() {
        match node.kind() {
            "function_item" | "struct_item" | "enum_item" | "trait_item" | "impl_item"
            | "mod_item" | "use_declaration" | "const_item" | "static_item"
            | "type_item" | "macro_definition" | "attribute_item" | "inner_attribute_item"
            | "line_comment" | "block_comment" => {
                extract_item(emitter, file_id, parent_id, node, source);
                return None;
            }
            // Bare expressions (last expression in block, no semicolon)
            _ => {
                // Try to extract as an expression
                return extract_expr(emitter, file_id, node, source, parent_id, index);
            }
        }
    }

    let stmt_kind = kind.unwrap();
    let stmt_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    emitter.emit("rs_stmts", vec![
        Value::Entity(stmt_id),
        Value::Int(stmt_kind),
        Value::Entity(parent_id),
        Value::Int(index),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(stmt_id),
        Value::Entity(loc_id),
    ]);

    // Process children based on statement kind
    match node.kind() {
        "let_declaration" => {
            extract_let_declaration(emitter, file_id, stmt_id, node, source, index);
        }
        "expression_statement" => {
            extract_expression_statement(emitter, file_id, stmt_id, node, source);
        }
        "return_expression" => {
            // Extract the return value expression if present
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() && child.kind() != "return" {
                        extract_expr(emitter, file_id, &child, source, stmt_id, 0);
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "if_expression" => {
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, stmt_id, 0);
            }
            if let Some(consequence) = node.child_by_field("consequence") {
                extract_block_stmts(emitter, file_id, stmt_id, &consequence, source);
            }
            if let Some(alternative) = node.child_by_field("alternative") {
                // else branch — could be block or another if_expression
                if alternative.kind() == "block" {
                    extract_block_stmts(emitter, file_id, stmt_id, &alternative, source);
                } else if alternative.kind() == "else_clause" {
                    let mut cursor = alternative.walk();
                    if cursor.goto_first_child() {
                        loop {
                            let child = cursor.node();
                            if child.is_named() {
                                extract_stmt(emitter, file_id, &child, source, stmt_id, 1);
                            }
                            if !cursor.goto_next_sibling() { break; }
                        }
                    }
                } else {
                    extract_stmt(emitter, file_id, &alternative, source, stmt_id, 1);
                }
            }
        }
        "match_expression" => {
            if let Some(value) = node.child_by_field("value") {
                extract_expr(emitter, file_id, &value, source, stmt_id, 0);
            }
            if let Some(body) = node.child_by_field("body") {
                let mut arm_idx = 0i64;
                let mut cursor = body.walk();
                if cursor.goto_first_child() {
                    loop {
                        let child = cursor.node();
                        if child.kind() == "match_arm" {
                            if let Some(value) = child.child_by_field("value") {
                                extract_expr(emitter, file_id, &value, source, stmt_id, arm_idx + 1);
                            }
                            arm_idx += 1;
                        }
                        if !cursor.goto_next_sibling() { break; }
                    }
                }
            }
        }
        "loop_expression" => {
            if let Some(body) = node.child_by_field("body") {
                extract_block_stmts(emitter, file_id, stmt_id, &body, source);
            }
        }
        "while_expression" => {
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, stmt_id, 0);
            }
            if let Some(body) = node.child_by_field("body") {
                extract_block_stmts(emitter, file_id, stmt_id, &body, source);
            }
        }
        "for_expression" => {
            if let Some(pattern) = node.child_by_field("pattern") {
                let name = pattern.text(source).to_string();
                if !name.is_empty() {
                    let var_id = emitter.alloc();
                    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &pattern);
                    let name_val = emitter.string(&name);
                    let type_val = emitter.string("unknown");
                    emitter.emit("rs_local_vars", vec![
                        Value::Entity(var_id),
                        name_val,
                        type_val,
                        Value::Entity(stmt_id),
                    ]);
                    emitter.emit("hasLocation", vec![
                        Value::Entity(var_id),
                        Value::Entity(loc_id),
                    ]);
                }
            }
            if let Some(value) = node.child_by_field("value") {
                extract_expr(emitter, file_id, &value, source, stmt_id, 0);
            }
            if let Some(body) = node.child_by_field("body") {
                extract_block_stmts(emitter, file_id, stmt_id, &body, source);
            }
        }
        "block" => {
            extract_block_stmts(emitter, file_id, stmt_id, node, source);
        }
        _ => {}
    }

    Some(stmt_id)
}

fn extract_let_declaration(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    parent_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    _index: i64,
) {
    let name = node.child_by_field("pattern")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    let type_text = node.child_by_field("type")
        .map(|t| t.text(source).to_string())
        .unwrap_or_else(|| "unknown".to_string());

    if !name.is_empty() {
        let var_id = emitter.alloc();
        let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
        let name_val = emitter.string(&name);
        let type_val = emitter.string(&type_text);
        emitter.emit("rs_local_vars", vec![
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

    // Extract initializer
    if let Some(value) = node.child_by_field("value") {
        extract_expr(emitter, file_id, &value, source, parent_id, 0);
    }
}

fn extract_expression_statement(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    parent_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.is_named() {
                // Some expression forms are better modeled as statements
                match child.kind() {
                    "if_expression" | "match_expression" | "loop_expression"
                    | "while_expression" | "for_expression" => {
                        extract_stmt(emitter, file_id, &child, source, parent_id, 0);
                    }
                    _ => {
                        extract_expr(emitter, file_id, &child, source, parent_id, 0);
                    }
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

// ---------------------------------------------------------------------------
// Expressions
// ---------------------------------------------------------------------------

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
        "field_expression" => Some(EXPR_FIELD),
        "index_expression" => Some(EXPR_INDEX),
        "identifier" | "scoped_identifier" => Some(EXPR_PATH),
        "integer_literal" => Some(EXPR_LITERAL_INT),
        "float_literal" => Some(EXPR_LITERAL_FLOAT),
        "string_literal" | "raw_string_literal" => Some(EXPR_LITERAL_STRING),
        "char_literal" => Some(EXPR_LITERAL_CHAR),
        "boolean_literal" => Some(EXPR_LITERAL_BOOL),
        "binary_expression" => Some(EXPR_BINARY),
        "unary_expression" => Some(EXPR_UNARY),
        "reference_expression" => Some(EXPR_REFERENCE),
        "try_expression" => Some(EXPR_TRY),
        "await_expression" => Some(EXPR_AWAIT),
        "return_expression" => Some(EXPR_RETURN),
        "break_expression" => Some(EXPR_BREAK),
        "continue_expression" => Some(EXPR_CONTINUE),
        "closure_expression" => Some(EXPR_CLOSURE),
        "if_expression" => Some(EXPR_IF),
        "match_expression" => Some(EXPR_MATCH),
        "block" => Some(EXPR_BLOCK),
        "tuple_expression" => Some(EXPR_TUPLE),
        "array_expression" => Some(EXPR_ARRAY),
        "struct_expression" => Some(EXPR_STRUCT),
        "range_expression" => Some(EXPR_RANGE),
        "assignment_expression" => Some(EXPR_ASSIGNMENT),
        "compound_assignment_expr" => Some(EXPR_COMPOUND_ASSIGNMENT),
        "macro_invocation" => Some(EXPR_MACRO_INVOCATION),
        "parenthesized_expression" => {
            // Unwrap parentheses, extract inner expression
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
        "type_cast_expression" => {
            // Extract the value being cast
            if let Some(value) = node.child_by_field("value") {
                return extract_expr(emitter, file_id, &value, source, parent_id, index);
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

    emitter.emit("rs_exprs", vec![
        Value::Entity(expr_id),
        Value::Int(expr_kind),
        Value::Entity(parent_id),
        Value::Int(index),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(expr_id),
        Value::Entity(loc_id),
    ]);

    // Recurse into children based on expression kind
    match node.kind() {
        "call_expression" => {
            if let Some(func) = node.child_by_field("function") {
                // Check if this is a method call (function is a field_expression)
                extract_expr(emitter, file_id, &func, source, expr_id, 0);
            }
            if let Some(args) = node.child_by_field("arguments") {
                let mut idx = 1i64;
                let mut cursor = args.walk();
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
        }
        "field_expression" => {
            if let Some(value) = node.child_by_field("value") {
                extract_expr(emitter, file_id, &value, source, expr_id, 0);
            }
        }
        "index_expression" => {
            // child(0) is the value, child(1) is the index
            let mut idx = 0i64;
            let mut cursor = node.walk();
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
        "binary_expression" => {
            if let Some(left) = node.child_by_field("left") {
                extract_expr(emitter, file_id, &left, source, expr_id, 0);
            }
            if let Some(right) = node.child_by_field("right") {
                extract_expr(emitter, file_id, &right, source, expr_id, 1);
            }
        }
        "unary_expression" => {
            // The operand
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
        "reference_expression" => {
            if let Some(value) = node.child_by_field("value") {
                extract_expr(emitter, file_id, &value, source, expr_id, 0);
            }
        }
        "assignment_expression" | "compound_assignment_expr" => {
            if let Some(left) = node.child_by_field("left") {
                extract_expr(emitter, file_id, &left, source, expr_id, 0);
            }
            if let Some(right) = node.child_by_field("right") {
                extract_expr(emitter, file_id, &right, source, expr_id, 1);
            }
        }
        "try_expression" => {
            // The inner expression is the first named child
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
        "await_expression" => {
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
        "return_expression" => {
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
        "closure_expression" => {
            if let Some(body) = node.child_by_field("body") {
                extract_expr(emitter, file_id, &body, source, expr_id, 0);
            }
        }
        "if_expression" => {
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, expr_id, 0);
            }
            if let Some(consequence) = node.child_by_field("consequence") {
                extract_block_stmts(emitter, file_id, expr_id, &consequence, source);
            }
        }
        "match_expression" => {
            if let Some(value) = node.child_by_field("value") {
                extract_expr(emitter, file_id, &value, source, expr_id, 0);
            }
        }
        "block" => {
            extract_block_stmts(emitter, file_id, expr_id, node, source);
        }
        "tuple_expression" | "array_expression" => {
            let mut idx = 0i64;
            let mut cursor = node.walk();
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
        "struct_expression" => {
            // Extract field initializer expressions
            if let Some(body) = node.child_by_field("body") {
                let mut idx = 0i64;
                let mut cursor = body.walk();
                if cursor.goto_first_child() {
                    loop {
                        let child = cursor.node();
                        if child.kind() == "field_initializer" {
                            if let Some(value) = child.child_by_field("value") {
                                extract_expr(emitter, file_id, &value, source, expr_id, idx);
                                idx += 1;
                            }
                        }
                        if !cursor.goto_next_sibling() { break; }
                    }
                }
            }
        }
        "range_expression" => {
            let mut idx = 0i64;
            let mut cursor = node.walk();
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
        "macro_invocation" => {
            // Arguments to macro are opaque, skip recursion
        }
        _ => {}
    }

    Some(expr_id)
}

// ---------------------------------------------------------------------------
// Comments
// ---------------------------------------------------------------------------

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
    emitter.emit("rs_comments", vec![
        Value::Entity(comment_id),
        text_val,
        Value::Entity(loc_id),
    ]);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ocql_database::Database;
    use crate::schema::rust_schema;

    fn extract_test_file(filename: &str) -> Database {
        let source = std::fs::read(
            format!("tests/fixtures/{}", filename)
        ).unwrap();
        let schema = rust_schema();
        let mut db = Database::from_schema(schema);
        let extractor = RustExtractor::new();
        let result = extractor.extract_source(&mut db, filename, &source);
        assert!(result.success, "Extraction failed for {}: {:?}", filename, result.error);
        db
    }

    #[test]
    fn test_modules() {
        let db = extract_test_file("simple.rs");
        let modules: Vec<_> = db.scan("rs_modules").unwrap().collect();
        let names: Vec<_> = modules.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Modules: {:?}", names);
        assert!(names.contains(&"inner"), "Should find module 'inner'");
    }

    #[test]
    fn test_functions() {
        let db = extract_test_file("simple.rs");
        let functions: Vec<_> = db.scan("rs_functions").unwrap().collect();
        let names: Vec<_> = functions.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Functions: {:?}", names);
        assert!(names.contains(&"main"), "Should find function 'main'");
        assert!(names.contains(&"add"), "Should find function 'add'");
        assert!(names.contains(&"process"), "Should find function 'process'");
        assert!(names.contains(&"greet"), "Should find function 'greet'");
    }

    #[test]
    fn test_structs() {
        let db = extract_test_file("simple.rs");
        let structs: Vec<_> = db.scan("rs_structs").unwrap().collect();
        let names: Vec<_> = structs.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Structs: {:?}", names);
        assert!(names.contains(&"Point"), "Should find struct 'Point'");
        assert!(names.contains(&"Config"), "Should find struct 'Config'");
    }

    #[test]
    fn test_enums() {
        let db = extract_test_file("simple.rs");
        let enums: Vec<_> = db.scan("rs_enums").unwrap().collect();
        let names: Vec<_> = enums.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Enums: {:?}", names);
        assert!(names.contains(&"Color"), "Should find enum 'Color'");
        assert!(names.contains(&"Shape"), "Should find enum 'Shape'");
    }

    #[test]
    fn test_enum_variants() {
        let db = extract_test_file("simple.rs");
        let variants: Vec<_> = db.scan("rs_enum_variants").unwrap().collect();
        let names: Vec<_> = variants.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Enum variants: {:?}", names);
        assert!(names.contains(&"Red"), "Should find variant 'Red'");
        assert!(names.contains(&"Green"), "Should find variant 'Green'");
        assert!(names.contains(&"Blue"), "Should find variant 'Blue'");
        assert!(names.contains(&"Circle"), "Should find variant 'Circle'");
        assert!(names.contains(&"Rectangle"), "Should find variant 'Rectangle'");
    }

    #[test]
    fn test_traits() {
        let db = extract_test_file("simple.rs");
        let traits: Vec<_> = db.scan("rs_traits").unwrap().collect();
        let names: Vec<_> = traits.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Traits: {:?}", names);
        assert!(names.contains(&"Describable"), "Should find trait 'Describable'");
    }

    #[test]
    fn test_impls() {
        let db = extract_test_file("simple.rs");
        let impls: Vec<_> = db.scan("rs_impls").unwrap().collect();
        let type_names: Vec<_> = impls.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Impl types: {:?}", type_names);
        assert!(type_names.contains(&"Point"), "Should find impl for 'Point'");
    }

    #[test]
    fn test_use_decls() {
        let db = extract_test_file("simple.rs");
        let uses: Vec<_> = db.scan("rs_use_decls").unwrap().collect();
        let paths: Vec<_> = uses.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Use paths: {:?}", paths);
        assert!(paths.iter().any(|p| p.contains("HashMap")), "Should find HashMap use");
        assert!(paths.iter().any(|p| p.contains("fmt")), "Should find fmt use");
    }

    #[test]
    fn test_fields() {
        let db = extract_test_file("simple.rs");
        let fields: Vec<_> = db.scan("rs_fields").unwrap().collect();
        let names: Vec<_> = fields.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Fields: {:?}", names);
        assert!(names.contains(&"x"), "Should find field 'x'");
        assert!(names.contains(&"y"), "Should find field 'y'");
    }

    #[test]
    fn test_params() {
        let db = extract_test_file("simple.rs");
        let params: Vec<_> = db.scan("rs_params").unwrap().collect();
        let names: Vec<_> = params.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Params: {:?}", names);
        assert!(names.contains(&"a"), "Should find param 'a'");
        assert!(names.contains(&"b"), "Should find param 'b'");
    }

    #[test]
    fn test_local_vars() {
        let db = extract_test_file("simple.rs");
        let vars: Vec<_> = db.scan("rs_local_vars").unwrap().collect();
        let names: Vec<_> = vars.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Local vars: {:?}", names);
        assert!(names.contains(&"result"), "Should find local 'result'");
        assert!(names.contains(&"p"), "Should find local 'p'");
    }

    #[test]
    fn test_stmts() {
        let db = extract_test_file("simple.rs");
        let stmts: Vec<_> = db.scan("rs_stmts").unwrap().collect();
        let kinds: Vec<i64> = stmts.iter().map(|t| t[1].as_int().unwrap()).collect();
        eprintln!("Statement kinds: {:?}", kinds);
        assert!(kinds.contains(&STMT_LET), "Should have let statements");
        assert!(kinds.contains(&STMT_IF), "Should have if statements");
        assert!(kinds.contains(&STMT_MATCH), "Should have match statements");
        assert!(kinds.contains(&STMT_FOR), "Should have for statements");
    }

    #[test]
    fn test_exprs() {
        let db = extract_test_file("simple.rs");
        let exprs: Vec<_> = db.scan("rs_exprs").unwrap().collect();
        let kinds: Vec<i64> = exprs.iter().map(|t| t[1].as_int().unwrap()).collect();
        eprintln!("Expression kinds: {:?}", kinds);
        assert!(kinds.contains(&EXPR_CALL), "Should have call expressions");
        assert!(kinds.contains(&EXPR_BINARY), "Should have binary expressions");
        assert!(kinds.contains(&EXPR_LITERAL_INT), "Should have integer literals");
        assert!(kinds.contains(&EXPR_LITERAL_STRING), "Should have string literals");
    }

    #[test]
    fn test_attributes() {
        let db = extract_test_file("simple.rs");
        let attrs: Vec<_> = db.scan("rs_attributes").unwrap().collect();
        let names: Vec<_> = attrs.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Attributes: {:?}", names);
        assert!(names.contains(&"derive"), "Should find #[derive(...)]");
    }

    #[test]
    fn test_comments() {
        let db = extract_test_file("simple.rs");
        let comments: Vec<_> = db.scan("rs_comments").unwrap().collect();
        eprintln!("Comments: {}", comments.len());
        assert!(comments.len() >= 2, "Should find at least 2 comments");
    }

    #[test]
    fn test_generics() {
        let db = extract_test_file("simple.rs");
        let generics: Vec<_> = db.scan("rs_generics").unwrap().collect();
        let names: Vec<_> = generics.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Generics: {:?}", names);
        assert!(names.contains(&"T"), "Should find generic 'T'");
    }

    #[test]
    fn test_trait_bounds() {
        let db = extract_test_file("simple.rs");
        let bounds: Vec<_> = db.scan("rs_trait_bounds").unwrap().collect();
        let names: Vec<_> = bounds.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Trait bounds: {:?}", names);
        assert!(names.iter().any(|n| n.contains("Display")), "Should find Display bound");
    }
}
