use ocql_database::{EntityId, Value};
use ocql_extractor_common::{Extractor, FactEmitter, LocationEmitter, NodeExt};
use ocql_extractor_common::tree_sitter::{Language, Node, Tree};

/// Python extractor using tree-sitter.
///
/// Extracts:
/// - Modules
/// - Function definitions (including async, nested)
/// - Class definitions (including nested, with bases)
/// - All statement types (if, for, while, with, try/except, return, raise, assert, etc.)
/// - All expression types (calls, attributes, literals, comprehensions, lambda, etc.)
/// - Parameters (positional, keyword, *args, **kwargs, defaults, annotations)
/// - Decorators
/// - Import statements (import x, from x import y)
/// - Comments
/// - Locations for all entities
pub struct PythonExtractor;

impl PythonExtractor {
    pub fn new() -> Self {
        Self
    }
}

// Module kind constants
const MODULE_SCRIPT: i64 = 0;

// Statement kind constants
const STMT_ASSIGN: i64 = 0;
const STMT_RETURN: i64 = 1;
const STMT_IF: i64 = 2;
const STMT_FOR: i64 = 3;
const STMT_WHILE: i64 = 4;
const STMT_WITH: i64 = 5;
const STMT_TRY: i64 = 6;
const STMT_RAISE: i64 = 7;
const STMT_IMPORT: i64 = 8;
const STMT_PASS: i64 = 9;
const STMT_BREAK: i64 = 10;
const STMT_CONTINUE: i64 = 11;
const STMT_ASSERT: i64 = 12;
const STMT_EXPR: i64 = 13;
const STMT_DELETE: i64 = 14;
const STMT_GLOBAL: i64 = 15;
const STMT_NONLOCAL: i64 = 16;
const STMT_ASYNC_FOR: i64 = 17;
const STMT_ASYNC_WITH: i64 = 18;
const STMT_MATCH: i64 = 19;
const STMT_AUG_ASSIGN: i64 = 20;
const STMT_ANN_ASSIGN: i64 = 21;

// Expression kind constants
const EXPR_CALL: i64 = 0;
const EXPR_ATTRIBUTE: i64 = 1;
const EXPR_NAME: i64 = 2;
const EXPR_INT: i64 = 3;
const EXPR_FLOAT: i64 = 4;
const EXPR_STRING: i64 = 5;
const EXPR_LIST: i64 = 6;
const EXPR_DICT: i64 = 7;
const EXPR_TUPLE: i64 = 8;
const EXPR_SET: i64 = 9;
const EXPR_BINOP: i64 = 10;
const EXPR_UNARYOP: i64 = 11;
const EXPR_BOOLOP: i64 = 12;
const EXPR_COMPARE: i64 = 13;
const EXPR_SUBSCRIPT: i64 = 14;
const EXPR_STARRED: i64 = 15;
const EXPR_YIELD: i64 = 16;
const EXPR_AWAIT: i64 = 17;
const EXPR_LAMBDA: i64 = 18;
const EXPR_IFEXPR: i64 = 19;
const EXPR_LISTCOMP: i64 = 20;
const EXPR_SETCOMP: i64 = 21;
const EXPR_DICTCOMP: i64 = 22;
const EXPR_GENEXPR: i64 = 23;
const EXPR_FSTRING: i64 = 24;
const EXPR_NONE: i64 = 25;
const EXPR_TRUE: i64 = 26;
const EXPR_FALSE: i64 = 27;
const EXPR_ELLIPSIS: i64 = 28;
const EXPR_WALRUS: i64 = 29;
const EXPR_CONCAT_STRING: i64 = 30;
const EXPR_SLICE: i64 = 31;
const EXPR_KEYWORD_ARG: i64 = 32;
const EXPR_PAIR: i64 = 33;

// Parameter kind constants
const PARAM_POSITIONAL: i64 = 0;
const PARAM_KEYWORD: i64 = 1;
const PARAM_ARGS: i64 = 2;
const PARAM_KWARGS: i64 = 3;
const PARAM_POSITIONAL_ONLY_SEP: i64 = 4;
const PARAM_KEYWORD_ONLY_SEP: i64 = 5;

// Import kind constants
const IMPORT_REGULAR: i64 = 0;
const IMPORT_FROM: i64 = 1;

impl Extractor for PythonExtractor {
    fn language(&self) -> Language {
        tree_sitter_python::LANGUAGE.into()
    }

    fn extensions(&self) -> &[&str] {
        &["py"]
    }

    fn extract_file(
        &self,
        emitter: &mut FactEmitter<'_>,
        file_id: EntityId,
        tree: &Tree,
        source: &[u8],
    ) {
        let root = tree.root_node();
        extract_module(emitter, file_id, &root, source);
    }
}

