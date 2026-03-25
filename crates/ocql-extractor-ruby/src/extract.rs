use ocql_database::{EntityId, Value};
use ocql_extractor_common::{Extractor, FactEmitter, LocationEmitter, NodeExt};
use ocql_extractor_common::tree_sitter::{Language, Node, Tree};

/// Ruby extractor using tree-sitter.
///
/// Extracts:
/// - Modules, classes (with superclass)
/// - Methods (instance and singleton)
/// - Blocks (do..end, {})
/// - Parameters (required, optional, rest, keyword, block)
/// - Statements (if, unless, while, until, for, case, begin/rescue/ensure, return, break, next, redo, retry, yield)
/// - Expressions (calls, identifiers, literals, binary/unary ops, assignments, etc.)
/// - Constants, require/require_relative
/// - Comments, local variables
pub struct RubyExtractor;

impl RubyExtractor {
    pub fn new() -> Self {
        Self
    }
}

// Statement kind constants
const STMT_IF: i64 = 0;
const STMT_UNLESS: i64 = 1;
const STMT_WHILE: i64 = 2;
const STMT_UNTIL: i64 = 3;
const STMT_FOR: i64 = 4;
const STMT_CASE: i64 = 5;
const STMT_BEGIN: i64 = 6;
const STMT_RESCUE: i64 = 7;
const STMT_ENSURE: i64 = 8;
const STMT_RETURN: i64 = 9;
const STMT_BREAK: i64 = 10;
const STMT_NEXT: i64 = 11;
const STMT_REDO: i64 = 12;
const STMT_RETRY: i64 = 13;
const STMT_YIELD: i64 = 14;
const STMT_RAISE: i64 = 15;
const STMT_IF_MOD: i64 = 16;
const STMT_UNLESS_MOD: i64 = 17;
const STMT_WHILE_MOD: i64 = 18;
const STMT_UNTIL_MOD: i64 = 19;
const STMT_WHEN: i64 = 20;
const STMT_ELSE: i64 = 21;

// Expression kind constants
const EXPR_CALL: i64 = 0;
const EXPR_IDENTIFIER: i64 = 1;
const EXPR_INTEGER: i64 = 2;
const EXPR_FLOAT: i64 = 3;
const EXPR_STRING: i64 = 4;
const EXPR_SYMBOL: i64 = 5;
const EXPR_ARRAY: i64 = 6;
const EXPR_HASH: i64 = 7;
const EXPR_REGEX: i64 = 8;
const EXPR_RANGE: i64 = 9;
const EXPR_SELF: i64 = 10;
const EXPR_NIL: i64 = 11;
const EXPR_TRUE: i64 = 12;
const EXPR_FALSE: i64 = 13;
const EXPR_BINARY: i64 = 14;
const EXPR_UNARY: i64 = 15;
const EXPR_ASSIGNMENT: i64 = 16;
const EXPR_ELEMENT_REF: i64 = 17;
const EXPR_CONDITIONAL: i64 = 18;
const EXPR_SPLAT: i64 = 19;
const EXPR_BLOCK_ARG: i64 = 20;
const EXPR_LAMBDA: i64 = 21;
const EXPR_HEREDOC: i64 = 22;
const EXPR_INTERPOLATION: i64 = 23;
const EXPR_CONSTANT: i64 = 24;
const EXPR_OP_ASSIGN: i64 = 25;
const EXPR_PAIR: i64 = 26;

// Parameter kind constants
const PARAM_REQUIRED: i64 = 0;
const PARAM_OPTIONAL: i64 = 1;
const PARAM_REST: i64 = 2;
const PARAM_KEYWORD: i64 = 3;
const PARAM_BLOCK: i64 = 4;

impl Extractor for RubyExtractor {
    fn language(&self) -> Language {
        tree_sitter_ruby::LANGUAGE.into()
    }

