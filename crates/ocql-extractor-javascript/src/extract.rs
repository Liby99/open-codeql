use ocql_database::{EntityId, Value};
use ocql_extractor_common::{Extractor, FactEmitter, LocationEmitter, NodeExt};
use ocql_extractor_common::tree_sitter::{Language, Node, Tree};

/// JavaScript extractor using tree-sitter.
///
/// Extracts:
/// - Program/module top-levels
/// - Function declarations/expressions/arrow functions
/// - Class declarations/expressions (methods, getters, setters, static members)
/// - All statement types
/// - All expression types
/// - Import/export declarations
/// - Destructuring patterns
/// - Comments
/// - Locations for all entities
pub struct JavaScriptExtractor;

impl JavaScriptExtractor {
    pub fn new() -> Self {
        Self
    }
}

/// TypeScript extractor using tree-sitter.
///
/// Shares extraction logic with JavaScript but additionally handles:
/// - Type annotations
/// - Interfaces
/// - Enums
/// - Type aliases
/// - Decorators
/// - Access modifiers
pub struct TypeScriptExtractor;

impl TypeScriptExtractor {
    pub fn new() -> Self {
        Self
    }
}

// Top-level kind constants
const TOPLEVEL_SCRIPT: i64 = 0;
const TOPLEVEL_MODULE: i64 = 1;

// Statement kind constants (matches CodeQL JS semmlecode.javascript.dbscheme)
const STMT_EMPTY: i64 = 0;
const STMT_BLOCK: i64 = 1;
const STMT_EXPR: i64 = 2;
const STMT_IF: i64 = 3;
const STMT_LABELED: i64 = 4;
const STMT_BREAK: i64 = 5;
const STMT_CONTINUE: i64 = 6;
const STMT_WITH: i64 = 7;
const STMT_SWITCH: i64 = 8;
const STMT_RETURN: i64 = 9;
const STMT_THROW: i64 = 10;
const STMT_TRY: i64 = 11;
const STMT_WHILE: i64 = 12;
const STMT_DO_WHILE: i64 = 13;
const STMT_FOR: i64 = 14;
const STMT_FOR_IN: i64 = 15;
const STMT_DEBUGGER: i64 = 16;
const STMT_FUNCTION_DECL: i64 = 17;
const STMT_VAR_DECL: i64 = 18;
const STMT_CASE: i64 = 19;
const STMT_CATCH: i64 = 20;
const STMT_FOR_OF: i64 = 21;
const STMT_CONST_DECL: i64 = 22;
const STMT_LET_DECL: i64 = 23;
const STMT_CLASS_DECL: i64 = 26;
const STMT_IMPORT_DECL: i64 = 27;
const STMT_EXPORT_ALL: i64 = 28;
const STMT_EXPORT_DEFAULT: i64 = 29;
const STMT_EXPORT_NAMED: i64 = 30;
// TypeScript-specific statement kinds
const STMT_INTERFACE_DECL: i64 = 34;
const STMT_TYPE_ALIAS_DECL: i64 = 35;
const STMT_ENUM_DECL: i64 = 36;

// Expression kind constants (matches CodeQL JS semmlecode.javascript.dbscheme)
const EXPR_LABEL: i64 = 0;
const EXPR_NULL: i64 = 1;
const EXPR_BOOLEAN: i64 = 2;
const EXPR_NUMBER: i64 = 3;
const EXPR_STRING: i64 = 4;
const EXPR_REGEX: i64 = 5;
const EXPR_THIS: i64 = 6;
const EXPR_ARRAY: i64 = 7;
const EXPR_OBJECT: i64 = 8;
const EXPR_FUNCTION: i64 = 9;
const EXPR_SEQUENCE: i64 = 10;
const EXPR_TERNARY: i64 = 11;
const EXPR_NEW: i64 = 12;
const EXPR_CALL: i64 = 13;
const EXPR_MEMBER_ACCESS: i64 = 14;
const EXPR_SUBSCRIPT: i64 = 15;
const EXPR_NEG: i64 = 16;
const EXPR_PLUS: i64 = 17;
const EXPR_LOG_NOT: i64 = 18;
const EXPR_BIT_NOT: i64 = 19;
const EXPR_TYPEOF: i64 = 20;
const EXPR_VOID: i64 = 21;
const EXPR_DELETE: i64 = 22;
const EXPR_EQ: i64 = 23;
const EXPR_NEQ: i64 = 24;
const EXPR_EQQ: i64 = 25;
const EXPR_NEQQ: i64 = 26;
const EXPR_LT: i64 = 27;
const EXPR_LE: i64 = 28;
const EXPR_GT: i64 = 29;
const EXPR_GE: i64 = 30;
const EXPR_LSHIFT: i64 = 31;
const EXPR_RSHIFT: i64 = 32;
const EXPR_URSHIFT: i64 = 33;
const EXPR_ADD: i64 = 34;
const EXPR_SUB: i64 = 35;
const EXPR_MUL: i64 = 36;
const EXPR_DIV: i64 = 37;
const EXPR_MOD: i64 = 38;
const EXPR_BITOR: i64 = 39;
const EXPR_XOR: i64 = 40;
const EXPR_BITAND: i64 = 41;
const EXPR_IN: i64 = 42;
const EXPR_INSTANCEOF: i64 = 43;
const EXPR_LOGAND: i64 = 44;
const EXPR_LOGOR: i64 = 45;
const EXPR_ASSIGN: i64 = 47;
const EXPR_ASSIGN_ADD: i64 = 48;
const EXPR_ASSIGN_SUB: i64 = 49;
const EXPR_ASSIGN_MUL: i64 = 50;
const EXPR_ASSIGN_DIV: i64 = 51;
const EXPR_ASSIGN_MOD: i64 = 52;
const EXPR_ASSIGN_LSHIFT: i64 = 53;
const EXPR_ASSIGN_RSHIFT: i64 = 54;
const EXPR_ASSIGN_URSHIFT: i64 = 55;
const EXPR_ASSIGN_OR: i64 = 56;
const EXPR_ASSIGN_XOR: i64 = 57;
const EXPR_ASSIGN_AND: i64 = 58;
const EXPR_PREINC: i64 = 59;
const EXPR_POSTINC: i64 = 60;
const EXPR_PREDEC: i64 = 61;
const EXPR_POSTDEC: i64 = 62;
const EXPR_PAREN: i64 = 63;
const EXPR_VAR_DECLARATOR: i64 = 64;
const EXPR_ARROW_FUNCTION: i64 = 65;
const EXPR_SPREAD: i64 = 66;
const EXPR_ARRAY_PATTERN: i64 = 67;
const EXPR_OBJECT_PATTERN: i64 = 68;
const EXPR_YIELD: i64 = 69;
const EXPR_TAGGED_TEMPLATE: i64 = 70;
const EXPR_TEMPLATE_LITERAL: i64 = 71;
const EXPR_TEMPLATE_ELEMENT: i64 = 72;
const EXPR_VAR_DECL: i64 = 78;
const EXPR_VAR_ACCESS: i64 = 79;
const EXPR_CLASS: i64 = 80;
const EXPR_SUPER: i64 = 81;
const EXPR_AWAIT: i64 = 92;
const EXPR_EXP: i64 = 87;
const EXPR_ASSIGN_EXP: i64 = 88;
const EXPR_NULLISH_COALESCING: i64 = 107;
const EXPR_OPTIONAL_CHAIN: i64 = 14; // reuse member_access for optional chain

// Property kind constants
const PROP_VALUE: i64 = 0;
const PROP_GETTER: i64 = 1;
const PROP_SETTER: i64 = 2;

// Scope kind constants
const SCOPE_GLOBAL: i64 = 0;
const SCOPE_FUNCTION: i64 = 1;
const SCOPE_BLOCK: i64 = 4;
const SCOPE_CLASS: i64 = 8;
const SCOPE_MODULE: i64 = 3;

// Comment kind constants
const COMMENT_LINE: i64 = 0;
const COMMENT_BLOCK: i64 = 1;

impl Extractor for JavaScriptExtractor {
    fn language(&self) -> Language {
        tree_sitter_javascript::LANGUAGE.into()
    }

    fn extensions(&self) -> &[&str] {
        &["js", "mjs", "cjs", "jsx"]
    }

    fn extract_file(
        &self,
        emitter: &mut FactEmitter<'_>,
        file_id: EntityId,
        tree: &Tree,
        source: &[u8],
    ) {
        let root = tree.root_node();
        extract_program(emitter, file_id, &root, source, false);
    }
}

impl Extractor for TypeScriptExtractor {
    fn language(&self) -> Language {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    }