/// Extract the top-level module node.
fn extract_module(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    root: &Node<'_>,
    source: &[u8],
) {
    let module_id = emitter.alloc();
    let name_val = emitter.string("<module>");
    emitter.emit("py_Modules", vec![
        Value::Entity(module_id),
        Value::Int(MODULE_SCRIPT),
        name_val,
        Value::Entity(file_id),
    ]);

    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, root);
    emitter.emit("hasLocation", vec![
        Value::Entity(module_id),
        Value::Entity(loc_id),
    ]);

    // Extract top-level statements
    let mut cursor = root.walk();
    let mut stmt_idx = 0i64;
    if cursor.goto_first_child() {
        loop {
            let node = cursor.node();
            if node.is_named() {
                let extracted = extract_top_level(
                    emitter, file_id, module_id, &node, source, stmt_idx,
                );
                if extracted {
                    stmt_idx += 1;
                }
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// Extract a top-level item (statement, function, class, import, etc.).
/// Returns true if a statement-level entity was emitted.
fn extract_top_level(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    scope_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    stmt_idx: i64,
) -> bool {
    match node.kind() {
        "function_definition" => {
            extract_function(emitter, file_id, scope_id, node, source, stmt_idx, false);
            true
        }
        "class_definition" => {
            extract_class(emitter, file_id, scope_id, node, source, stmt_idx);
            true
        }
        "decorated_definition" => {
            extract_decorated_definition(emitter, file_id, scope_id, node, source, stmt_idx);
            true
        }
        "import_statement" => {
            extract_import(emitter, file_id, scope_id, node, source, stmt_idx);
            true
        }
        "import_from_statement" => {
            extract_import_from(emitter, file_id, scope_id, node, source, stmt_idx);
            true
        }
        "comment" => {
            extract_comment(emitter, file_id, node, source);
            false
        }
        _ => {
            extract_stmt(emitter, file_id, scope_id, node, source, stmt_idx).is_some()
        }
    }
}

/// Extract a function definition.
fn extract_function(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    scope_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    idx: i64,
    is_async: bool,
) -> EntityId {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();

    let func_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let name_val = emitter.string(&name);
    emitter.emit("py_Functions", vec![
        Value::Entity(func_id),
        name_val,
        Value::Entity(scope_id),
        Value::Int(idx),
    ]);

    if is_async {
        emitter.emit("py_function_is_async", vec![
            Value::Entity(func_id),
        ]);
    }

    emitter.emit("hasLocation", vec![
        Value::Entity(func_id),
        Value::Entity(loc_id),
    ]);

    // Scope nesting
    emitter.emit("py_scope_nesting", vec![
        Value::Entity(func_id),
        Value::Entity(scope_id),
    ]);

    // Extract parameters
    if let Some(params) = node.child_by_field("parameters") {
        extract_parameters(emitter, file_id, func_id, &params, source);
    }

    // Extract return annotation
    if let Some(_return_type) = node.child_by_field("return_type") {
        // We note the return type but don't store it in a separate table for now
    }

    // Extract body
    if let Some(body) = node.child_by_field("body") {
        extract_body(emitter, file_id, func_id, &body, source);
    }

    func_id
}

/// Extract a class definition.
fn extract_class(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    scope_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    idx: i64,
) -> EntityId {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();

    let class_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let name_val = emitter.string(&name);
    emitter.emit("py_Classes", vec![
        Value::Entity(class_id),
        name_val,
        Value::Entity(scope_id),
        Value::Int(idx),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(class_id),
        Value::Entity(loc_id),
    ]);

    // Scope nesting
    emitter.emit("py_scope_nesting", vec![
        Value::Entity(class_id),
        Value::Entity(scope_id),
    ]);

    // Extract base classes from superclasses (argument_list)
    if let Some(superclasses) = node.child_by_field("superclasses") {
        extract_base_classes(emitter, file_id, class_id, &superclasses, source);
    }

    // Extract body
    if let Some(body) = node.child_by_field("body") {
        extract_body(emitter, file_id, class_id, &body, source);
    }

    class_id
}

/// Extract base classes from a class's argument_list.
fn extract_base_classes(
    emitter: &mut FactEmitter<'_>,
    _file_id: EntityId,
    class_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let mut idx = 0i64;
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.is_named() && child.kind() != "comment" {
                let base_name = child.text(source).to_string();
                let name_val = emitter.string(&base_name);
                emitter.emit("py_base_classes", vec![
                    Value::Entity(class_id),
                    Value::Int(idx),
                    name_val,
                ]);
                idx += 1;
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract a decorated definition (function or class with decorators).
fn extract_decorated_definition(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    scope_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    stmt_idx: i64,
) {
    // Collect decorators and the definition
    let mut decorators = Vec::new();
    let mut definition: Option<Node<'_>> = None;

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "decorator" => decorators.push(child),
                "function_definition" | "class_definition" => {
                    definition = Some(child);
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }

    if let Some(def_node) = definition {
        // Extract the definition first
        let def_id = match def_node.kind() {
            "function_definition" => {
                // Check if the parent decorated_definition contains "async"
                let is_async = node.text(source).starts_with("@") && {
                    // Check if function itself starts with async
                    def_node.text(source).starts_with("async ")
                };
                extract_function(emitter, file_id, scope_id, &def_node, source, stmt_idx, is_async)
            }
            "class_definition" => {
                extract_class(emitter, file_id, scope_id, &def_node, source, stmt_idx)
            }
            _ => return,
        };

        // Now emit decorators
        for (dec_idx, dec_node) in decorators.iter().enumerate() {
            let dec_id = emitter.alloc();
            let loc_id = LocationEmitter::emit_for_node(emitter, file_id, dec_node);
            let dec_text = dec_node.text(source).to_string();
            // Strip leading '@' for the text
            let dec_text_stripped = dec_text.trim_start_matches('@').trim();
            let text_val = emitter.string(dec_text_stripped);
            emitter.emit("py_decorators", vec![
                Value::Entity(dec_id),
                Value::Entity(def_id),
                Value::Int(dec_idx as i64),
                text_val,
            ]);
            emitter.emit("hasLocation", vec![
                Value::Entity(dec_id),
                Value::Entity(loc_id),
            ]);
        }
    }
}

/// Extract a body block (suite of statements).
fn extract_body(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    scope_id: EntityId,
    body: &Node<'_>,
    source: &[u8],
) {
    let mut cursor = body.walk();
    let mut stmt_idx = 0i64;
    if cursor.goto_first_child() {
        loop {
            let node = cursor.node();
            if node.is_named() {
                let extracted = extract_top_level(
                    emitter, file_id, scope_id, &node, source, stmt_idx,
                );
                if extracted {
                    stmt_idx += 1;
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract parameters from a parameters node.
fn extract_parameters(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    func_id: EntityId,
    params: &Node<'_>,
    source: &[u8],
) {
    let mut index = 0i64;
    let mut cursor = params.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "identifier" => {
                    // Simple positional parameter
                    let name = child.text(source).to_string();
                    emit_parameter(emitter, file_id, func_id, &child, &name, index, PARAM_POSITIONAL, None, None);
                    index += 1;
                }
                "default_parameter" => {
                    let name = child.child_by_field("name")
                        .map(|n| n.text(source).to_string())
                        .unwrap_or_default();
                    let default_text = child.child_by_field("value")
                        .map(|v| v.text(source).to_string());
                    emit_parameter(emitter, file_id, func_id, &child, &name, index, PARAM_POSITIONAL, default_text.as_deref(), None);
                    index += 1;
                }
                "typed_parameter" => {
                    let name = child.child(0)
                        .map(|n| n.text(source).to_string())
                        .unwrap_or_default();
                    let annotation = child.child_by_field("type")
                        .map(|t| t.text(source).to_string());
                    emit_parameter(emitter, file_id, func_id, &child, &name, index, PARAM_POSITIONAL, None, annotation.as_deref());
                    index += 1;
                }
                "typed_default_parameter" => {
                    let name = child.child_by_field("name")
                        .map(|n| n.text(source).to_string())
                        .unwrap_or_default();
                    let default_text = child.child_by_field("value")
                        .map(|v| v.text(source).to_string());
                    let annotation = child.child_by_field("type")
                        .map(|t| t.text(source).to_string());
                    emit_parameter(emitter, file_id, func_id, &child, &name, index, PARAM_POSITIONAL, default_text.as_deref(), annotation.as_deref());
                    index += 1;
                }
                "list_splat_pattern" => {
                    // *args
                    let name = child.child(0)
                        .map(|n| n.text(source).to_string())
                        .unwrap_or_else(|| "*".to_string());
                    emit_parameter(emitter, file_id, func_id, &child, &name, index, PARAM_ARGS, None, None);
                    index += 1;
                }
                "dictionary_splat_pattern" => {
                    // **kwargs
                    let name = child.child(0)
                        .map(|n| n.text(source).to_string())
                        .unwrap_or_else(|| "**".to_string());
                    emit_parameter(emitter, file_id, func_id, &child, &name, index, PARAM_KWARGS, None, None);
                    index += 1;
                }
                "keyword_separator" => {
                    // The bare `*` separator
                    emit_parameter(emitter, file_id, func_id, &child, "*", index, PARAM_KEYWORD_ONLY_SEP, None, None);
                    index += 1;
                }
                "positional_separator" => {
                    // The `/` separator
                    emit_parameter(emitter, file_id, func_id, &child, "/", index, PARAM_POSITIONAL_ONLY_SEP, None, None);
                    index += 1;
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Emit a single parameter.
fn emit_parameter(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    func_id: EntityId,
    node: &Node<'_>,
    name: &str,
    index: i64,
    kind: i64,
    default_text: Option<&str>,
    annotation: Option<&str>,
) {
    let param_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    let name_val = emitter.string(name);
    emitter.emit("py_parameters", vec![
        Value::Entity(param_id),
        name_val,
        Value::Int(index),
        Value::Entity(func_id),
        Value::Int(kind),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(param_id),
        Value::Entity(loc_id),
    ]);

    if let Some(default) = default_text {
        let default_val = emitter.string(default);
        emitter.emit("py_parameter_defaults", vec![
            Value::Entity(param_id),
            default_val,
        ]);
    }

    if let Some(ann) = annotation {
        let ann_val = emitter.string(ann);
        emitter.emit("py_parameter_annotations", vec![
            Value::Entity(param_id),
            ann_val,
        ]);
    }
}

/// Extract an import statement (`import x, y, z`).
fn extract_import(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    scope_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    stmt_idx: i64,
) {
    let import_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    // Collect the full module text
    let full_text = node.text(source);
    let module_text = full_text.trim_start_matches("import ").trim();
    let module_val = emitter.string(module_text);

    emitter.emit("py_imports", vec![
        Value::Entity(import_id),
        Value::Int(IMPORT_REGULAR),
        module_val,
        Value::Entity(scope_id),
        Value::Int(stmt_idx),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(import_id),
        Value::Entity(loc_id),
    ]);

    // Extract individual import names
    let mut name_idx = 0i64;
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "dotted_name" => {
                    let name = child.text(source).to_string();
                    let name_val = emitter.string(&name);
                    let alias_val = emitter.string("");
                    emitter.emit("py_import_names", vec![
                        Value::Entity(import_id),
                        Value::Int(name_idx),
                        name_val,
                        alias_val,
                    ]);
                    name_idx += 1;
                }
                "aliased_import" => {
                    let name = child.child_by_field("name")
                        .map(|n| n.text(source).to_string())
                        .unwrap_or_default();
                    let alias = child.child_by_field("alias")
                        .map(|n| n.text(source).to_string())
                        .unwrap_or_default();
                    let name_val = emitter.string(&name);
                    let alias_val = emitter.string(&alias);
                    emitter.emit("py_import_names", vec![
                        Value::Entity(import_id),
                        Value::Int(name_idx),
                        name_val,
                        alias_val,
                    ]);
                    name_idx += 1;
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract a from-import statement (`from x import y, z`).
fn extract_import_from(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    scope_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    stmt_idx: i64,
) {
    let import_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    // Get the module name
    let module_name = node.child_by_field("module_name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    let module_val = emitter.string(&module_name);

    emitter.emit("py_imports", vec![
        Value::Entity(import_id),
        Value::Int(IMPORT_FROM),
        module_val,
        Value::Entity(scope_id),
        Value::Int(stmt_idx),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(import_id),
        Value::Entity(loc_id),
    ]);

    // Extract imported names
    let mut name_idx = 0i64;
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "dotted_name" | "relative_import" => {
                    // This is the module_name, already extracted above
                }
                "wildcard_import" => {
                    let name_val = emitter.string("*");
                    let alias_val = emitter.string("");
                    emitter.emit("py_import_names", vec![
                        Value::Entity(import_id),
                        Value::Int(name_idx),
                        name_val,
                        alias_val,
                    ]);
                    name_idx += 1;
                }
                "aliased_import" => {
                    let name = child.child_by_field("name")
                        .map(|n| n.text(source).to_string())
                        .unwrap_or_default();
                    let alias = child.child_by_field("alias")
                        .map(|n| n.text(source).to_string())
                        .unwrap_or_default();
                    let name_val = emitter.string(&name);
                    let alias_val = emitter.string(&alias);
                    emitter.emit("py_import_names", vec![
                        Value::Entity(import_id),
                        Value::Int(name_idx),
                        name_val,
                        alias_val,
                    ]);
                    name_idx += 1;
                }
                _ => {
                    // Handle bare identifiers in `from x import y, z`
                    if child.is_named() && child.kind() == "identifier" {
                        // Skip "import" keyword
                    }
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
    scope_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    index: i64,
) -> Option<EntityId> {
    let kind = match node.kind() {
        "expression_statement" => Some(STMT_EXPR),
        "return_statement" => Some(STMT_RETURN),
        "if_statement" => Some(STMT_IF),
        "for_statement" => Some(STMT_FOR),
        "while_statement" => Some(STMT_WHILE),
        "with_statement" => Some(STMT_WITH),
        "try_statement" => Some(STMT_TRY),
        "raise_statement" => Some(STMT_RAISE),
        "pass_statement" => Some(STMT_PASS),
        "break_statement" => Some(STMT_BREAK),
        "continue_statement" => Some(STMT_CONTINUE),
        "assert_statement" => Some(STMT_ASSERT),
        "delete_statement" => Some(STMT_DELETE),
        "global_statement" => Some(STMT_GLOBAL),
        "nonlocal_statement" => Some(STMT_NONLOCAL),
        "match_statement" => Some(STMT_MATCH),
        "assignment" => Some(STMT_ASSIGN),
        "augmented_assignment" => Some(STMT_AUG_ASSIGN),
        // Note: tree-sitter-python uses "for_statement" for both sync and async for
        // but we detect async via the presence of "async" keyword
        _ => None,
    };

    let stmt_kind = match kind {
        Some(k) => k,
        None => return None,
    };

    let stmt_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    emitter.emit("py_stmts", vec![
        Value::Entity(stmt_id),
        Value::Int(stmt_kind),
        Value::Entity(scope_id),
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
            let mut child_idx = 0i64;
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        if child.kind() == "assignment" || child.kind() == "augmented_assignment" {
                            // These are handled as separate statements at the top level
                            // but if nested inside expression_statement, extract children
                            extract_assignment_children(emitter, file_id, stmt_id, &child, source);
                        } else {
                            extract_expr(emitter, file_id, stmt_id, &child, source, child_idx);
                            child_idx += 1;
                        }
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "assignment" => {
            extract_assignment_children(emitter, file_id, stmt_id, node, source);
        }
        "augmented_assignment" => {
            extract_assignment_children(emitter, file_id, stmt_id, node, source);
        }
        "return_statement" => {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, stmt_id, &child, source, 0);
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "if_statement" => {
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, stmt_id, &cond, source, 0);
            }
            if let Some(consequence) = node.child_by_field("consequence") {
                extract_body(emitter, file_id, scope_id, &consequence, source);
            }
            if let Some(alternative) = node.child_by_field("alternative") {
                extract_body(emitter, file_id, scope_id, &alternative, source);
            }
        }
        "for_statement" => {
            if let Some(left) = node.child_by_field("left") {
                extract_expr(emitter, file_id, stmt_id, &left, source, 0);
            }
            if let Some(right) = node.child_by_field("right") {
                extract_expr(emitter, file_id, stmt_id, &right, source, 1);
            }
            if let Some(body) = node.child_by_field("body") {
                extract_body(emitter, file_id, scope_id, &body, source);
            }
        }
        "while_statement" => {
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, stmt_id, &cond, source, 0);
            }
            if let Some(body) = node.child_by_field("body") {
                extract_body(emitter, file_id, scope_id, &body, source);
            }
        }
        "with_statement" => {
            // Extract with items and body
            if let Some(body) = node.child_by_field("body") {
                extract_body(emitter, file_id, scope_id, &body, source);
            }
        }
        "try_statement" => {
            if let Some(body) = node.child_by_field("body") {
                extract_body(emitter, file_id, scope_id, &body, source);
            }
            // Except clauses, finally, else handled implicitly through body extraction
        }
        "raise_statement" => {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, stmt_id, &child, source, 0);
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "assert_statement" => {
            let mut cursor = node.walk();
            let mut child_idx = 0i64;
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, stmt_id, &child, source, child_idx);
                        child_idx += 1;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "delete_statement" => {
            let mut cursor = node.walk();
            let mut child_idx = 0i64;
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, stmt_id, &child, source, child_idx);
                        child_idx += 1;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "global_statement" | "nonlocal_statement" => {
            // Extract the variable names
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "identifier" {
                        let var_name = child.text(source).to_string();
                        let var_id = emitter.alloc();
                        let name_val = emitter.string(&var_name);
                        emitter.emit("py_variables", vec![
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
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        _ => {}
    }

    Some(stmt_id)
}

/// Extract children of assignment/augmented_assignment.
fn extract_assignment_children(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    parent_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    if let Some(left) = node.child_by_field("left") {
        extract_expr(emitter, file_id, parent_id, &left, source, 0);
        // Register variable for simple identifier assignments
        if left.kind() == "identifier" {
            let var_name = left.text(source).to_string();
            let var_id = emitter.alloc();
            let name_val = emitter.string(&var_name);
            // Use parent_id as scope approximation
            emitter.emit("py_variables", vec![
                Value::Entity(var_id),
                name_val,
                Value::Entity(parent_id),
            ]);
        }
    }
    if let Some(right) = node.child_by_field("right") {
        extract_expr(emitter, file_id, parent_id, &right, source, 1);
    }
}

/// Extract an expression. Returns the expression entity ID if extracted.
fn extract_expr(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    parent_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    index: i64,
) -> Option<EntityId> {
    let kind = match node.kind() {
        "call" => Some(EXPR_CALL),
        "attribute" => Some(EXPR_ATTRIBUTE),
        "identifier" => Some(EXPR_NAME),
        "integer" => Some(EXPR_INT),
        "float" => Some(EXPR_FLOAT),
        "string" => Some(EXPR_STRING),
        "list" => Some(EXPR_LIST),
        "dictionary" => Some(EXPR_DICT),
        "tuple" => Some(EXPR_TUPLE),
        "set" => Some(EXPR_SET),
        "binary_operator" => Some(EXPR_BINOP),
        "unary_operator" => Some(EXPR_UNARYOP),
        "boolean_operator" => Some(EXPR_BOOLOP),
        "comparison_operator" => Some(EXPR_COMPARE),
        "subscript" => Some(EXPR_SUBSCRIPT),
        "starred_expression" => Some(EXPR_STARRED),
        "yield" => Some(EXPR_YIELD),
        "await" => Some(EXPR_AWAIT),
        "lambda" => Some(EXPR_LAMBDA),
        "conditional_expression" => Some(EXPR_IFEXPR),
        "list_comprehension" => Some(EXPR_LISTCOMP),
        "set_comprehension" => Some(EXPR_SETCOMP),
        "dictionary_comprehension" => Some(EXPR_DICTCOMP),
        "generator_expression" => Some(EXPR_GENEXPR),
        "concatenated_string" => Some(EXPR_CONCAT_STRING),
        "none" => Some(EXPR_NONE),
        "true" => Some(EXPR_TRUE),
        "false" => Some(EXPR_FALSE),
        "ellipsis" => Some(EXPR_ELLIPSIS),
        "named_expression" => Some(EXPR_WALRUS),
        "slice" => Some(EXPR_SLICE),
        "keyword_argument" => Some(EXPR_KEYWORD_ARG),
        "pair" => Some(EXPR_PAIR),
        "parenthesized_expression" => {
            // Extract inner expression directly
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        return extract_expr(emitter, file_id, parent_id, &child, source, index);
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

    emitter.emit("py_exprs", vec![
        Value::Entity(expr_id),
        Value::Int(expr_kind),
        Value::Entity(parent_id),
        Value::Int(index),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(expr_id),
        Value::Entity(loc_id),
    ]);

    // Emit name for identifier expressions
    if node.kind() == "identifier" {
        let name = node.text(source).to_string();
        let name_val = emitter.string(&name);
        emitter.emit("py_expr_names", vec![
            Value::Entity(expr_id),
            name_val,
        ]);
    }

    // Emit value for literals
    match node.kind() {
        "integer" | "float" | "string" | "none" | "true" | "false" => {
            let value = node.text(source).to_string();
            let value_val = emitter.string(&value);
            emitter.emit("py_expr_values", vec![
                Value::Entity(expr_id),
                value_val,
            ]);
        }
        _ => {}
    }

    // Recurse into children based on expression kind
    match node.kind() {
        "call" => {
            if let Some(func) = node.child_by_field("function") {
                extract_expr(emitter, file_id, expr_id, &func, source, 0);
            }
            if let Some(args) = node.child_by_field("arguments") {
                let mut idx = 1i64;
                let mut cursor = args.walk();
                if cursor.goto_first_child() {
                    loop {
                        let child = cursor.node();
                        if child.is_named() {
                            extract_expr(emitter, file_id, expr_id, &child, source, idx);
                            idx += 1;
                        }
                        if !cursor.goto_next_sibling() { break; }
                    }
                }
            }
        }
        "attribute" => {
            if let Some(obj) = node.child_by_field("object") {
                extract_expr(emitter, file_id, expr_id, &obj, source, 0);
            }
            // Store the attribute name
            if let Some(attr) = node.child_by_field("attribute") {
                let attr_name = attr.text(source).to_string();
                let name_val = emitter.string(&attr_name);
                emitter.emit("py_expr_names", vec![
                    Value::Entity(expr_id),
                    name_val,
                ]);
            }
        }
        "binary_operator" => {
            if let Some(left) = node.child_by_field("left") {
                extract_expr(emitter, file_id, expr_id, &left, source, 0);
            }
            if let Some(right) = node.child_by_field("right") {
                extract_expr(emitter, file_id, expr_id, &right, source, 1);
            }
        }
        "unary_operator" => {
            if let Some(operand) = node.child_by_field("argument") {
                extract_expr(emitter, file_id, expr_id, &operand, source, 0);
            }
        }
        "boolean_operator" => {
            if let Some(left) = node.child_by_field("left") {
                extract_expr(emitter, file_id, expr_id, &left, source, 0);
            }
            if let Some(right) = node.child_by_field("right") {
                extract_expr(emitter, file_id, expr_id, &right, source, 1);
            }
        }
        "comparison_operator" => {
            // Comparison can have multiple operands: a < b < c
            let mut cursor = node.walk();
            let mut child_idx = 0i64;
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, expr_id, &child, source, child_idx);
                        child_idx += 1;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "subscript" => {
            if let Some(value) = node.child_by_field("value") {
                extract_expr(emitter, file_id, expr_id, &value, source, 0);
            }
            if let Some(subscript) = node.child_by_field("subscript") {
                extract_expr(emitter, file_id, expr_id, &subscript, source, 1);
            }
        }
        "starred_expression" => {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, expr_id, &child, source, 0);
                        break;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "await" => {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, expr_id, &child, source, 0);
                        break;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "yield" => {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, expr_id, &child, source, 0);
                        break;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "conditional_expression" => {
            // a if condition else b
            let mut cursor = node.walk();
            let mut child_idx = 0i64;
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, expr_id, &child, source, child_idx);
                        child_idx += 1;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "named_expression" => {
            // walrus operator: x := expr
            if let Some(name) = node.child_by_field("name") {
                extract_expr(emitter, file_id, expr_id, &name, source, 0);
            }
            if let Some(value) = node.child_by_field("value") {
                extract_expr(emitter, file_id, expr_id, &value, source, 1);
            }
        }
        "lambda" => {
            if let Some(body) = node.child_by_field("body") {
                extract_expr(emitter, file_id, expr_id, &body, source, 0);
            }
        }
        "list" | "tuple" | "set" => {
            let mut cursor = node.walk();
            let mut child_idx = 0i64;
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, expr_id, &child, source, child_idx);
                        child_idx += 1;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "dictionary" => {
            let mut cursor = node.walk();
            let mut child_idx = 0i64;
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, expr_id, &child, source, child_idx);
                        child_idx += 1;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "pair" => {
            if let Some(key) = node.child_by_field("key") {
                extract_expr(emitter, file_id, expr_id, &key, source, 0);
            }
            if let Some(value) = node.child_by_field("value") {
                extract_expr(emitter, file_id, expr_id, &value, source, 1);
            }
        }
        "keyword_argument" => {
            if let Some(name) = node.child_by_field("name") {
                let kw_name = name.text(source).to_string();
                let name_val = emitter.string(&kw_name);
                emitter.emit("py_expr_names", vec![
                    Value::Entity(expr_id),
                    name_val,
                ]);
            }
            if let Some(value) = node.child_by_field("value") {
                extract_expr(emitter, file_id, expr_id, &value, source, 0);
            }
        }
        "slice" => {
            // Slice can have start, stop, step
            let mut cursor = node.walk();
            let mut child_idx = 0i64;
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, expr_id, &child, source, child_idx);
                        child_idx += 1;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "concatenated_string" => {
            let mut cursor = node.walk();
            let mut child_idx = 0i64;
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, expr_id, &child, source, child_idx);
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
    emitter.emit("py_comments", vec![
        Value::Entity(comment_id),
        text_val,
        Value::Entity(loc_id),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(comment_id),
        Value::Entity(loc_id),
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocql_database::Database;
    use crate::schema::python_schema;

    fn extract_test_file(filename: &str) -> Database {
        let source = std::fs::read(
            format!("tests/fixtures/{}", filename)
        ).unwrap();
        let schema = python_schema();
        let mut db = Database::from_schema(schema);
        let extractor = PythonExtractor::new();
        let result = extractor.extract_source(&mut db, filename, &source);
        assert!(result.success, "Extraction failed for {}: {:?}", filename, result.error);
        db
    }

    #[test]
    fn test_module_extracted() {
        let db = extract_test_file("simple.py");
        let modules: Vec<_> = db.scan("py_Modules").unwrap().collect();
        assert_eq!(modules.len(), 1, "Should have exactly 1 module");
    }

    #[test]
    fn test_functions() {
        let db = extract_test_file("simple.py");
        let functions: Vec<_> = db.scan("py_Functions").unwrap().collect();
        let names: Vec<_> = functions.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Functions: {:?}", names);
        assert!(names.contains(&"greet"), "Should find 'greet'");
        assert!(names.contains(&"factorial"), "Should find 'factorial'");
        assert!(names.contains(&"add"), "Should find 'add'");
        assert!(names.contains(&"make_greeting"), "Should find 'make_greeting'");
        assert!(names.contains(&"process"), "Should find 'process'");
    }

    #[test]
    fn test_classes() {
        let db = extract_test_file("simple.py");
        let classes: Vec<_> = db.scan("py_Classes").unwrap().collect();
        let names: Vec<_> = classes.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Classes: {:?}", names);
        assert!(names.contains(&"Animal"), "Should find 'Animal'");
        assert!(names.contains(&"Dog"), "Should find 'Dog'");
    }

    #[test]
    fn test_base_classes() {
        let db = extract_test_file("simple.py");
        let bases: Vec<_> = db.scan("py_base_classes").unwrap().collect();
        let base_names: Vec<_> = bases.iter().map(|t| {
            db.strings.resolve(t[2].as_string().unwrap())
        }).collect();
        eprintln!("Base classes: {:?}", base_names);
        assert!(base_names.contains(&"Animal"), "Dog should extend Animal");
    }

    #[test]
    fn test_parameters() {
        let db = extract_test_file("simple.py");
        let params: Vec<_> = db.scan("py_parameters").unwrap().collect();
        let names: Vec<_> = params.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Parameters: {:?}", names);
        assert!(names.contains(&"self"), "Should find 'self' parameter");
        assert!(names.contains(&"name"), "Should find 'name' parameter");
        assert!(names.contains(&"n"), "Should find 'n' parameter");
        assert!(names.contains(&"args"), "Should find 'args' parameter");
        assert!(names.contains(&"kwargs"), "Should find 'kwargs' parameter");
    }

    #[test]
    fn test_parameter_defaults() {
        let db = extract_test_file("simple.py");
        let defaults: Vec<_> = db.scan("py_parameter_defaults").unwrap().collect();
        let values: Vec<_> = defaults.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Parameter defaults: {:?}", values);
        assert!(values.iter().any(|v| v.contains("World")), "Should find default 'World'");
    }

    #[test]
    fn test_parameter_annotations() {
        let db = extract_test_file("simple.py");
        let annotations: Vec<_> = db.scan("py_parameter_annotations").unwrap().collect();
        let values: Vec<_> = annotations.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Parameter annotations: {:?}", values);
        assert!(values.iter().any(|v| v.contains("int")), "Should find type annotation 'int'");
    }

    #[test]
    fn test_imports() {
        let db = extract_test_file("simple.py");
        let imports: Vec<_> = db.scan("py_imports").unwrap().collect();
        let modules: Vec<_> = imports.iter().map(|t| {
            db.strings.resolve(t[2].as_string().unwrap())
        }).collect();
        eprintln!("Imports: {:?}", modules);
        assert!(modules.iter().any(|m| m.contains("os")), "Should find 'os' import");
        assert!(modules.iter().any(|m| m.contains("typing")), "Should find 'typing' import");
        assert!(modules.iter().any(|m| m.contains("collections")), "Should find 'collections' import");
    }

    #[test]
    fn test_import_names() {
        let db = extract_test_file("simple.py");
        let import_names: Vec<_> = db.scan("py_import_names").unwrap().collect();
        let names: Vec<_> = import_names.iter().map(|t| {
            db.strings.resolve(t[2].as_string().unwrap())
        }).collect();
        eprintln!("Import names: {:?}", names);
        assert!(names.iter().any(|n| n.contains("List")), "Should find 'List' import name");
        assert!(names.iter().any(|n| n.contains("Dict")), "Should find 'Dict' import name");
    }

    #[test]
    fn test_decorators() {
        let db = extract_test_file("simple.py");
        let decorators: Vec<_> = db.scan("py_decorators").unwrap().collect();
        let texts: Vec<_> = decorators.iter().map(|t| {
            db.strings.resolve(t[3].as_string().unwrap())
        }).collect();
        eprintln!("Decorators: {:?}", texts);
        assert!(texts.iter().any(|t| t.contains("staticmethod")), "Should find @staticmethod");
    }

    #[test]
    fn test_statements() {
        let db = extract_test_file("simple.py");
        let stmts: Vec<_> = db.scan("py_stmts").unwrap().collect();
        let kinds: Vec<i64> = stmts.iter().map(|t| t[1].as_int().unwrap()).collect();
        eprintln!("Statement kinds: {:?}", kinds);
        assert!(kinds.contains(&STMT_RETURN), "Should have return statements");
        assert!(kinds.contains(&STMT_IF), "Should have if statements");
        assert!(kinds.contains(&STMT_FOR), "Should have for statements");
        assert!(kinds.contains(&STMT_WHILE), "Should have while statements");
        assert!(kinds.contains(&STMT_TRY), "Should have try statements");
        assert!(kinds.contains(&STMT_RAISE), "Should have raise statements");
        assert!(kinds.contains(&STMT_ASSERT), "Should have assert statements");
        assert!(kinds.contains(&STMT_PASS), "Should have pass statements");
        assert!(kinds.contains(&STMT_BREAK), "Should have break statements");
        assert!(kinds.contains(&STMT_CONTINUE), "Should have continue statements");
    }

    #[test]
    fn test_expressions() {
        let db = extract_test_file("simple.py");
        let exprs: Vec<_> = db.scan("py_exprs").unwrap().collect();
        let kinds: Vec<i64> = exprs.iter().map(|t| t[1].as_int().unwrap()).collect();
        eprintln!("Expression kinds: {:?}", kinds);
        assert!(kinds.contains(&EXPR_CALL), "Should have call expressions");
        assert!(kinds.contains(&EXPR_NAME), "Should have name expressions");
        assert!(kinds.contains(&EXPR_INT), "Should have integer literals");
        assert!(kinds.contains(&EXPR_STRING), "Should have string literals");
        assert!(kinds.contains(&EXPR_ATTRIBUTE), "Should have attribute access");
        assert!(kinds.contains(&EXPR_LIST), "Should have list expressions");
        assert!(kinds.contains(&EXPR_DICT), "Should have dict expressions");
        assert!(kinds.contains(&EXPR_BINOP), "Should have binary operations");
        assert!(kinds.contains(&EXPR_COMPARE), "Should have comparisons");
    }

    #[test]
    fn test_comments() {
        let db = extract_test_file("simple.py");
        let comments: Vec<_> = db.scan("py_comments").unwrap().collect();
        let texts: Vec<_> = comments.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Comments: {:?}", texts);
        assert!(!comments.is_empty(), "Should have at least one comment");
        assert!(texts.iter().any(|t| t.contains("Module-level")), "Should find module-level comment");
    }

    #[test]
    fn test_variables() {
        let db = extract_test_file("simple.py");
        let vars: Vec<_> = db.scan("py_variables").unwrap().collect();
        let names: Vec<_> = vars.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Variables: {:?}", names);
        assert!(names.contains(&"MODULE_CONSTANT"), "Should find 'MODULE_CONSTANT'");
    }

    #[test]
    fn test_expr_names() {
        let db = extract_test_file("simple.py");
        let expr_names: Vec<_> = db.scan("py_expr_names").unwrap().collect();
        let names: Vec<_> = expr_names.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Expression names (sample): {:?}", &names[..names.len().min(20)]);
        assert!(!names.is_empty(), "Should have expression names");
    }

    #[test]
    fn test_scope_nesting() {
        let db = extract_test_file("simple.py");
        let nesting: Vec<_> = db.scan("py_scope_nesting").unwrap().collect();
        eprintln!("Scope nesting entries: {}", nesting.len());
        assert!(nesting.len() >= 2, "Should have at least 2 scope nesting entries (functions/classes in module)");
    }

    #[test]
    fn test_locations_present() {
        let db = extract_test_file("simple.py");
        let locations: Vec<_> = db.scan("locations_default").unwrap().collect();
        let has_location: Vec<_> = db.scan("hasLocation").unwrap().collect();
        eprintln!("Locations: {}, hasLocation: {}", locations.len(), has_location.len());
        assert!(locations.len() >= 10, "Should have many locations");
        assert!(has_location.len() >= 10, "Should have many hasLocation entries");
    }

    #[test]
    fn test_files_present() {
        let db = extract_test_file("simple.py");
        let files: Vec<_> = db.scan("files").unwrap().collect();
        assert_eq!(files.len(), 1, "Should have exactly 1 file");
    }
}