    fn extensions(&self) -> &[&str] {
        &["rb", "rake", "gemspec"]
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
    let mut idx = 0i64;
    if cursor.goto_first_child() {
        loop {
            let node = cursor.node();
            if node.is_named() {
                extract_node(emitter, file_id, &node, source, file_id, idx);
                idx += 1;
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// Maximum recursion depth to prevent stack overflow.
const MAX_DEPTH: usize = 100;

/// Extract a node in any scope context.
fn extract_node(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
    index: i64,
) {
    extract_node_depth(emitter, file_id, node, source, parent_id, index, 0);
}

fn extract_node_depth(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
    index: i64,
    depth: usize,
) {
    if depth > MAX_DEPTH {
        return;
    }
    match node.kind() {
        "module" => {
            extract_module(emitter, file_id, node, source, parent_id);
        }
        "class" => {
            extract_class(emitter, file_id, node, source, parent_id);
        }
        "singleton_class" => {
            extract_singleton_class(emitter, file_id, node, source, parent_id);
        }
        "method" => {
            extract_method(emitter, file_id, node, source, parent_id);
        }
        "singleton_method" => {
            extract_singleton_method(emitter, file_id, node, source, parent_id);
        }
        "comment" => {
            extract_comment(emitter, file_id, node, source);
        }
        "call" => {
            // Check for require/require_relative calls
            if is_require_call(node, source) {
                extract_require(emitter, file_id, node, source);
            }
            extract_expr(emitter, file_id, node, source, parent_id, index);
        }
        _ => {
            // Try as statement, then as expression
            if extract_stmt(emitter, file_id, node, source, parent_id, index).is_none() {
                extract_expr(emitter, file_id, node, source, parent_id, index);
            }
        }
    }
}

/// Extract a module definition.
fn extract_module(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    if name.is_empty() { return; }

    let mod_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let name_val = emitter.string(&name);
    emitter.emit("rb_modules", vec![
        Value::Entity(mod_id),
        name_val,
        Value::Entity(parent_id),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(mod_id),
        Value::Entity(loc_id),
    ]);

    // Extract body
    if let Some(body) = node.child_by_field("body") {
        extract_body(emitter, file_id, &body, source, mod_id);
    }
}

/// Extract a class definition.
fn extract_class(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    if name.is_empty() { return; }

    let superclass_name = node.child_by_field("superclass")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();

    let class_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let name_val = emitter.string(&name);
    let super_val = emitter.string(&superclass_name);
    emitter.emit("rb_classes", vec![
        Value::Entity(class_id),
        name_val,
        Value::Entity(parent_id),
        super_val,
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(class_id),
        Value::Entity(loc_id),
    ]);

    // Extract body
    if let Some(body) = node.child_by_field("body") {
        extract_body(emitter, file_id, &body, source, class_id);
    }
}

/// Extract a singleton class (class << self).
fn extract_singleton_class(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let class_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let name_val = emitter.string("<<self>>");
    let super_val = emitter.string("");
    emitter.emit("rb_classes", vec![
        Value::Entity(class_id),
        name_val,
        Value::Entity(parent_id),
        super_val,
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(class_id),
        Value::Entity(loc_id),
    ]);

    // Extract body
    if let Some(body) = node.child_by_field("body") {
        extract_body(emitter, file_id, &body, source, class_id);
    }
}

/// Extract a method definition.
fn extract_method(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    if name.is_empty() { return; }

    let method_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let name_val = emitter.string(&name);
    emitter.emit("rb_methods", vec![
        Value::Entity(method_id),
        name_val,
        Value::Entity(parent_id),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(method_id),
        Value::Entity(loc_id),
    ]);

    // Extract parameters
    if let Some(params) = node.child_by_field("parameters") {
        extract_parameters(emitter, file_id, method_id, &params, source);
    }

    // Extract body
    if let Some(body) = node.child_by_field("body") {
        extract_body(emitter, file_id, &body, source, method_id);
    }
}

/// Extract a singleton method definition (def self.method_name).
fn extract_singleton_method(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    if name.is_empty() { return; }

    let method_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let name_val = emitter.string(&name);
    emitter.emit("rb_singleton_methods", vec![
        Value::Entity(method_id),
        name_val,
        Value::Entity(parent_id),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(method_id),
        Value::Entity(loc_id),
    ]);

    // Extract parameters
    if let Some(params) = node.child_by_field("parameters") {
        extract_parameters(emitter, file_id, method_id, &params, source);
    }

    // Extract body
    if let Some(body) = node.child_by_field("body") {
        extract_body(emitter, file_id, &body, source, method_id);
    }
}

/// Extract a block (do..end or {}).
fn extract_block(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) -> EntityId {
    let block_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    emitter.emit("rb_blocks", vec![
        Value::Entity(block_id),
        Value::Entity(parent_id),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(block_id),
        Value::Entity(loc_id),
    ]);

    // Extract block parameters
    if let Some(params) = node.child_by_field("parameters") {
        extract_parameters(emitter, file_id, block_id, &params, source);
    }

    // Extract body
    if let Some(body) = node.child_by_field("body") {
        extract_body(emitter, file_id, &body, source, block_id);
    }

    block_id
}

/// Extract parameters from a method_parameters, block_parameters, or lambda_parameters node.
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
            let (name, kind) = match child.kind() {
                "identifier" => {
                    (child.text(source).to_string(), PARAM_REQUIRED)
                }
                "optional_parameter" => {
                    let n = child.child_by_field("name")
                        .map(|n| n.text(source).to_string())
                        .unwrap_or_default();
                    (n, PARAM_OPTIONAL)
                }
                "splat_parameter" => {
                    let n = child.child_by_field("name")
                        .map(|n| n.text(source).to_string())
                        .unwrap_or_else(|| "*".to_string());
                    (n, PARAM_REST)
                }
                "hash_splat_parameter" => {
                    let n = child.child_by_field("name")
                        .map(|n| n.text(source).to_string())
                        .unwrap_or_else(|| "**".to_string());
                    (n, PARAM_KEYWORD)
                }
                "keyword_parameter" => {
                    let n = child.child_by_field("name")
                        .map(|n| n.text(source).to_string())
                        .unwrap_or_default();
                    (n, PARAM_KEYWORD)
                }
                "block_parameter" => {
                    let n = child.child_by_field("name")
                        .map(|n| n.text(source).to_string())
                        .unwrap_or_default();
                    (n, PARAM_BLOCK)
                }
                _ => continue,
            };

            if !name.is_empty() {
                let param_id = emitter.alloc();
                let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                let name_val = emitter.string(&name);
                emitter.emit("rb_params", vec![
                    Value::Entity(param_id),
                    name_val,
                    Value::Int(kind),
                    Value::Int(index),
                    Value::Entity(callable_id),
                ]);
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

/// Extract a body node (children of a body_statement or similar).
fn extract_body(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    body: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let mut cursor = body.walk();
    let mut idx = 0i64;
    if cursor.goto_first_child() {
        loop {
            let node = cursor.node();
            if node.is_named() {
                extract_node(emitter, file_id, &node, source, parent_id, idx);
                idx += 1;
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
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
        "if" => Some(STMT_IF),
        "unless" => Some(STMT_UNLESS),
        "while" => Some(STMT_WHILE),
        "until" => Some(STMT_UNTIL),
        "for" => Some(STMT_FOR),
        "case" => Some(STMT_CASE),
        "begin" => Some(STMT_BEGIN),
        "rescue" => Some(STMT_RESCUE),
        "ensure" => Some(STMT_ENSURE),
        "return" => Some(STMT_RETURN),
        "break" => Some(STMT_BREAK),
        "next" => Some(STMT_NEXT),
        "redo" => Some(STMT_REDO),
        "retry" => Some(STMT_RETRY),
        "yield" => Some(STMT_YIELD),
        "if_modifier" => Some(STMT_IF_MOD),
        "unless_modifier" => Some(STMT_UNLESS_MOD),
        "while_modifier" => Some(STMT_WHILE_MOD),
        "until_modifier" => Some(STMT_UNTIL_MOD),
        "when" => Some(STMT_WHEN),
        "else" => Some(STMT_ELSE),
        _ => None,
    };

    let stmt_kind = kind?;

    let stmt_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    emitter.emit("rb_stmts", vec![
        Value::Entity(stmt_id),
        Value::Int(stmt_kind),
        Value::Entity(parent_id),
        Value::Int(index),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(stmt_id),
        Value::Entity(loc_id),
    ]);

    // Recurse into children for compound statements
    match node.kind() {
        "if" | "unless" => {
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, stmt_id, 0);
            }
            if let Some(consequence) = node.child_by_field("consequence") {
                extract_body(emitter, file_id, &consequence, source, stmt_id);
            }
            if let Some(alternative) = node.child_by_field("alternative") {
                extract_node(emitter, file_id, &alternative, source, stmt_id, 1);
            }
        }
        "while" | "until" => {
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, stmt_id, 0);
            }
            if let Some(body) = node.child_by_field("body") {
                extract_body(emitter, file_id, &body, source, stmt_id);
            }
        }
        "for" => {
            if let Some(body) = node.child_by_field("body") {
                extract_body(emitter, file_id, &body, source, stmt_id);
            }
        }
        "case" => {
            if let Some(value) = node.child_by_field("value") {
                extract_expr(emitter, file_id, &value, source, stmt_id, 0);
            }
            // Extract when/else children
            let mut cursor = node.walk();
            let mut child_idx = 1i64;
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "when" || child.kind() == "else" {
                        extract_stmt(emitter, file_id, &child, source, stmt_id, child_idx);
                        child_idx += 1;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "begin" => {
            // Extract body and rescue/ensure/else children
            let mut cursor = node.walk();
            let mut child_idx = 0i64;
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        match child.kind() {
                            "rescue" | "ensure" | "else" => {
                                extract_stmt(emitter, file_id, &child, source, stmt_id, child_idx);
                                child_idx += 1;
                            }
                            _ => {
                                extract_node(emitter, file_id, &child, source, stmt_id, child_idx);
                                child_idx += 1;
                            }
                        }
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "rescue" | "ensure" | "when" | "else" => {
            let mut cursor = node.walk();
            let mut child_idx = 0i64;
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_node(emitter, file_id, &child, source, stmt_id, child_idx);
                        child_idx += 1;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "return" | "break" | "next" | "yield" => {
            // Extract argument expressions
            let mut cursor = node.walk();
            let mut child_idx = 0i64;
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, &child, source, stmt_id, child_idx);
                        child_idx += 1;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "if_modifier" | "unless_modifier" | "while_modifier" | "until_modifier" => {
            if let Some(body) = node.child_by_field("body") {
                extract_expr(emitter, file_id, &body, source, stmt_id, 0);
            }
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, stmt_id, 1);
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
        "call" => Some(EXPR_CALL),
        "identifier" => Some(EXPR_IDENTIFIER),
        "integer" => Some(EXPR_INTEGER),
        "float" => Some(EXPR_FLOAT),
        "string" | "string_content" => Some(EXPR_STRING),
        "symbol" | "simple_symbol" | "hash_key_symbol" => Some(EXPR_SYMBOL),
        "array" => Some(EXPR_ARRAY),
        "hash" => Some(EXPR_HASH),
        "regex" => Some(EXPR_REGEX),
        "range" => Some(EXPR_RANGE),
        "self" => Some(EXPR_SELF),
        "nil" => Some(EXPR_NIL),
        "true" => Some(EXPR_TRUE),
        "false" => Some(EXPR_FALSE),
        "binary" => Some(EXPR_BINARY),
        "unary" => Some(EXPR_UNARY),
        "assignment" => Some(EXPR_ASSIGNMENT),
        "operator_assignment" => Some(EXPR_OP_ASSIGN),
        "element_reference" => Some(EXPR_ELEMENT_REF),
        "conditional" => Some(EXPR_CONDITIONAL),
        "splat_argument" => Some(EXPR_SPLAT),
        "block_argument" => Some(EXPR_BLOCK_ARG),
        "lambda" => Some(EXPR_LAMBDA),
        "heredoc_body" => Some(EXPR_HEREDOC),
        "interpolation" => Some(EXPR_INTERPOLATION),
        "constant" | "scope_resolution" => Some(EXPR_CONSTANT),
        "pair" => Some(EXPR_PAIR),
        "parenthesized_statements" => {
            // Extract inner expression(s) directly
            let mut cursor = node.walk();
            let mut child_idx = 0i64;
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, &child, source, parent_id, index + child_idx);
                        child_idx += 1;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
            return None;
        }
        _ => None,
    };

    let expr_kind = kind?;

    let expr_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    emitter.emit("rb_exprs", vec![
        Value::Entity(expr_id),
        Value::Int(expr_kind),
        Value::Entity(parent_id),
        Value::Int(index),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(expr_id),
        Value::Entity(loc_id),
    ]);

    // Handle identifier as local variable
    if node.kind() == "identifier" {
        let name = node.text(source);
        // Only register as local var if it looks like an assignment target or lhs
        // We'll be conservative and register all identifiers as potential local vars
        let var_id = emitter.alloc();
        let name_val = emitter.string(name);
        emitter.emit("rb_local_vars", vec![
            Value::Entity(var_id),
            name_val,
            Value::Entity(parent_id),
        ]);
    }

    // Handle constant assignments
    if node.kind() == "assignment" {
        if let Some(left) = node.child_by_field("left") {
            if left.kind() == "constant" {
                let name = left.text(source).to_string();
                let const_id = emitter.alloc();
                let const_loc = LocationEmitter::emit_for_node(emitter, file_id, node);
                let name_val = emitter.string(&name);
                emitter.emit("rb_constants", vec![
                    Value::Entity(const_id),
                    name_val,
                    Value::Entity(parent_id),
                ]);
                emitter.emit("hasLocation", vec![
                    Value::Entity(const_id),
                    Value::Entity(const_loc),
                ]);
            }
        }
    }

    // Recurse into children based on expression type
    match node.kind() {
        "call" => {
            extract_call_details(emitter, file_id, node, source, expr_id);
        }
        "binary" => {
            if let Some(left) = node.child_by_field("left") {
                extract_expr(emitter, file_id, &left, source, expr_id, 0);
            }
            if let Some(right) = node.child_by_field("right") {
                extract_expr(emitter, file_id, &right, source, expr_id, 1);
            }
        }
        "unary" => {
            if let Some(operand) = node.child_by_field("operand") {
                extract_expr(emitter, file_id, &operand, source, expr_id, 0);
            }
        }
        "assignment" | "operator_assignment" => {
            if let Some(left) = node.child_by_field("left") {
                extract_expr(emitter, file_id, &left, source, expr_id, 0);
            }
            if let Some(right) = node.child_by_field("right") {
                extract_expr(emitter, file_id, &right, source, expr_id, 1);
            }
        }
        "conditional" => {
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
        "element_reference" => {
            let mut cursor = node.walk();
            let mut child_idx = 0i64;
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, &child, source, expr_id, child_idx);
                        child_idx += 1;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "array" => {
            let mut cursor = node.walk();
            let mut child_idx = 0i64;
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, &child, source, expr_id, child_idx);
                        child_idx += 1;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "hash" => {
            let mut cursor = node.walk();
            let mut child_idx = 0i64;
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, &child, source, expr_id, child_idx);
                        child_idx += 1;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "pair" => {
            if let Some(key) = node.child_by_field("key") {
                extract_expr(emitter, file_id, &key, source, expr_id, 0);
            }
            if let Some(value) = node.child_by_field("value") {
                extract_expr(emitter, file_id, &value, source, expr_id, 1);
            }
        }
        "range" => {
            let mut cursor = node.walk();
            let mut child_idx = 0i64;
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, &child, source, expr_id, child_idx);
                        child_idx += 1;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "splat_argument" | "block_argument" => {
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
        "lambda" => {
            if let Some(params) = node.child_by_field("parameters") {
                extract_parameters(emitter, file_id, expr_id, &params, source);
            }
            if let Some(body) = node.child_by_field("body") {
                extract_body(emitter, file_id, &body, source, expr_id);
            }
        }
        "string" => {
            // Extract interpolations within strings
            let mut cursor = node.walk();
            let mut child_idx = 0i64;
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "interpolation" {
                        extract_expr(emitter, file_id, &child, source, expr_id, child_idx);
                        child_idx += 1;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        _ => {}
    }

    Some(expr_id)
}

/// Extract call details into rb_calls.
fn extract_call_details(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    call_expr_id: EntityId,
) {
    let method_name = node.child_by_field("method")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();

    let receiver_id = if let Some(receiver) = node.child_by_field("receiver") {
        extract_expr(emitter, file_id, &receiver, source, call_expr_id, 0)
            .unwrap_or(call_expr_id)
    } else {
        call_expr_id
    };

    let call_text = node.text(source);
    let call_name = emitter.string(call_text);
    let method_val = emitter.string(&method_name);
    emitter.emit("rb_calls", vec![
        Value::Entity(call_expr_id),
        call_name,
        Value::Entity(receiver_id),
        method_val,
    ]);

    // Extract arguments
    if let Some(args) = node.child_by_field("arguments") {
        let mut cursor = args.walk();
        let mut arg_idx = 1i64; // start at 1; 0 is receiver
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.is_named() {
                    extract_expr(emitter, file_id, &child, source, call_expr_id, arg_idx);
                    arg_idx += 1;
                }
                if !cursor.goto_next_sibling() { break; }
            }
        }
    }

    // Extract attached block (do..end or {})
    // tree-sitter-ruby stores blocks as the "block" field on call nodes
    if let Some(blk) = node.child_by_field("block") {
        extract_block(emitter, file_id, &blk, source, call_expr_id);
    } else {
        // Also check direct children for block/do_block if not a named field
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "block" || child.kind() == "do_block" {
                    extract_block(emitter, file_id, &child, source, call_expr_id);
                    break; // only one block per call
                }
                if !cursor.goto_next_sibling() { break; }
            }
        }
    }
}

/// Check if a call node is a require/require_relative.
fn is_require_call(node: &Node<'_>, source: &[u8]) -> bool {
    if let Some(method) = node.child_by_field("method") {
        let name = method.text(source);
        return name == "require" || name == "require_relative";
    }
    false
}

/// Extract a require/require_relative call.
fn extract_require(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    // Find the string argument
    if let Some(args) = node.child_by_field("arguments") {
        let mut cursor = args.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "argument_list" {
                    // Recurse into argument_list
                    let mut inner = child.walk();
                    if inner.goto_first_child() {
                        loop {
                            let arg = inner.node();
                            if arg.kind() == "string" {
                                let path = extract_string_content(&arg, source);
                                let req_id = emitter.alloc();
                                let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
                                let path_val = emitter.string(&path);
                                emitter.emit("rb_requires", vec![
                                    Value::Entity(req_id),
                                    path_val,
                                ]);
                                emitter.emit("hasLocation", vec![
                                    Value::Entity(req_id),
                                    Value::Entity(loc_id),
                                ]);
                                return;
                            }
                            if !inner.goto_next_sibling() { break; }
                        }
                    }
                }
                if child.kind() == "string" {
                    let path = extract_string_content(&child, source);
                    let req_id = emitter.alloc();
                    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
                    let path_val = emitter.string(&path);
                    emitter.emit("rb_requires", vec![
                        Value::Entity(req_id),
                        path_val,
                    ]);
                    emitter.emit("hasLocation", vec![
                        Value::Entity(req_id),
                        Value::Entity(loc_id),
                    ]);
                    return;
                }
                if !cursor.goto_next_sibling() { break; }
            }
        }
    }
}

/// Extract the text content of a string node (strip quotes).
fn extract_string_content(node: &Node<'_>, source: &[u8]) -> String {
    let text = node.text(source);
    // Strip surrounding quotes
    if (text.starts_with('"') && text.ends_with('"'))
        || (text.starts_with('\'') && text.ends_with('\''))
    {
        text[1..text.len()-1].to_string()
    } else {
        text.to_string()
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
    emitter.emit("rb_comments", vec![
        Value::Entity(comment_id),
        text_val,
        Value::Entity(loc_id),
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocql_database::Database;
    use crate::schema::ruby_schema;

    fn extract_test_file(filename: &str) -> Database {
        let source = std::fs::read(
            format!("tests/fixtures/{}", filename)
        ).unwrap();
        let schema = ruby_schema();
        let mut db = Database::from_schema(schema);
        let extractor = RubyExtractor::new();
        let result = extractor.extract_source(&mut db, filename, &source);
        assert!(result.success, "Extraction failed for {}: {:?}", filename, result.error);
        db
    }

    #[test]
    fn test_tree_structure_block() {
        let source = b"[1,2,3].each do |x|\n  puts x\nend\n";
        let mut parser = tree_sitter::Parser::new();
        let lang: tree_sitter::Language = tree_sitter_ruby::LANGUAGE.into();
        parser.set_language(&lang).unwrap();
        let tree = parser.parse(source, None).unwrap();
        fn print_node(node: &tree_sitter::Node, source: &[u8], depth: usize) {
            let indent = "  ".repeat(depth);
            let text = node.utf8_text(source).unwrap_or("");
            let short = if text.len() > 50 { &text[..50] } else { text };
            eprintln!("{}[{}] named={} children={} {:?}",
                indent, node.kind(), node.is_named(), node.child_count(), short);
            if depth < 10 {
                let mut cursor = node.walk();
                if cursor.goto_first_child() {
                    loop {
                        print_node(&cursor.node(), source, depth + 1);
                        if !cursor.goto_next_sibling() { break; }
                    }
                }
            }
        }
        print_node(&tree.root_node(), source, 0);
    }

    #[test]
    fn test_tree_structure() {
        let source = b"puts 'hello'\n";
        let mut parser = tree_sitter::Parser::new();
        let lang: tree_sitter::Language = tree_sitter_ruby::LANGUAGE.into();
        parser.set_language(&lang).unwrap();
        let tree = parser.parse(source, None).unwrap();
        fn print_node(node: &tree_sitter::Node, source: &[u8], depth: usize) {
            let indent = "  ".repeat(depth);
            let text = node.utf8_text(source).unwrap_or("");
            let short = if text.len() > 40 { &text[..40] } else { text };
            eprintln!("{}[{}] named={} children={} {:?}",
                indent, node.kind(), node.is_named(), node.child_count(), short);
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    print_node(&cursor.node(), source, depth + 1);
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        print_node(&tree.root_node(), source, 0);
    }

    #[test]
    fn test_minimal_extraction() {
        let source = b"x = 1\n";
        let schema = ruby_schema();
        let mut db = Database::from_schema(schema);
        let extractor = RubyExtractor::new();
        let result = extractor.extract_source(&mut db, "test.rb", source);
        assert!(result.success, "Minimal extraction failed: {:?}", result.error);
        eprintln!("Minimal extraction succeeded");
    }

    #[test]
    fn test_call_extraction() {
        let source = b"puts 'hello'\nrequire 'json'\n";
        let schema = ruby_schema();
        let mut db = Database::from_schema(schema);
        let extractor = RubyExtractor::new();
        let result = extractor.extract_source(&mut db, "test.rb", source);
        assert!(result.success, "Call extraction failed: {:?}", result.error);
        eprintln!("Call extraction succeeded");
    }

    #[test]
    fn test_class_extraction() {
        let source = b"class Foo\n  def bar\n    42\n  end\nend\n";
        let schema = ruby_schema();
        let mut db = Database::from_schema(schema);
        let extractor = RubyExtractor::new();
        let result = extractor.extract_source(&mut db, "test.rb", source);
        assert!(result.success, "Class extraction failed: {:?}", result.error);
        eprintln!("Class extraction succeeded");
    }

    #[test]
    fn test_block_extraction_simpler() {
        let source = b"foo do\n  bar\nend\n";
        let schema = ruby_schema();
        let mut db = Database::from_schema(schema);
        let extractor = RubyExtractor::new();
        let result = extractor.extract_source(&mut db, "test.rb", source);
        assert!(result.success);
        eprintln!("Simpler block extraction succeeded");
    }

    #[test]
    fn test_block_extraction_with_receiver() {
        let source = b"x.each do |i|\n  puts i\nend\n";
        let schema = ruby_schema();
        let mut db = Database::from_schema(schema);
        let extractor = RubyExtractor::new();
        let result = extractor.extract_source(&mut db, "test.rb", source);
        assert!(result.success);
        eprintln!("Receiver block extraction succeeded");
    }

    #[test]
    fn test_block_extraction_with_array() {
        let source = b"[1,2,3].each do |i|\n  puts i\nend\n";
        let schema = ruby_schema();
        let mut db = Database::from_schema(schema);
        let extractor = RubyExtractor::new();
        let result = extractor.extract_source(&mut db, "test.rb", source);
        assert!(result.success);
        eprintln!("Array block extraction succeeded");
    }

    #[test]
    fn test_block_extraction_simple() {
        let source = b"foo do\nend\n";
        let schema = ruby_schema();
        let mut db = Database::from_schema(schema);
        let extractor = RubyExtractor::new();
        let result = extractor.extract_source(&mut db, "test.rb", source);
        assert!(result.success, "Block extraction failed: {:?}", result.error);
        eprintln!("Simple block extraction succeeded");
    }

    #[test]
    fn test_block_extraction() {
        let source = b"[1,2,3].each do |x|\n  puts x\nend\n";
        let schema = ruby_schema();
        let mut db = Database::from_schema(schema);
        let extractor = RubyExtractor::new();
        let result = extractor.extract_source(&mut db, "test.rb", source);
        assert!(result.success, "Block extraction failed: {:?}", result.error);
        eprintln!("Block extraction succeeded");
    }

    #[test]
    fn test_if_extraction() {
        let source = b"if true\n  puts 'yes'\nelse\n  puts 'no'\nend\n";
        let schema = ruby_schema();
        let mut db = Database::from_schema(schema);
        let extractor = RubyExtractor::new();
        let result = extractor.extract_source(&mut db, "test.rb", source);
        assert!(result.success, "If extraction failed: {:?}", result.error);
        eprintln!("If extraction succeeded");
    }

    #[test]
    fn test_begin_rescue_extraction() {
        let source = b"begin\n  x = 1\nrescue => e\n  puts e\nensure\n  puts 'done'\nend\n";
        let schema = ruby_schema();
        let mut db = Database::from_schema(schema);
        let extractor = RubyExtractor::new();
        let result = extractor.extract_source(&mut db, "test.rb", source);
        assert!(result.success, "Begin/rescue extraction failed: {:?}", result.error);
        eprintln!("Begin/rescue extraction succeeded");
    }

    #[test]
    fn test_simple_modules() {
        let db = extract_test_file("simple.rb");
        let modules: Vec<_> = db.scan("rb_modules").unwrap().collect();
        let names: Vec<_> = modules.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Modules: {:?}", names);
        assert!(names.contains(&"Animals"), "Should find 'Animals' module");
    }

    #[test]
    fn test_simple_classes() {
        let db = extract_test_file("simple.rb");
        let classes: Vec<_> = db.scan("rb_classes").unwrap().collect();
        let names: Vec<_> = classes.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Classes: {:?}", names);
        assert!(names.contains(&"Animal"), "Should find 'Animal' class");
        assert!(names.contains(&"Dog"), "Should find 'Dog' class");
        assert!(names.contains(&"Cat"), "Should find 'Cat' class");
    }

    #[test]
    fn test_simple_methods() {
        let db = extract_test_file("simple.rb");
        let methods: Vec<_> = db.scan("rb_methods").unwrap().collect();
        let names: Vec<_> = methods.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Methods: {:?}", names);
        assert!(names.contains(&"initialize"), "Should find 'initialize' method");
        assert!(names.contains(&"speak"), "Should find 'speak' method");
        assert!(names.contains(&"name"), "Should find 'name' method");
    }

    #[test]
    fn test_simple_singleton_methods() {
        let db = extract_test_file("simple.rb");
        let methods: Vec<_> = db.scan("rb_singleton_methods").unwrap().collect();
        let names: Vec<_> = methods.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Singleton methods: {:?}", names);
        assert!(names.contains(&"create"), "Should find 'create' singleton method");
    }

    #[test]
    fn test_simple_params() {
        let db = extract_test_file("simple.rb");
        let params: Vec<_> = db.scan("rb_params").unwrap().collect();
        let names: Vec<_> = params.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Params: {:?}", names);
        assert!(names.contains(&"name"), "Should find param 'name'");
        assert!(names.contains(&"age"), "Should find param 'age'");
    }

    #[test]
    fn test_simple_comments() {
        let db = extract_test_file("simple.rb");
        let comments: Vec<_> = db.scan("rb_comments").unwrap().collect();
        eprintln!("Comments: {} total", comments.len());
        assert!(comments.len() >= 1, "Should have at least 1 comment");
    }

    #[test]
    fn test_simple_requires() {
        let db = extract_test_file("simple.rb");
        let requires: Vec<_> = db.scan("rb_requires").unwrap().collect();
        let paths: Vec<_> = requires.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Requires: {:?}", paths);
        assert!(paths.contains(&"json"), "Should find require 'json'");
        assert!(paths.contains(&"./helpers"), "Should find require_relative './helpers'");
    }

    #[test]
    fn test_simple_statements() {
        let db = extract_test_file("simple.rb");
        let stmts: Vec<_> = db.scan("rb_stmts").unwrap().collect();
        let kinds: Vec<i64> = stmts.iter().map(|t| t[1].as_int().unwrap()).collect();
        eprintln!("Statement kinds: {:?}", kinds);
        assert!(kinds.contains(&STMT_IF), "Should have if statements");
        assert!(kinds.contains(&STMT_WHILE), "Should have while statements");
        assert!(kinds.contains(&STMT_RETURN), "Should have return statements");
    }

    #[test]
    fn test_simple_expressions() {
        let db = extract_test_file("simple.rb");
        let exprs: Vec<_> = db.scan("rb_exprs").unwrap().collect();
        let kinds: Vec<i64> = exprs.iter().map(|t| t[1].as_int().unwrap()).collect();
        eprintln!("Expression kinds: {:?}", kinds);
        assert!(kinds.contains(&EXPR_CALL), "Should have call expressions");
        assert!(kinds.contains(&EXPR_STRING), "Should have string expressions");
        assert!(kinds.contains(&EXPR_SYMBOL), "Should have symbol expressions");
        assert!(kinds.contains(&EXPR_INTEGER), "Should have integer expressions");
    }

    #[test]
    fn test_simple_blocks() {
        let db = extract_test_file("simple.rb");
        let blocks: Vec<_> = db.scan("rb_blocks").unwrap().collect();
        eprintln!("Blocks: {} total", blocks.len());
        assert!(blocks.len() >= 1, "Should have at least 1 block");
    }

    #[test]
    fn test_simple_constants() {
        let db = extract_test_file("simple.rb");
        let constants: Vec<_> = db.scan("rb_constants").unwrap().collect();
        let names: Vec<_> = constants.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Constants: {:?}", names);
        assert!(names.contains(&"MAX_AGE"), "Should find constant 'MAX_AGE'");
    }

    #[test]
    fn test_locations_present() {
        let db = extract_test_file("simple.rb");
        let locs: Vec<_> = db.scan("locations_default").unwrap().collect();
        eprintln!("Locations: {} total", locs.len());
        assert!(locs.len() >= 10, "Should have many locations");
    }

    #[test]
    fn test_hasLocation_present() {
        let db = extract_test_file("simple.rb");
        let has_locs: Vec<_> = db.scan("hasLocation").unwrap().collect();
        eprintln!("hasLocation: {} total", has_locs.len());
        assert!(has_locs.len() >= 10, "Should have many hasLocation entries");
    }
}