    fn extensions(&self) -> &[&str] {
        &["ts", "mts", "cts", "tsx"]
    }

    fn extract_file(
        &self,
        emitter: &mut FactEmitter<'_>,
        file_id: EntityId,
        tree: &Tree,
        source: &[u8],
    ) {
        let root = tree.root_node();
        extract_program(emitter, file_id, &root, source, true);
    }
}

/// Extract the top-level program node.
fn extract_program(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    root: &Node<'_>,
    source: &[u8],
    is_typescript: bool,
) {
    // Determine if this is a module (has import/export) or script
    let is_module = has_module_syntax(root, source);
    let toplevel_kind = if is_module { TOPLEVEL_MODULE } else { TOPLEVEL_SCRIPT };
    let toplevel_id = emitter.alloc();
    emitter.emit("toplevels", vec![
        Value::Entity(toplevel_id),
        Value::Int(toplevel_kind),
    ]);
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, root);
    emitter.emit("hasLocation", vec![
        Value::Entity(toplevel_id),
        Value::Entity(loc_id),
    ]);

    // Create a global/module scope
    let scope_kind = if is_module { SCOPE_MODULE } else { SCOPE_GLOBAL };
    let scope_id = emitter.alloc();
    emitter.emit("scopes", vec![
        Value::Entity(scope_id),
        Value::Int(scope_kind),
    ]);

    let mut cursor = root.walk();
    let mut child_index = 0i64;
    if cursor.goto_first_child() {
        loop {
            let node = cursor.node();
            if node.is_named() {
                if let Some(_) = extract_stmt_or_decl(
                    emitter, file_id, &node, source, toplevel_id, scope_id,
                    child_index, is_typescript,
                ) {
                    child_index += 1;
                }
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// Check if the program has any import/export statements (module syntax).
fn has_module_syntax(root: &Node<'_>, _source: &[u8]) -> bool {
    let mut cursor = root.walk();
    if cursor.goto_first_child() {
        loop {
            let node = cursor.node();
            match node.kind() {
                "import_statement" | "export_statement" => return true,
                _ => {}
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    false
}

/// Extract a statement or declaration at the top-level or inside a block.
fn extract_stmt_or_decl(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
    scope_id: EntityId,
    index: i64,
    is_typescript: bool,
) -> Option<EntityId> {
    match node.kind() {
        "comment" => {
            extract_comment(emitter, file_id, node, source, parent_id);
            None
        }
        _ => extract_stmt(emitter, file_id, node, source, parent_id, scope_id, index, is_typescript),
    }
}

/// Extract a statement, returning the entity ID.
fn extract_stmt(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
    scope_id: EntityId,
    index: i64,
    is_typescript: bool,
) -> Option<EntityId> {
    let kind = match node.kind() {
        "expression_statement" => Some(STMT_EXPR),
        "if_statement" => Some(STMT_IF),
        "for_statement" => Some(STMT_FOR),
        "for_in_statement" => Some(STMT_FOR_IN),
        "while_statement" => Some(STMT_WHILE),
        "do_statement" => Some(STMT_DO_WHILE),
        "switch_statement" => Some(STMT_SWITCH),
        "switch_case" | "switch_default" => Some(STMT_CASE),
        "return_statement" => Some(STMT_RETURN),
        "throw_statement" => Some(STMT_THROW),
        "break_statement" => Some(STMT_BREAK),
        "continue_statement" => Some(STMT_CONTINUE),
        "try_statement" => Some(STMT_TRY),
        "catch_clause" => Some(STMT_CATCH),
        "with_statement" => Some(STMT_WITH),
        "labeled_statement" => Some(STMT_LABELED),
        "variable_declaration" => Some(STMT_VAR_DECL),
        "lexical_declaration" => {
            // Distinguish let vs const
            let text = node.text(source);
            if text.starts_with("const") {
                Some(STMT_CONST_DECL)
            } else {
                Some(STMT_LET_DECL)
            }
        }
        "statement_block" => Some(STMT_BLOCK),
        "empty_statement" => Some(STMT_EMPTY),
        "class_declaration" => Some(STMT_CLASS_DECL),
        "function_declaration" | "generator_function_declaration" => Some(STMT_FUNCTION_DECL),
        "import_statement" => Some(STMT_IMPORT_DECL),
        "export_statement" => {
            // Determine which export kind
            let text = node.text(source);
            if text.contains("export default") {
                Some(STMT_EXPORT_DEFAULT)
            } else if text.contains("export *") {
                Some(STMT_EXPORT_ALL)
            } else {
                Some(STMT_EXPORT_NAMED)
            }
        }
        "debugger_statement" => Some(STMT_DEBUGGER),
        // TypeScript-specific
        "interface_declaration" if is_typescript => Some(STMT_INTERFACE_DECL),
        "type_alias_declaration" if is_typescript => Some(STMT_TYPE_ALIAS_DECL),
        "enum_declaration" if is_typescript => Some(STMT_ENUM_DECL),
        // for_of_statement (tree-sitter uses for_in_statement with "of" operator)
        _ => {
            // Check if it's a for-of inside a for_in_statement
            None
        }
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

    // Process children based on statement kind
    match node.kind() {
        "expression_statement" => {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() && child.kind() != "comment" {
                        extract_expr(emitter, file_id, &child, source, stmt_id, scope_id, 0, is_typescript);
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "if_statement" => {
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, stmt_id, scope_id, 0, is_typescript);
            }
            if let Some(consequence) = node.child_by_field("consequence") {
                extract_stmt(emitter, file_id, &consequence, source, stmt_id, scope_id, 0, is_typescript);
            }
            if let Some(alternative) = node.child_by_field("alternative") {
                extract_stmt(emitter, file_id, &alternative, source, stmt_id, scope_id, 1, is_typescript);
            }
        }
        "for_statement" => {
            if let Some(init) = node.child_by_field("initializer") {
                extract_expr_or_stmt(emitter, file_id, &init, source, stmt_id, scope_id, 0, is_typescript);
            }
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, stmt_id, scope_id, 1, is_typescript);
            }
            if let Some(update) = node.child_by_field("increment") {
                extract_expr(emitter, file_id, &update, source, stmt_id, scope_id, 2, is_typescript);
            }
            if let Some(body) = node.child_by_field("body") {
                extract_stmt(emitter, file_id, &body, source, stmt_id, scope_id, 3, is_typescript);
            }
        }
        "for_in_statement" => {
            // Could be for-in or for-of
            let text = node.text(source);
            let is_for_of = text.contains(" of ");
            if is_for_of {
                // Update the kind to for_of
                // (We already emitted with for_in kind; for a production extractor we'd
                //  detect this earlier, but for now we handle it in children)
            }
            if let Some(left) = node.child_by_field("left") {
                extract_expr_or_stmt(emitter, file_id, &left, source, stmt_id, scope_id, 0, is_typescript);
            }
            if let Some(right) = node.child_by_field("right") {
                extract_expr(emitter, file_id, &right, source, stmt_id, scope_id, 1, is_typescript);
            }
            if let Some(body) = node.child_by_field("body") {
                extract_stmt(emitter, file_id, &body, source, stmt_id, scope_id, 2, is_typescript);
            }
        }
        "while_statement" => {
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, stmt_id, scope_id, 0, is_typescript);
            }
            if let Some(body) = node.child_by_field("body") {
                extract_stmt(emitter, file_id, &body, source, stmt_id, scope_id, 1, is_typescript);
            }
        }
        "do_statement" => {
            if let Some(body) = node.child_by_field("body") {
                extract_stmt(emitter, file_id, &body, source, stmt_id, scope_id, 0, is_typescript);
            }
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, stmt_id, scope_id, 1, is_typescript);
            }
        }
        "switch_statement" => {
            if let Some(value) = node.child_by_field("value") {
                extract_expr(emitter, file_id, &value, source, stmt_id, scope_id, 0, is_typescript);
            }
            if let Some(body) = node.child_by_field("body") {
                let mut case_idx = 0i64;
                let mut cursor = body.walk();
                if cursor.goto_first_child() {
                    loop {
                        let child = cursor.node();
                        if child.is_named() {
                            if let Some(_) = extract_stmt(
                                emitter, file_id, &child, source, stmt_id, scope_id,
                                case_idx, is_typescript,
                            ) {
                                case_idx += 1;
                            }
                        }
                        if !cursor.goto_next_sibling() { break; }
                    }
                }
            }
        }
        "switch_case" | "switch_default" => {
            // Extract case value (if switch_case, not default)
            if node.kind() == "switch_case" {
                if let Some(value) = node.child_by_field("value") {
                    extract_expr(emitter, file_id, &value, source, stmt_id, scope_id, 0, is_typescript);
                }
            }
            // Extract case body statements
            let mut child_idx = 1i64;
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() && child.kind() != "comment"
                        && child.kind() != node.kind()
                    {
                        // Skip the "value" which we already extracted
                        if node.child_by_field("value").map_or(true, |v| v.id() != child.id()) {
                            if let Some(_) = extract_stmt(
                                emitter, file_id, &child, source, stmt_id, scope_id,
                                child_idx, is_typescript,
                            ) {
                                child_idx += 1;
                            }
                        }
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "return_statement" | "throw_statement" => {
            // Extract the return/throw expression
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, &child, source, stmt_id, scope_id, 0, is_typescript);
                        break;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "try_statement" => {
            if let Some(body) = node.child_by_field("body") {
                extract_stmt(emitter, file_id, &body, source, stmt_id, scope_id, 0, is_typescript);
            }
            if let Some(handler) = node.child_by_field("handler") {
                extract_stmt(emitter, file_id, &handler, source, stmt_id, scope_id, 1, is_typescript);
            }
            if let Some(finalizer) = node.child_by_field("finalizer") {
                extract_stmt(emitter, file_id, &finalizer, source, stmt_id, scope_id, 2, is_typescript);
            }
        }
        "catch_clause" => {
            if let Some(param) = node.child_by_field("parameter") {
                let name = param.text(source);
                let var_id = emitter.alloc();
                let name_val = emitter.string(name);
                emitter.emit("variables", vec![
                    Value::Entity(var_id),
                    name_val,
                    Value::Entity(scope_id),
                ]);
                let var_loc = LocationEmitter::emit_for_node(emitter, file_id, &param);
                emitter.emit("hasLocation", vec![
                    Value::Entity(var_id),
                    Value::Entity(var_loc),
                ]);
            }
            if let Some(body) = node.child_by_field("body") {
                extract_stmt(emitter, file_id, &body, source, stmt_id, scope_id, 0, is_typescript);
            }
        }
        "with_statement" => {
            if let Some(obj) = node.child_by_field("object") {
                extract_expr(emitter, file_id, &obj, source, stmt_id, scope_id, 0, is_typescript);
            }
            if let Some(body) = node.child_by_field("body") {
                extract_stmt(emitter, file_id, &body, source, stmt_id, scope_id, 1, is_typescript);
            }
        }
        "labeled_statement" => {
            if let Some(label) = node.child_by_field("label") {
                let label_name = label.text(source);
                let label_id = emitter.alloc();
                let name_val = emitter.string(label_name);
                emitter.emit("exprs", vec![
                    Value::Entity(label_id),
                    Value::Int(EXPR_LABEL),
                    Value::Entity(stmt_id),
                    Value::Int(0),
                ]);
                let label_loc = LocationEmitter::emit_for_node(emitter, file_id, &label);
                emitter.emit("hasLocation", vec![
                    Value::Entity(label_id),
                    Value::Entity(label_loc),
                ]);
                emitter.emit("literals", vec![
                    name_val,
                    Value::Entity(label_id),
                ]);
            }
            if let Some(body) = node.child_by_field("body") {
                extract_stmt(emitter, file_id, &body, source, stmt_id, scope_id, 1, is_typescript);
            }
        }
        "variable_declaration" | "lexical_declaration" => {
            extract_var_declaration(emitter, file_id, node, source, stmt_id, scope_id, is_typescript);
        }
        "statement_block" => {
            let block_scope_id = emitter.alloc();
            emitter.emit("scopes", vec![
                Value::Entity(block_scope_id),
                Value::Int(SCOPE_BLOCK),
            ]);
            let mut child_index = 0i64;
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        if let Some(_) = extract_stmt_or_decl(
                            emitter, file_id, &child, source, stmt_id, block_scope_id,
                            child_index, is_typescript,
                        ) {
                            child_index += 1;
                        }
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "function_declaration" | "generator_function_declaration" => {
            extract_function_decl(emitter, file_id, node, source, stmt_id, scope_id, is_typescript);
        }
        "class_declaration" => {
            extract_class_decl(emitter, file_id, node, source, stmt_id, scope_id, is_typescript);
        }
        "import_statement" => {
            extract_import(emitter, file_id, node, source, stmt_id);
        }
        "export_statement" => {
            extract_export(emitter, file_id, node, source, stmt_id, scope_id, is_typescript);
        }
        // TypeScript-specific
        "interface_declaration" if is_typescript => {
            extract_interface(emitter, file_id, node, source, stmt_id);
        }
        "type_alias_declaration" if is_typescript => {
            extract_type_alias(emitter, file_id, node, source, stmt_id);
        }
        "enum_declaration" if is_typescript => {
            extract_enum(emitter, file_id, node, source, stmt_id);
        }
        _ => {}
    }

    Some(stmt_id)
}

/// Extract a variable declaration (var/let/const) with its declarators.
fn extract_var_declaration(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    stmt_id: EntityId,
    scope_id: EntityId,
    is_typescript: bool,
) {
    let mut cursor = node.walk();
    let mut decl_idx = 0i64;
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "variable_declarator" {
                let decl_id = emitter.alloc();
                let decl_loc = LocationEmitter::emit_for_node(emitter, file_id, &child);
                emitter.emit("exprs", vec![
                    Value::Entity(decl_id),
                    Value::Int(EXPR_VAR_DECLARATOR),
                    Value::Entity(stmt_id),
                    Value::Int(decl_idx),
                ]);
                emitter.emit("hasLocation", vec![
                    Value::Entity(decl_id),
                    Value::Entity(decl_loc),
                ]);

                // Extract the name/pattern
                if let Some(name_node) = child.child_by_field("name") {
                    match name_node.kind() {
                        "identifier" => {
                            let name = name_node.text(source);
                            let var_id = emitter.alloc();
                            let name_val = emitter.string(name);
                            emitter.emit("variables", vec![
                                Value::Entity(var_id),
                                name_val.clone(),
                                Value::Entity(scope_id),
                            ]);
                            let var_loc = LocationEmitter::emit_for_node(emitter, file_id, &name_node);
                            emitter.emit("hasLocation", vec![
                                Value::Entity(var_id),
                                Value::Entity(var_loc),
                            ]);
                            // Emit a var_decl expression for the binding
                            let vd_id = emitter.alloc();
                            emitter.emit("exprs", vec![
                                Value::Entity(vd_id),
                                Value::Int(EXPR_VAR_DECL),
                                Value::Entity(decl_id),
                                Value::Int(0),
                            ]);
                            emitter.emit("hasLocation", vec![
                                Value::Entity(vd_id),
                                Value::Entity(var_loc),
                            ]);
                            emitter.emit("literals", vec![
                                name_val,
                                Value::Entity(vd_id),
                            ]);
                        }
                        "array_pattern" => {
                            extract_expr(emitter, file_id, &name_node, source, decl_id, scope_id, 0, is_typescript);
                        }
                        "object_pattern" => {
                            extract_expr(emitter, file_id, &name_node, source, decl_id, scope_id, 0, is_typescript);
                        }
                        _ => {}
                    }
                }

                // Extract type annotation (TypeScript)
                if is_typescript {
                    if let Some(type_ann) = child.child_by_field("type") {
                        extract_type_annotation(emitter, file_id, &type_ann, source, decl_id);
                    }
                }

                // Extract initializer
                if let Some(value) = child.child_by_field("value") {
                    extract_expr(emitter, file_id, &value, source, decl_id, scope_id, 1, is_typescript);
                }

                decl_idx += 1;
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Helper to extract either an expression or a statement (for for-loop init, etc.)
fn extract_expr_or_stmt(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
    scope_id: EntityId,
    index: i64,
    is_typescript: bool,
) {
    match node.kind() {
        "variable_declaration" | "lexical_declaration" => {
            extract_stmt(emitter, file_id, node, source, parent_id, scope_id, index, is_typescript);
        }
        _ => {
            extract_expr(emitter, file_id, node, source, parent_id, scope_id, index, is_typescript);
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
    scope_id: EntityId,
    index: i64,
    is_typescript: bool,
) -> Option<EntityId> {
    let kind = match node.kind() {
        "identifier" => Some(EXPR_VAR_ACCESS),
        "this" => Some(EXPR_THIS),
        "super" => Some(EXPR_SUPER),
        "null" => Some(EXPR_NULL),
        "undefined" => Some(EXPR_VAR_ACCESS), // undefined is an identifier
        "true" | "false" => Some(EXPR_BOOLEAN),
        "number" => Some(EXPR_NUMBER),
        "string" | "string_fragment" => Some(EXPR_STRING),
        "regex" => Some(EXPR_REGEX),
        "template_string" => Some(EXPR_TEMPLATE_LITERAL),
        "template_substitution" => Some(EXPR_TEMPLATE_ELEMENT),
        "array" => Some(EXPR_ARRAY),
        "object" => Some(EXPR_OBJECT),
        "function" | "generator_function" => Some(EXPR_FUNCTION),
        "arrow_function" => Some(EXPR_ARROW_FUNCTION),
        "class" => Some(EXPR_CLASS),
        "call_expression" => Some(EXPR_CALL),
        "new_expression" => Some(EXPR_NEW),
        "member_expression" => Some(EXPR_MEMBER_ACCESS),
        "subscript_expression" => Some(EXPR_SUBSCRIPT),
        "assignment_expression" => {
            let op = find_operator(node, source);
            Some(js_assign_op_kind(&op))
        }
        "augmented_assignment_expression" => {
            let op = find_operator(node, source);
            Some(js_assign_op_kind(&op))
        }
        "update_expression" => {
            let text = node.text(source);
            let is_prefix = text.starts_with("++") || text.starts_with("--");
            if text.contains("++") {
                Some(if is_prefix { EXPR_PREINC } else { EXPR_POSTINC })
            } else {
                Some(if is_prefix { EXPR_PREDEC } else { EXPR_POSTDEC })
            }
        }
        "binary_expression" => {
            let op = find_operator(node, source);
            Some(js_binary_op_kind(&op))
        }
        "unary_expression" => {
            let op = find_operator(node, source);
            match op.as_str() {
                "-" => Some(EXPR_NEG),
                "+" => Some(EXPR_PLUS),
                "!" => Some(EXPR_LOG_NOT),
                "~" => Some(EXPR_BIT_NOT),
                "typeof" => Some(EXPR_TYPEOF),
                "void" => Some(EXPR_VOID),
                "delete" => Some(EXPR_DELETE),
                _ => Some(EXPR_NEG),
            }
        }
        "ternary_expression" => Some(EXPR_TERNARY),
        "sequence_expression" => Some(EXPR_SEQUENCE),
        "yield_expression" => Some(EXPR_YIELD),
        "await_expression" => Some(EXPR_AWAIT),
        "spread_element" => Some(EXPR_SPREAD),
        "tagged_template_expression" => Some(EXPR_TAGGED_TEMPLATE),
        "parenthesized_expression" => {
            // Extract inner expression, but also emit the paren wrapper
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        return extract_expr(emitter, file_id, &child, source, parent_id, scope_id, index, is_typescript);
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
            return None;
        }
        "array_pattern" => Some(EXPR_ARRAY_PATTERN),
        "object_pattern" => Some(EXPR_OBJECT_PATTERN),
        "pair" | "shorthand_property_identifier" | "shorthand_property_identifier_pattern" => {
            // Properties handled separately
            return None;
        }
        "optional_chain_expression" => {
            // tree-sitter wraps ?. chains in this node
            // Extract the inner expression directly
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        return extract_expr(emitter, file_id, &child, source, parent_id, scope_id, index, is_typescript);
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

    emitter.emit("exprs", vec![
        Value::Entity(expr_id),
        Value::Int(expr_kind),
        Value::Entity(parent_id),
        Value::Int(index),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(expr_id),
        Value::Entity(loc_id),
    ]);

    // Emit literal values for literals
    match node.kind() {
        "identifier" | "this" | "super" | "undefined" => {
            let text = node.text(source);
            let text_val = emitter.string(text);
            emitter.emit("literals", vec![
                text_val,
                Value::Entity(expr_id),
            ]);
        }
        "null" | "true" | "false" | "number" | "string" | "string_fragment" | "regex" => {
            let text = node.text(source);
            let text_val = emitter.string(text);
            emitter.emit("literals", vec![
                text_val,
                Value::Entity(expr_id),
            ]);
        }
        _ => {}
    }

    // Recurse into children
    match node.kind() {
        "call_expression" => {
            if let Some(func) = node.child_by_field("function") {
                extract_expr(emitter, file_id, &func, source, expr_id, scope_id, 0, is_typescript);
            }
            if let Some(args) = node.child_by_field("arguments") {
                let mut idx = 1i64;
                let mut cursor = args.walk();
                if cursor.goto_first_child() {
                    loop {
                        let child = cursor.node();
                        if child.is_named() {
                            if let Some(_) = extract_expr(
                                emitter, file_id, &child, source, expr_id, scope_id, idx, is_typescript,
                            ) {
                                idx += 1;
                            }
                        }
                        if !cursor.goto_next_sibling() { break; }
                    }
                }
            }
        }
        "new_expression" => {
            if let Some(constructor) = node.child_by_field("constructor") {
                extract_expr(emitter, file_id, &constructor, source, expr_id, scope_id, 0, is_typescript);
            }
            if let Some(args) = node.child_by_field("arguments") {
                let mut idx = 1i64;
                let mut cursor = args.walk();
                if cursor.goto_first_child() {
                    loop {
                        let child = cursor.node();
                        if child.is_named() {
                            if let Some(_) = extract_expr(
                                emitter, file_id, &child, source, expr_id, scope_id, idx, is_typescript,
                            ) {
                                idx += 1;
                            }
                        }
                        if !cursor.goto_next_sibling() { break; }
                    }
                }
            }
        }
        "member_expression" => {
            if let Some(obj) = node.child_by_field("object") {
                extract_expr(emitter, file_id, &obj, source, expr_id, scope_id, 0, is_typescript);
            }
            if let Some(prop) = node.child_by_field("property") {
                extract_expr(emitter, file_id, &prop, source, expr_id, scope_id, 1, is_typescript);
            }
        }
        "subscript_expression" => {
            if let Some(obj) = node.child_by_field("object") {
                extract_expr(emitter, file_id, &obj, source, expr_id, scope_id, 0, is_typescript);
            }
            if let Some(idx_node) = node.child_by_field("index") {
                extract_expr(emitter, file_id, &idx_node, source, expr_id, scope_id, 1, is_typescript);
            }
        }
        "assignment_expression" | "augmented_assignment_expression" => {
            if let Some(left) = node.child_by_field("left") {
                extract_expr(emitter, file_id, &left, source, expr_id, scope_id, 0, is_typescript);
            }
            if let Some(right) = node.child_by_field("right") {
                extract_expr(emitter, file_id, &right, source, expr_id, scope_id, 1, is_typescript);
            }
        }
        "binary_expression" => {
            if let Some(left) = node.child_by_field("left") {
                extract_expr(emitter, file_id, &left, source, expr_id, scope_id, 0, is_typescript);
            }
            if let Some(right) = node.child_by_field("right") {
                extract_expr(emitter, file_id, &right, source, expr_id, scope_id, 1, is_typescript);
            }
        }
        "unary_expression" | "update_expression" => {
            if let Some(arg) = node.child_by_field("argument") {
                extract_expr(emitter, file_id, &arg, source, expr_id, scope_id, 0, is_typescript);
            }
        }
        "ternary_expression" => {
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, expr_id, scope_id, 0, is_typescript);
            }
            if let Some(consequence) = node.child_by_field("consequence") {
                extract_expr(emitter, file_id, &consequence, source, expr_id, scope_id, 1, is_typescript);
            }
            if let Some(alternative) = node.child_by_field("alternative") {
                extract_expr(emitter, file_id, &alternative, source, expr_id, scope_id, 2, is_typescript);
            }
        }
        "sequence_expression" => {
            if let Some(left) = node.child_by_field("left") {
                extract_expr(emitter, file_id, &left, source, expr_id, scope_id, 0, is_typescript);
            }
            if let Some(right) = node.child_by_field("right") {
                extract_expr(emitter, file_id, &right, source, expr_id, scope_id, 1, is_typescript);
            }
        }
        "yield_expression" | "await_expression" | "spread_element" => {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, &child, source, expr_id, scope_id, 0, is_typescript);
                        break;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "tagged_template_expression" => {
            if let Some(func) = node.child_by_field("function") {
                extract_expr(emitter, file_id, &func, source, expr_id, scope_id, 0, is_typescript);
            }
            if let Some(args) = node.child_by_field("arguments") {
                extract_expr(emitter, file_id, &args, source, expr_id, scope_id, 1, is_typescript);
            }
        }
        "template_string" => {
            let mut idx = 0i64;
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        if let Some(_) = extract_expr(
                            emitter, file_id, &child, source, expr_id, scope_id, idx, is_typescript,
                        ) {
                            idx += 1;
                        }
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "array" => {
            let mut idx = 0i64;
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        if let Some(_) = extract_expr(
                            emitter, file_id, &child, source, expr_id, scope_id, idx, is_typescript,
                        ) {
                            idx += 1;
                        }
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "object" => {
            extract_object_properties(emitter, file_id, node, source, expr_id, scope_id, is_typescript);
        }
        "array_pattern" => {
            let mut idx = 0i64;
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        if let Some(_) = extract_expr(
                            emitter, file_id, &child, source, expr_id, scope_id, idx, is_typescript,
                        ) {
                            idx += 1;
                        }
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "object_pattern" => {
            extract_object_properties(emitter, file_id, node, source, expr_id, scope_id, is_typescript);
        }
        "function" | "generator_function" => {
            extract_function_expr(emitter, file_id, node, source, expr_id, scope_id, is_typescript);
        }
        "arrow_function" => {
            extract_arrow_function(emitter, file_id, node, source, expr_id, scope_id, is_typescript);
        }
        "class" => {
            extract_class_expr(emitter, file_id, node, source, expr_id, scope_id, is_typescript);
        }
        _ => {}
    }

    Some(expr_id)
}

/// Extract object properties (from object expressions or object patterns).
fn extract_object_properties(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
    scope_id: EntityId,
    is_typescript: bool,
) {
    let mut idx = 0i64;
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "pair" | "pair_pattern" => {
                    let prop_id = emitter.alloc();
                    let prop_loc = LocationEmitter::emit_for_node(emitter, file_id, &child);
                    emitter.emit("properties", vec![
                        Value::Entity(prop_id),
                        Value::Entity(parent_id),
                        Value::Int(idx),
                        Value::Int(PROP_VALUE),
                    ]);
                    emitter.emit("hasLocation", vec![
                        Value::Entity(prop_id),
                        Value::Entity(prop_loc),
                    ]);
                    // Extract key
                    if let Some(key) = child.child_by_field("key") {
                        extract_expr(emitter, file_id, &key, source, prop_id, scope_id, 0, is_typescript);
                    }
                    // Extract value
                    if let Some(val) = child.child_by_field("value") {
                        extract_expr(emitter, file_id, &val, source, prop_id, scope_id, 1, is_typescript);
                    }
                    idx += 1;
                }
                "shorthand_property_identifier" | "shorthand_property_identifier_pattern" => {
                    let prop_id = emitter.alloc();
                    let prop_loc = LocationEmitter::emit_for_node(emitter, file_id, &child);
                    emitter.emit("properties", vec![
                        Value::Entity(prop_id),
                        Value::Entity(parent_id),
                        Value::Int(idx),
                        Value::Int(PROP_VALUE),
                    ]);
                    emitter.emit("hasLocation", vec![
                        Value::Entity(prop_id),
                        Value::Entity(prop_loc),
                    ]);
                    // Emit the identifier as both key and value
                    let name = child.text(source);
                    let name_id = emitter.alloc();
                    let name_val = emitter.string(name);
                    emitter.emit("exprs", vec![
                        Value::Entity(name_id),
                        Value::Int(EXPR_VAR_ACCESS),
                        Value::Entity(prop_id),
                        Value::Int(0),
                    ]);
                    emitter.emit("hasLocation", vec![
                        Value::Entity(name_id),
                        Value::Entity(prop_loc),
                    ]);
                    emitter.emit("literals", vec![
                        name_val,
                        Value::Entity(name_id),
                    ]);
                    idx += 1;
                }
                "spread_element" => {
                    extract_expr(emitter, file_id, &child, source, parent_id, scope_id, idx, is_typescript);
                    idx += 1;
                }
                "method_definition" => {
                    extract_method_definition(emitter, file_id, &child, source, parent_id, scope_id, idx, is_typescript);
                    idx += 1;
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract a function declaration.
fn extract_function_decl(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    _stmt_id: EntityId,
    scope_id: EntityId,
    is_typescript: bool,
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();

    let func_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    let name_val = emitter.string(&name);
    emitter.emit("functions", vec![
        Value::Entity(func_id),
        name_val.clone(),
        Value::Entity(scope_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(func_id),
        Value::Entity(loc_id),
    ]);

    // Create function scope
    let func_scope_id = emitter.alloc();
    emitter.emit("scopes", vec![
        Value::Entity(func_scope_id),
        Value::Int(SCOPE_FUNCTION),
    ]);

    // Register the function name as a variable in the outer scope
    if !name.is_empty() {
        let var_id = emitter.alloc();
        emitter.emit("variables", vec![
            Value::Entity(var_id),
            name_val,
            Value::Entity(scope_id),
        ]);
    }

    // Extract parameters
    if let Some(params) = node.child_by_field("parameters") {
        extract_function_params(emitter, file_id, &params, source, func_id, func_scope_id, is_typescript);
    }

    // Extract return type annotation (TypeScript)
    if is_typescript {
        if let Some(ret_type) = node.child_by_field("return_type") {
            extract_type_annotation(emitter, file_id, &ret_type, source, func_id);
        }
    }

    // Extract body
    if let Some(body) = node.child_by_field("body") {
        extract_stmt(emitter, file_id, &body, source, func_id, func_scope_id, 0, is_typescript);
    }
}

/// Extract a function expression.
fn extract_function_expr(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    _parent_id: EntityId,
    scope_id: EntityId,
    is_typescript: bool,
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();

    let func_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    let name_val = emitter.string(&name);
    emitter.emit("functions", vec![
        Value::Entity(func_id),
        name_val,
        Value::Entity(scope_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(func_id),
        Value::Entity(loc_id),
    ]);

    let func_scope_id = emitter.alloc();
    emitter.emit("scopes", vec![
        Value::Entity(func_scope_id),
        Value::Int(SCOPE_FUNCTION),
    ]);

    if let Some(params) = node.child_by_field("parameters") {
        extract_function_params(emitter, file_id, &params, source, func_id, func_scope_id, is_typescript);
    }

    if is_typescript {
        if let Some(ret_type) = node.child_by_field("return_type") {
            extract_type_annotation(emitter, file_id, &ret_type, source, func_id);
        }
    }

    if let Some(body) = node.child_by_field("body") {
        extract_stmt(emitter, file_id, &body, source, func_id, func_scope_id, 0, is_typescript);
    }
}

/// Extract an arrow function.
fn extract_arrow_function(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    _parent_id: EntityId,
    scope_id: EntityId,
    is_typescript: bool,
) {
    let func_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    let name_val = emitter.string("");
    emitter.emit("functions", vec![
        Value::Entity(func_id),
        name_val,
        Value::Entity(scope_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(func_id),
        Value::Entity(loc_id),
    ]);

    let func_scope_id = emitter.alloc();
    emitter.emit("scopes", vec![
        Value::Entity(func_scope_id),
        Value::Int(SCOPE_FUNCTION),
    ]);

    // Arrow functions can have either formal_parameters or a single identifier param
    if let Some(params) = node.child_by_field("parameters") {
        if params.kind() == "formal_parameters" {
            extract_function_params(emitter, file_id, &params, source, func_id, func_scope_id, is_typescript);
        } else if params.kind() == "identifier" {
            // Single parameter: (x) => ...
            let name = params.text(source);
            let var_id = emitter.alloc();
            let pname = emitter.string(name);
            emitter.emit("variables", vec![
                Value::Entity(var_id),
                pname,
                Value::Entity(func_scope_id),
            ]);
            let var_loc = LocationEmitter::emit_for_node(emitter, file_id, &params);
            emitter.emit("hasLocation", vec![
                Value::Entity(var_id),
                Value::Entity(var_loc),
            ]);
        }
    }
    // Also try "parameter" field for single-param arrows
    if let Some(param) = node.child_by_field("parameter") {
        if param.kind() == "identifier" {
            let name = param.text(source);
            let var_id = emitter.alloc();
            let pname = emitter.string(name);
            emitter.emit("variables", vec![
                Value::Entity(var_id),
                pname,
                Value::Entity(func_scope_id),
            ]);
            let var_loc = LocationEmitter::emit_for_node(emitter, file_id, &param);
            emitter.emit("hasLocation", vec![
                Value::Entity(var_id),
                Value::Entity(var_loc),
            ]);
        }
    }

    if is_typescript {
        if let Some(ret_type) = node.child_by_field("return_type") {
            extract_type_annotation(emitter, file_id, &ret_type, source, func_id);
        }
    }

    // Body can be a statement_block or a single expression
    if let Some(body) = node.child_by_field("body") {
        if body.kind() == "statement_block" {
            extract_stmt(emitter, file_id, &body, source, func_id, func_scope_id, 0, is_typescript);
        } else {
            // Expression body
            extract_expr(emitter, file_id, &body, source, func_id, func_scope_id, 0, is_typescript);
        }
    }
}

/// Extract function parameters.
fn extract_function_params(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    params: &Node<'_>,
    source: &[u8],
    _func_id: EntityId,
    func_scope_id: EntityId,
    is_typescript: bool,
) {
    let mut cursor = params.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "identifier" => {
                    let name = child.text(source);
                    let var_id = emitter.alloc();
                    let name_val = emitter.string(name);
                    emitter.emit("variables", vec![
                        Value::Entity(var_id),
                        name_val,
                        Value::Entity(func_scope_id),
                    ]);
                    let var_loc = LocationEmitter::emit_for_node(emitter, file_id, &child);
                    emitter.emit("hasLocation", vec![
                        Value::Entity(var_id),
                        Value::Entity(var_loc),
                    ]);
                }
                "assignment_pattern" => {
                    // Default parameter: name = defaultValue
                    if let Some(left) = child.child_by_field("left") {
                        if left.kind() == "identifier" {
                            let name = left.text(source);
                            let var_id = emitter.alloc();
                            let name_val = emitter.string(name);
                            emitter.emit("variables", vec![
                                Value::Entity(var_id),
                                name_val,
                                Value::Entity(func_scope_id),
                            ]);
                            let var_loc = LocationEmitter::emit_for_node(emitter, file_id, &left);
                            emitter.emit("hasLocation", vec![
                                Value::Entity(var_id),
                                Value::Entity(var_loc),
                            ]);
                        }
                    }
                }
                "rest_pattern" => {
                    // ...rest parameter
                    let mut inner = child.walk();
                    if inner.goto_first_child() {
                        loop {
                            let ic = inner.node();
                            if ic.kind() == "identifier" {
                                let name = ic.text(source);
                                let var_id = emitter.alloc();
                                let name_val = emitter.string(name);
                                emitter.emit("variables", vec![
                                    Value::Entity(var_id),
                                    name_val,
                                    Value::Entity(func_scope_id),
                                ]);
                                let var_loc = LocationEmitter::emit_for_node(emitter, file_id, &ic);
                                emitter.emit("hasLocation", vec![
                                    Value::Entity(var_id),
                                    Value::Entity(var_loc),
                                ]);
                            }
                            if !inner.goto_next_sibling() { break; }
                        }
                    }
                }
                "array_pattern" | "object_pattern" => {
                    // Destructuring parameter — extract the pattern
                    // For now, just register identifiers found within
                    extract_pattern_vars(emitter, file_id, &child, source, func_scope_id);
                }
                // TypeScript: required_parameter, optional_parameter
                "required_parameter" | "optional_parameter" if is_typescript => {
                    if let Some(pattern) = child.child_by_field("pattern") {
                        if pattern.kind() == "identifier" {
                            let name = pattern.text(source);
                            let var_id = emitter.alloc();
                            let name_val = emitter.string(name);
                            emitter.emit("variables", vec![
                                Value::Entity(var_id),
                                name_val,
                                Value::Entity(func_scope_id),
                            ]);
                            let var_loc = LocationEmitter::emit_for_node(emitter, file_id, &pattern);
                            emitter.emit("hasLocation", vec![
                                Value::Entity(var_id),
                                Value::Entity(var_loc),
                            ]);
                        }
                    }
                    // Extract type annotation
                    if let Some(type_ann) = child.child_by_field("type") {
                        extract_type_annotation(emitter, file_id, &type_ann, source, _func_id);
                    }
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Recursively extract variable names from destructuring patterns.
fn extract_pattern_vars(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    scope_id: EntityId,
) {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "identifier" => {
                    let name = child.text(source);
                    let var_id = emitter.alloc();
                    let name_val = emitter.string(name);
                    emitter.emit("variables", vec![
                        Value::Entity(var_id),
                        name_val,
                        Value::Entity(scope_id),
                    ]);
                    let var_loc = LocationEmitter::emit_for_node(emitter, file_id, &child);
                    emitter.emit("hasLocation", vec![
                        Value::Entity(var_id),
                        Value::Entity(var_loc),
                    ]);
                }
                "array_pattern" | "object_pattern" | "pair_pattern" | "assignment_pattern"
                | "rest_pattern" | "shorthand_property_identifier_pattern" => {
                    extract_pattern_vars(emitter, file_id, &child, source, scope_id);
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract a class declaration.
fn extract_class_decl(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
    scope_id: EntityId,
    is_typescript: bool,
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();

    let class_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    let name_val = emitter.string(&name);
    emitter.emit("classes", vec![
        Value::Entity(class_id),
        name_val.clone(),
        Value::Entity(parent_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(class_id),
        Value::Entity(loc_id),
    ]);

    // Create class scope
    let class_scope_id = emitter.alloc();
    emitter.emit("scopes", vec![
        Value::Entity(class_scope_id),
        Value::Int(SCOPE_CLASS),
    ]);

    // Register the class name as a variable in the outer scope
    if !name.is_empty() {
        let var_id = emitter.alloc();
        emitter.emit("variables", vec![
            Value::Entity(var_id),
            name_val,
            Value::Entity(scope_id),
        ]);
    }

    // Extract decorators (TypeScript)
    if is_typescript {
        extract_decorators(emitter, file_id, node, source, class_id);
    }

    // Extract body
    if let Some(body) = node.child_by_field("body") {
        extract_class_body(emitter, file_id, &body, source, class_id, class_scope_id, is_typescript);
    }
}

/// Extract a class expression.
fn extract_class_expr(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
    scope_id: EntityId,
    is_typescript: bool,
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();

    let class_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    let name_val = emitter.string(&name);
    emitter.emit("classes", vec![
        Value::Entity(class_id),
        name_val,
        Value::Entity(parent_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(class_id),
        Value::Entity(loc_id),
    ]);

    let class_scope_id = emitter.alloc();
    emitter.emit("scopes", vec![
        Value::Entity(class_scope_id),
        Value::Int(SCOPE_CLASS),
    ]);

    if let Some(body) = node.child_by_field("body") {
        extract_class_body(emitter, file_id, &body, source, class_id, class_scope_id, is_typescript);
    }
}

/// Extract a class body (methods, fields, getters, setters).
fn extract_class_body(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    body: &Node<'_>,
    source: &[u8],
    class_id: EntityId,
    class_scope_id: EntityId,
    is_typescript: bool,
) {
    let mut idx = 0i64;
    let mut cursor = body.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "method_definition" => {
                    extract_method_definition(emitter, file_id, &child, source, class_id, class_scope_id, idx, is_typescript);
                    idx += 1;
                }
                "field_definition" | "public_field_definition" => {
                    extract_field_definition(emitter, file_id, &child, source, class_id, class_scope_id, idx, is_typescript);
                    idx += 1;
                }
                "comment" => {
                    extract_comment(emitter, file_id, &child, source, class_id);
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract a method definition (in class or object literal).
fn extract_method_definition(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
    scope_id: EntityId,
    index: i64,
    is_typescript: bool,
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();

    // Determine property kind (value/getter/setter)
    let text = node.text(source);
    let prop_kind = if text.starts_with("get ") || text.starts_with("static get ") {
        PROP_GETTER
    } else if text.starts_with("set ") || text.starts_with("static set ") {
        PROP_SETTER
    } else {
        PROP_VALUE
    };

    let prop_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    emitter.emit("properties", vec![
        Value::Entity(prop_id),
        Value::Entity(parent_id),
        Value::Int(index),
        Value::Int(prop_kind),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(prop_id),
        Value::Entity(loc_id),
    ]);

    // Create the function for this method
    let func_id = emitter.alloc();
    let func_loc = LocationEmitter::emit_for_node(emitter, file_id, node);
    let name_val = emitter.string(&name);
    emitter.emit("functions", vec![
        Value::Entity(func_id),
        name_val,
        Value::Entity(scope_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(func_id),
        Value::Entity(func_loc),
    ]);

    let func_scope_id = emitter.alloc();
    emitter.emit("scopes", vec![
        Value::Entity(func_scope_id),
        Value::Int(SCOPE_FUNCTION),
    ]);

    // Extract decorators (TypeScript)
    if is_typescript {
        extract_decorators(emitter, file_id, node, source, func_id);
    }

    // Extract parameters
    if let Some(params) = node.child_by_field("parameters") {
        extract_function_params(emitter, file_id, &params, source, func_id, func_scope_id, is_typescript);
    }

    // Extract return type annotation (TypeScript)
    if is_typescript {
        if let Some(ret_type) = node.child_by_field("return_type") {
            extract_type_annotation(emitter, file_id, &ret_type, source, func_id);
        }
    }

    // Extract body
    if let Some(body) = node.child_by_field("body") {
        extract_stmt(emitter, file_id, &body, source, func_id, func_scope_id, 0, is_typescript);
    }
}

/// Extract a field definition (class fields).
fn extract_field_definition(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
    scope_id: EntityId,
    index: i64,
    is_typescript: bool,
) {
    let prop_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    emitter.emit("properties", vec![
        Value::Entity(prop_id),
        Value::Entity(parent_id),
        Value::Int(index),
        Value::Int(PROP_VALUE),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(prop_id),
        Value::Entity(loc_id),
    ]);

    // Extract property name
    if let Some(name_node) = node.child_by_field("property") {
        extract_expr(emitter, file_id, &name_node, source, prop_id, scope_id, 0, is_typescript);
    }

    // Extract type annotation (TypeScript)
    if is_typescript {
        if let Some(type_ann) = node.child_by_field("type") {
            extract_type_annotation(emitter, file_id, &type_ann, source, prop_id);
        }
    }

    // Extract value
    if let Some(value) = node.child_by_field("value") {
        extract_expr(emitter, file_id, &value, source, prop_id, scope_id, 1, is_typescript);
    }
}

/// Extract an import statement.
fn extract_import(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let import_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    // Extract the import source path
    let path = node.child_by_field("source")
        .map(|s| {
            let text = s.text(source);
            // Remove quotes
            text.trim_matches('\'').trim_matches('"').to_string()
        })
        .unwrap_or_default();

    // Extract imported names
    let mut import_names = Vec::new();
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "import_clause" => {
                    // Collect names from import clause
                    let mut inner = child.walk();
                    if inner.goto_first_child() {
                        loop {
                            let ic = inner.node();
                            match ic.kind() {
                                "identifier" => {
                                    // default import
                                    import_names.push(ic.text(source).to_string());
                                }
                                "named_imports" => {
                                    let mut named = ic.walk();
                                    if named.goto_first_child() {
                                        loop {
                                            let nc = named.node();
                                            if nc.kind() == "import_specifier" {
                                                if let Some(name) = nc.child_by_field("name") {
                                                    import_names.push(name.text(source).to_string());
                                                }
                                            }
                                            if !named.goto_next_sibling() { break; }
                                        }
                                    }
                                }
                                "namespace_import" => {
                                    let text = ic.text(source);
                                    import_names.push(text.to_string());
                                }
                                _ => {}
                            }
                            if !inner.goto_next_sibling() { break; }
                        }
                    }
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }

    let name = if import_names.is_empty() {
        // Side-effect import: import 'module'
        path.clone()
    } else {
        import_names.join(", ")
    };

    let name_val = emitter.string(&name);
    let path_val = emitter.string(&path);
    emitter.emit("imports", vec![
        Value::Entity(import_id),
        Value::Entity(parent_id),
        name_val,
        path_val,
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(import_id),
        Value::Entity(loc_id),
    ]);
}

/// Extract an export statement.
fn extract_export(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    stmt_id: EntityId,
    scope_id: EntityId,
    is_typescript: bool,
) {
    let export_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    emitter.emit("exports", vec![
        Value::Entity(export_id),
        Value::Entity(stmt_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(export_id),
        Value::Entity(loc_id),
    ]);

    // Extract the exported declaration or expression
    let mut cursor = node.walk();
    let mut child_idx = 0i64;
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.is_named() {
                match child.kind() {
                    "function_declaration" | "generator_function_declaration" => {
                        extract_stmt(emitter, file_id, &child, source, stmt_id, scope_id, child_idx, is_typescript);
                        child_idx += 1;
                    }
                    "class_declaration" => {
                        extract_stmt(emitter, file_id, &child, source, stmt_id, scope_id, child_idx, is_typescript);
                        child_idx += 1;
                    }
                    "variable_declaration" | "lexical_declaration" => {
                        extract_stmt(emitter, file_id, &child, source, stmt_id, scope_id, child_idx, is_typescript);
                        child_idx += 1;
                    }
                    "interface_declaration" if is_typescript => {
                        extract_stmt(emitter, file_id, &child, source, stmt_id, scope_id, child_idx, is_typescript);
                        child_idx += 1;
                    }
                    "type_alias_declaration" if is_typescript => {
                        extract_stmt(emitter, file_id, &child, source, stmt_id, scope_id, child_idx, is_typescript);
                        child_idx += 1;
                    }
                    "enum_declaration" if is_typescript => {
                        extract_stmt(emitter, file_id, &child, source, stmt_id, scope_id, child_idx, is_typescript);
                        child_idx += 1;
                    }
                    _ => {
                        // Could be an expression (export default expr)
                        extract_expr(emitter, file_id, &child, source, stmt_id, scope_id, child_idx, is_typescript);
                        child_idx += 1;
                    }
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract a comment.
fn extract_comment(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let comment_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    let text = node.text(source);
    let kind = if text.starts_with("//") {
        COMMENT_LINE
    } else {
        COMMENT_BLOCK
    };
    let text_val = emitter.string(text);
    emitter.emit("comments", vec![
        Value::Entity(comment_id),
        Value::Int(kind),
        Value::Entity(parent_id),
        text_val,
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(comment_id),
        Value::Entity(loc_id),
    ]);
}

/// Extract a TypeScript type annotation.
fn extract_type_annotation(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let ann_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    let text = node.text(source);
    let text_val = emitter.string(text);
    emitter.emit("type_annotations", vec![
        Value::Entity(ann_id),
        Value::Entity(parent_id),
        text_val,
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(ann_id),
        Value::Entity(loc_id),
    ]);
}

/// Extract a TypeScript interface declaration.
fn extract_interface(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();

    let iface_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    let name_val = emitter.string(&name);
    emitter.emit("interfaces", vec![
        Value::Entity(iface_id),
        name_val,
        Value::Entity(parent_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(iface_id),
        Value::Entity(loc_id),
    ]);
}

/// Extract a TypeScript enum declaration.
fn extract_enum(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();

    let enum_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    let name_val = emitter.string(&name);
    emitter.emit("enums", vec![
        Value::Entity(enum_id),
        name_val,
        Value::Entity(parent_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(enum_id),
        Value::Entity(loc_id),
    ]);
}

/// Extract a TypeScript type alias declaration.
fn extract_type_alias(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();

    let alias_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    let name_val = emitter.string(&name);
    emitter.emit("type_aliases", vec![
        Value::Entity(alias_id),
        name_val,
        Value::Entity(parent_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(alias_id),
        Value::Entity(loc_id),
    ]);
}

/// Extract TypeScript decorators from a node.
fn extract_decorators(
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
            if child.kind() == "decorator" {
                let text = child.text(source);
                // Extract decorator name (strip @)
                let name = text.trim_start_matches('@')
                    .split('(')
                    .next()
                    .unwrap_or(text)
                    .to_string();
                let dec_id = emitter.alloc();
                let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                let name_val = emitter.string(&name);
                emitter.emit("decorators", vec![
                    Value::Entity(dec_id),
                    name_val,
                    Value::Entity(parent_id),
                ]);
                emitter.emit("hasLocation", vec![
                    Value::Entity(dec_id),
                    Value::Entity(loc_id),
                ]);
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Find an operator token in a node's children.
fn find_operator(node: &Node<'_>, source: &[u8]) -> String {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if !child.is_named() {
                let text = child.text(source);
                match text {
                    "+" | "-" | "*" | "/" | "%" | "**"
                    | "&" | "|" | "^" | "<<" | ">>" | ">>>"
                    | "==" | "!=" | "===" | "!==" | "<" | ">" | "<=" | ">="
                    | "&&" | "||" | "??"
                    | "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "**="
                    | "&=" | "|=" | "^=" | "<<=" | ">>=" | ">>>="
                    | "&&=" | "||=" | "??="
                    | "!" | "~" | "++" | "--"
                    | "typeof" | "void" | "delete"
                    | "in" | "instanceof" => {
                        return text.to_string();
                    }
                    _ => {}
                }
            }
            // Also check named children for typeof/void/delete/in/instanceof
            if child.is_named() {
                let text = child.text(source);
                if matches!(text, "typeof" | "void" | "delete" | "in" | "instanceof") {
                    // These are keywords that appear as named nodes sometimes
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
    // Fallback: try the operator field
    if let Some(op) = node.child_by_field("operator") {
        return op.text(source).to_string();
    }
    String::new()
}

fn js_binary_op_kind(op: &str) -> i64 {
    match op {
        "==" => EXPR_EQ,
        "!=" => EXPR_NEQ,
        "===" => EXPR_EQQ,
        "!==" => EXPR_NEQQ,
        "<" => EXPR_LT,
        "<=" => EXPR_LE,
        ">" => EXPR_GT,
        ">=" => EXPR_GE,
        "<<" => EXPR_LSHIFT,
        ">>" => EXPR_RSHIFT,
        ">>>" => EXPR_URSHIFT,
        "+" => EXPR_ADD,
        "-" => EXPR_SUB,
        "*" => EXPR_MUL,
        "/" => EXPR_DIV,
        "%" => EXPR_MOD,
        "**" => EXPR_EXP,
        "|" => EXPR_BITOR,
        "^" => EXPR_XOR,
        "&" => EXPR_BITAND,
        "in" => EXPR_IN,
        "instanceof" => EXPR_INSTANCEOF,
        "&&" => EXPR_LOGAND,
        "||" => EXPR_LOGOR,
        "??" => EXPR_NULLISH_COALESCING,
        _ => EXPR_ADD,
    }
}

fn js_assign_op_kind(op: &str) -> i64 {
    match op {
        "=" => EXPR_ASSIGN,
        "+=" => EXPR_ASSIGN_ADD,
        "-=" => EXPR_ASSIGN_SUB,
        "*=" => EXPR_ASSIGN_MUL,
        "/=" => EXPR_ASSIGN_DIV,
        "%=" => EXPR_ASSIGN_MOD,
        "**=" => EXPR_ASSIGN_EXP,
        "<<=" => EXPR_ASSIGN_LSHIFT,
        ">>=" => EXPR_ASSIGN_RSHIFT,
        ">>>=" => EXPR_ASSIGN_URSHIFT,
        "|=" => EXPR_ASSIGN_OR,
        "^=" => EXPR_ASSIGN_XOR,
        "&=" => EXPR_ASSIGN_AND,
        _ => EXPR_ASSIGN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocql_database::Database;
    use crate::schema::javascript_schema;

    fn extract_js_file(filename: &str) -> Database {
        let source = std::fs::read(
            format!("tests/fixtures/{}", filename)
        ).unwrap();
        let schema = javascript_schema();
        let mut db = Database::from_schema(schema);
        let extractor = JavaScriptExtractor::new();
        let result = extractor.extract_source(&mut db, filename, &source);
        assert!(result.success, "Extraction failed for {}: {:?}", filename, result.error);
        db
    }

    fn extract_ts_file(filename: &str) -> Database {
        let source = std::fs::read(
            format!("tests/fixtures/{}", filename)
        ).unwrap();
        let schema = javascript_schema();
        let mut db = Database::from_schema(schema);
        let extractor = TypeScriptExtractor::new();
        let result = extractor.extract_source(&mut db, filename, &source);
        assert!(result.success, "Extraction failed for {}: {:?}", filename, result.error);
        db
    }

    #[test]
    fn test_js_toplevels() {
        let db = extract_js_file("simple.js");
        let toplevels: Vec<_> = db.scan("toplevels").unwrap().collect();
        eprintln!("Toplevels: {} entries", toplevels.len());
        assert!(toplevels.len() >= 1, "Should have at least 1 toplevel");
    }

    #[test]
    fn test_js_functions() {
        let db = extract_js_file("simple.js");
        let functions: Vec<_> = db.scan("functions").unwrap().collect();
        let names: Vec<_> = functions.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Functions: {:?}", names);
        assert!(names.contains(&"add"), "Should find function 'add'");
        assert!(names.contains(&"greet"), "Should find function 'greet'");
    }

    #[test]
    fn test_js_classes() {
        let db = extract_js_file("simple.js");
        let classes: Vec<_> = db.scan("classes").unwrap().collect();
        let names: Vec<_> = classes.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Classes: {:?}", names);
        assert!(names.contains(&"Animal"), "Should find class 'Animal'");
        assert!(names.contains(&"Dog"), "Should find class 'Dog'");
    }

    #[test]
    fn test_js_statements() {
        let db = extract_js_file("simple.js");
        let stmts: Vec<_> = db.scan("stmts").unwrap().collect();
        let kinds: Vec<i64> = stmts.iter().map(|t| t[1].as_int().unwrap()).collect();
        eprintln!("Statement kinds: {:?}", kinds);
        assert!(kinds.contains(&STMT_IF), "Should have if statements");
        assert!(kinds.contains(&STMT_FOR), "Should have for statements");
        assert!(kinds.contains(&STMT_RETURN), "Should have return statements");
    }

    #[test]
    fn test_js_expressions() {
        let db = extract_js_file("simple.js");
        let exprs: Vec<_> = db.scan("exprs").unwrap().collect();
        let kinds: Vec<i64> = exprs.iter().map(|t| t[1].as_int().unwrap()).collect();
        eprintln!("Expression kinds: {:?}", kinds);
        assert!(kinds.contains(&EXPR_CALL), "Should have call expressions");
        assert!(kinds.contains(&EXPR_NEW), "Should have new expressions");
    }

    #[test]
    fn test_js_imports() {
        let db = extract_js_file("simple.js");
        let imports: Vec<_> = db.scan("imports").unwrap().collect();
        eprintln!("Imports: {} entries", imports.len());
        assert!(imports.len() >= 1, "Should have at least 1 import");
    }

    #[test]
    fn test_js_exports() {
        let db = extract_js_file("simple.js");
        let exports: Vec<_> = db.scan("exports").unwrap().collect();
        eprintln!("Exports: {} entries", exports.len());
        assert!(exports.len() >= 1, "Should have at least 1 export");
    }

    #[test]
    fn test_js_variables() {
        let db = extract_js_file("simple.js");
        let vars: Vec<_> = db.scan("variables").unwrap().collect();
        let names: Vec<_> = vars.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Variables: {:?}", names);
        assert!(names.len() >= 3, "Should have multiple variables");
    }

    #[test]
    fn test_js_comments() {
        let db = extract_js_file("simple.js");
        let comments: Vec<_> = db.scan("comments").unwrap().collect();
        eprintln!("Comments: {} entries", comments.len());
        assert!(comments.len() >= 1, "Should have at least 1 comment");
    }

    #[test]
    fn test_js_scopes() {
        let db = extract_js_file("simple.js");
        let scopes: Vec<_> = db.scan("scopes").unwrap().collect();
        eprintln!("Scopes: {} entries", scopes.len());
        assert!(scopes.len() >= 1, "Should have at least 1 scope");
    }

    #[test]
    fn test_js_literals() {
        let db = extract_js_file("simple.js");
        let lits: Vec<_> = db.scan("literals").unwrap().collect();
        eprintln!("Literals: {} entries", lits.len());
        assert!(lits.len() >= 1, "Should have at least 1 literal");
    }

    #[test]
    fn test_js_properties() {
        let db = extract_js_file("simple.js");
        let props: Vec<_> = db.scan("properties").unwrap().collect();
        eprintln!("Properties: {} entries", props.len());
        assert!(props.len() >= 1, "Should have at least 1 property");
    }

    #[test]
    fn test_ts_interfaces() {
        let db = extract_ts_file("simple.ts");
        let ifaces: Vec<_> = db.scan("interfaces").unwrap().collect();
        let names: Vec<_> = ifaces.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Interfaces: {:?}", names);
        assert!(names.contains(&"Shape"), "Should find interface 'Shape'");
    }

    #[test]
    fn test_ts_enums() {
        let db = extract_ts_file("simple.ts");
        let enums: Vec<_> = db.scan("enums").unwrap().collect();
        let names: Vec<_> = enums.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Enums: {:?}", names);
        assert!(names.contains(&"Color"), "Should find enum 'Color'");
    }

    #[test]
    fn test_ts_type_aliases() {
        let db = extract_ts_file("simple.ts");
        let aliases: Vec<_> = db.scan("type_aliases").unwrap().collect();
        let names: Vec<_> = aliases.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Type aliases: {:?}", names);
        assert!(names.contains(&"StringOrNumber"), "Should find type alias 'StringOrNumber'");
    }

    #[test]
    fn test_ts_functions() {
        let db = extract_ts_file("simple.ts");
        let functions: Vec<_> = db.scan("functions").unwrap().collect();
        let names: Vec<_> = functions.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("TS Functions: {:?}", names);
        assert!(names.contains(&"identity"), "Should find function 'identity'");
    }

    #[test]
    fn test_ts_classes() {
        let db = extract_ts_file("simple.ts");
        let classes: Vec<_> = db.scan("classes").unwrap().collect();
        let names: Vec<_> = classes.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("TS Classes: {:?}", names);
        assert!(names.contains(&"Circle"), "Should find class 'Circle'");
    }
}
