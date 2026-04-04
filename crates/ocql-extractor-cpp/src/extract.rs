use ocql_database::{EntityId, Value};
use ocql_extractor_common::{Extractor, FactEmitter, LocationEmitter, NodeExt};
use ocql_extractor_common::tree_sitter::{Language, Node, Tree};

/// C/C++ extractor using tree-sitter.
///
/// Phase 3a extracts:
/// - Files and locations
/// - Function declarations (name, return type, parameters)
/// - Struct/class/union/enum declarations (name, kind, fields, bases)
/// - Global/local variable declarations
/// - #include directives
pub struct CppExtractor {
    use_cpp: bool,
}

impl CppExtractor {
    /// Create an extractor for C++ files.
    pub fn cpp() -> Self {
        Self { use_cpp: true }
    }

    /// Create an extractor for C files.
    pub fn c() -> Self {
        Self { use_cpp: false }
    }
}

// Function kind constants (matches CodeQL convention)
const FUNCTION_NORMAL: i64 = 1;
#[allow(dead_code)]
const FUNCTION_CONSTRUCTOR: i64 = 2;
const FUNCTION_DESTRUCTOR: i64 = 3;

// Variable kind constants
const VAR_GLOBAL: i64 = 1;
const VAR_LOCAL: i64 = 2;

// User type kind constants (matches CodeQL convention)
const USERTYPE_STRUCT: i64 = 1;
const USERTYPE_CLASS: i64 = 2;
const USERTYPE_UNION: i64 = 3;
const USERTYPE_ENUM: i64 = 4;

// Statement kind constants (matches semmlecode.cpp.dbscheme)
const STMT_EXPR: i64 = 1;
const STMT_IF: i64 = 2;
const STMT_WHILE: i64 = 3;
const STMT_GOTO: i64 = 4;
const STMT_LABEL: i64 = 5;
const STMT_RETURN: i64 = 6;
const STMT_BLOCK: i64 = 7;
const STMT_DO_WHILE: i64 = 8;
const STMT_FOR: i64 = 9;
const STMT_SWITCH_CASE: i64 = 10;
const STMT_SWITCH: i64 = 11;
const STMT_DECL: i64 = 17;
const STMT_CONTINUE: i64 = 27;
const STMT_BREAK: i64 = 28;

// Expression kind constants (matches semmlecode.cpp.dbscheme)
const EXPR_ADDRESS_OF: i64 = 2;
const EXPR_INDIRECT: i64 = 4;
const EXPR_PAREXPR: i64 = 12;
const EXPR_LOGNOT: i64 = 13;
const EXPR_BITNOT: i64 = 14;
const EXPR_UNARYMINUS: i64 = 15;
const EXPR_UNARYPLUS: i64 = 16;
const EXPR_PREFIXINC: i64 = 20;
const EXPR_PREFIXDEC: i64 = 21;
const EXPR_POSTFIXINC: i64 = 22;
const EXPR_POSTFIXDEC: i64 = 23;
const EXPR_CONDITIONAL: i64 = 24;
const EXPR_ADD: i64 = 25;
const EXPR_SUB: i64 = 26;
const EXPR_MUL: i64 = 27;
const EXPR_DIV: i64 = 28;
const EXPR_REM: i64 = 29;
const EXPR_BITAND: i64 = 39;
const EXPR_BITOR: i64 = 40;
const EXPR_BITXOR: i64 = 41;
const EXPR_LSHIFT: i64 = 42;
const EXPR_RSHIFT: i64 = 43;
const EXPR_EQ: i64 = 44;
const EXPR_NE: i64 = 45;
const EXPR_LT: i64 = 46;
const EXPR_GT: i64 = 47;
const EXPR_LE: i64 = 48;
const EXPR_GE: i64 = 49;
const EXPR_ASSIGN: i64 = 52;
const EXPR_ASSIGNADD: i64 = 53;
const EXPR_ASSIGNSUB: i64 = 54;
const EXPR_ASSIGNMUL: i64 = 55;
const EXPR_ASSIGNDIV: i64 = 56;
const EXPR_ASSIGNREM: i64 = 57;
const EXPR_ASSIGNAND: i64 = 58;
const EXPR_ASSIGNOR: i64 = 59;
const EXPR_ASSIGNXOR: i64 = 60;
const EXPR_ASSIGNLSHIFT: i64 = 61;
const EXPR_ASSIGNRSHIFT: i64 = 62;
const EXPR_LOGAND: i64 = 65;
const EXPR_LOGOR: i64 = 66;
const EXPR_COMMA: i64 = 67;
const EXPR_SUBSCRIPT: i64 = 68;
const EXPR_CALL: i64 = 74;
const EXPR_VARACCESS: i64 = 84;
#[allow(dead_code)]
const EXPR_ROUTINEEXPR: i64 = 97;
const EXPR_SIZEOF: i64 = 93;
const EXPR_LITERAL: i64 = 140;
const EXPR_CAST: i64 = 141;
const EXPR_FIELD_ACCESS: i64 = 142;

impl Extractor for CppExtractor {
    fn language(&self) -> Language {
        if self.use_cpp {
            tree_sitter_cpp::LANGUAGE.into()
        } else {
            tree_sitter_c::LANGUAGE.into()
        }
    }

    fn extensions(&self) -> &[&str] {
        if self.use_cpp {
            &["cpp", "cxx", "cc", "C", "hpp", "hxx", "hh", "h"]
        } else {
            &["c", "h"]
        }
    }

    fn extract_file(
        &self,
        emitter: &mut FactEmitter<'_>,
        file_id: EntityId,
        tree: &Tree,
        source: &[u8],
    ) {
        let root = tree.root_node();
        extract_translation_unit(emitter, file_id, &root, source);
        resolve_call_bindings(emitter);
    }
}

/// Post-extraction pass: resolve call bindings (funbind table).
///
/// For each call expression (kind 74 / @callexpr), find the callee identifier
/// (child at index 0 in exprparents), look up its name from valuetext, and resolve
/// it to a function entity from the functions table. Emits funbind(call_id, func_id).
fn resolve_call_bindings(emitter: &mut FactEmitter<'_>) {
    use std::collections::{HashMap, HashSet};

    // Step 1: Find all kind-74 (callexpr) expression entity IDs
    let call_ids: HashSet<EntityId> = emitter.db.scan("exprs")
        .into_iter()
        .flatten()
        .filter(|t| t[1] == Value::Int(EXPR_CALL))
        .filter_map(|t| match t[0] { Value::Entity(id) => Some(id), _ => None })
        .collect();

    if call_ids.is_empty() {
        return;
    }

    // Step 2: For each call, find the child expression at index 0 (callee identifier)
    let callee_of_call: HashMap<EntityId, EntityId> = emitter.db.scan("exprparents")
        .into_iter()
        .flatten()
        .filter(|t| t[1] == Value::Int(0)) // child_index = 0
        .filter_map(|t| {
            if let (Value::Entity(child), Value::Entity(parent)) = (&t[0], &t[2]) {
                if call_ids.contains(parent) {
                    return Some((*parent, *child));
                }
            }
            None
        })
        .collect();

    // Step 3: Get valuetext for each callee entity
    let callee_entity_ids: HashSet<EntityId> = callee_of_call.values().copied().collect();
    let valuetext_map: HashMap<EntityId, Value> = emitter.db.scan("valuetext")
        .into_iter()
        .flatten()
        .filter_map(|t| {
            if let Value::Entity(id) = &t[0] {
                if callee_entity_ids.contains(id) {
                    return Some((*id, t[1].clone()));
                }
            }
            None
        })
        .collect();

    // Step 4: Build function name → entity ID map
    let func_map: HashMap<Value, EntityId> = emitter.db.scan("functions")
        .into_iter()
        .flatten()
        .filter_map(|t| {
            if let Value::Entity(id) = &t[0] {
                return Some((t[1].clone(), *id));
            }
            None
        })
        .collect();

    // Step 5: Resolve and collect funbind pairs
    let mut bindings = Vec::new();
    for (&call_id, &callee_id) in &callee_of_call {
        if let Some(name_val) = valuetext_map.get(&callee_id) {
            if let Some(&func_id) = func_map.get(name_val) {
                bindings.push((call_id, func_id));
            }
        }
    }

    // Step 6: Emit funbind rows
    for (call_id, func_id) in bindings {
        emitter.emit("funbind", vec![Value::Entity(call_id), Value::Entity(func_id)]);
    }
}

/// Extract all top-level declarations from a translation unit.
fn extract_translation_unit(
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
                extract_top_level(emitter, file_id, &node, source);
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// Extract a single top-level declaration.
fn extract_top_level(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    match node.kind() {
        "function_definition" => {
            extract_function(emitter, file_id, node, source, FUNCTION_NORMAL);
        }
        "declaration" => {
            extract_declaration(emitter, file_id, node, source);
        }
        "struct_specifier" | "class_specifier" | "union_specifier" | "enum_specifier" => {
            extract_type_specifier(emitter, file_id, node, source);
        }
        "preproc_include" => {
            extract_include(emitter, file_id, node, source);
        }
        "namespace_definition" => {
            extract_namespace(emitter, file_id, node, source);
        }
        "linkage_specification" => {
            // extern "C" { ... } — recurse into body
            if let Some(body) = node.child_by_field("body") {
                let mut cursor = body.walk();
                if cursor.goto_first_child() {
                    loop {
                        let child = cursor.node();
                        if child.is_named() {
                            extract_top_level(emitter, file_id, &child, source);
                        }
                        if !cursor.goto_next_sibling() {
                            break;
                        }
                    }
                }
            }
        }
        "type_definition" => {
            // typedef — extract the underlying type if it's a struct/class/union/enum
            extract_typedef(emitter, file_id, node, source);
        }
        "template_declaration" => {
            // template<...> declaration — extract the inner declaration
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() && child.kind() != "template_parameter_list" {
                        extract_top_level(emitter, file_id, &child, source);
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
        "comment" => {
            extract_comment(emitter, file_id, node, source);
        }
        "preproc_ifdef" | "preproc_if" | "preproc_else" | "preproc_elif" => {
            // Recurse into preprocessor conditional blocks
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_top_level(emitter, file_id, &child, source);
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
        _ => {}
    }
}

/// Extract a function definition.
fn extract_function(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    kind: i64,
) {
    // Get function name from the declarator
    let (name, kind) = match find_function_name(node, source) {
        Some((n, k)) => (n, k.unwrap_or(kind)),
        None => return,
    };

    let func_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    // Emit function
    let name_val = emitter.string(&name);
    emitter.emit("functions", vec![
        Value::Entity(func_id),
        name_val.clone(),
        Value::Int(kind),
    ]);

    // Emit location
    emitter.emit("element_location", vec![
        Value::Entity(func_id),
        Value::Entity(loc_id),
    ]);

    // Emit fun_decls (declaration entry for this function)
    let fun_decl_id = emitter.alloc();
    emitter.emit("fun_decls", vec![
        Value::Entity(fun_decl_id),
        Value::Entity(func_id),
        name_val.clone(),
        Value::Int(kind),
        Value::Entity(loc_id),
    ]);

    // Mark as definition (fun_def)
    emitter.emit("fun_def", vec![
        Value::Entity(fun_decl_id),
    ]);

    // Emit mangled name (use the plain name as a stand-in since tree-sitter doesn't mangle)
    let mangled_id = emitter.alloc();
    let mangled_val = emitter.string(&name);
    emitter.emit("manglednames", vec![
        Value::Entity(mangled_id),
        mangled_val,
    ]);
    emitter.emit("mangled_name", vec![
        Value::Entity(func_id),
        Value::Entity(mangled_id),
        Value::Int(1), // is_complete = 1
    ]);

    // Return type — includes pointer/reference from the declarator
    if let Some(type_node) = node.child_by_field("type") {
        let base_type = type_node.text(source);
        let suffix = node.child_by_field("declarator")
            .map(|d| match d.kind() {
                "pointer_declarator" | "reference_declarator" => {
                    // Count the outermost pointer/reference wrapping the function_declarator
                    fn outer_suffix(n: &Node) -> String {
                        match n.kind() {
                            "pointer_declarator" => {
                                let mut cursor = n.walk();
                                if cursor.goto_first_child() {
                                    loop {
                                        let c = cursor.node();
                                        if matches!(c.kind(), "pointer_declarator" | "reference_declarator") {
                                            return format!("*{}", outer_suffix(&c));
                                        }
                                        if c.kind() == "function_declarator" {
                                            return "*".to_string();
                                        }
                                        if !cursor.goto_next_sibling() { break; }
                                    }
                                }
                                "*".to_string()
                            }
                            "reference_declarator" => {
                                let mut cursor = n.walk();
                                if cursor.goto_first_child() {
                                    loop {
                                        let c = cursor.node();
                                        if matches!(c.kind(), "pointer_declarator" | "reference_declarator") {
                                            return format!("&{}", outer_suffix(&c));
                                        }
                                        if c.kind() == "function_declarator" {
                                            return "&".to_string();
                                        }
                                        if !cursor.goto_next_sibling() { break; }
                                    }
                                }
                                "&".to_string()
                            }
                            _ => String::new(),
                        }
                    }
                    outer_suffix(&d)
                }
                _ => String::new(),
            })
            .unwrap_or_default();
        let return_type = if suffix.is_empty() {
            base_type.to_string()
        } else {
            format!("{} {}", base_type, suffix)
        };
        let rt_val = emitter.string(&return_type);
        emitter.emit("function_return_type", vec![
            Value::Entity(func_id),
            rt_val,
        ]);
    }

    // Parameters
    if let Some(declarator) = node.child_by_field("declarator") {
        extract_parameters(emitter, func_id, &declarator, source);
    }

    // Function body — extract statements and expressions
    if let Some(body) = node.child_by_field("body") {
        if body.kind() == "compound_statement" {
            let body_stmt_id = extract_stmt(emitter, file_id, &body, source, func_id);
            if let Some(stmt_id) = body_stmt_id {
                emitter.emit("function_entry_point", vec![
                    Value::Entity(func_id),
                    Value::Entity(stmt_id),
                ]);
            }
        }
    }
}

/// Find the function name, traversing through pointer/reference declarators.
/// Returns (name, optional_kind_override) — kind override for constructors/destructors.
fn find_function_name(node: &Node<'_>, source: &[u8]) -> Option<(String, Option<i64>)> {
    let declarator = node.child_by_field("declarator")?;
    find_name_in_declarator(&declarator, source)
}

fn find_name_in_declarator(node: &Node<'_>, source: &[u8]) -> Option<(String, Option<i64>)> {
    match node.kind() {
        "function_declarator" => {
            if let Some(decl) = node.child_by_field("declarator") {
                return find_name_in_declarator(&decl, source);
            }
            None
        }
        "pointer_declarator" | "reference_declarator" => {
            if let Some(decl) = node.child_by_field("declarator") {
                return find_name_in_declarator(&decl, source);
            }
            // Fallback: iterate named children (reference_declarator may not use field names)
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        if let Some(result) = find_name_in_declarator(&child, source) {
                            return Some(result);
                        }
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
            None
        }
        "qualified_identifier" | "scoped_identifier" => {
            // Class::method — use the full qualified name
            Some((node.text(source).to_string(), None))
        }
        "destructor_name" => {
            Some((node.text(source).to_string(), Some(FUNCTION_DESTRUCTOR)))
        }
        "identifier" | "field_identifier" => {
            Some((node.text(source).to_string(), None))
        }
        "operator_name" => {
            Some((node.text(source).to_string(), None))
        }
        _ => {
            // Fallback: try to get text
            let text = node.text(source);
            if !text.is_empty() {
                Some((text.to_string(), None))
            } else {
                None
            }
        }
    }
}

/// Extract function parameters from a function_declarator.
fn extract_parameters(
    emitter: &mut FactEmitter<'_>,
    func_id: EntityId,
    declarator: &Node<'_>,
    source: &[u8],
) {
    // Find the parameter_list child (may be nested inside function_declarator)
    let params_node = if declarator.kind() == "function_declarator" {
        declarator.child_by_field("parameters")
    } else {
        // Try to find function_declarator inside pointer/reference declarators
        find_params_list(declarator)
    };

    if let Some(params) = params_node {
        let mut index = 0i64;
        let mut cursor = params.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "parameter_declaration" || child.kind() == "optional_parameter_declaration" {
                    let full_type = build_full_type(&child, source);
                    let param_type = if full_type.is_empty() { "unknown".to_string() } else { full_type };
                    let param_name = child.child_by_field("declarator")
                        .map(|d| extract_declarator_name(&d, source))
                        .unwrap_or_default();

                    let type_val = emitter.string(&param_type);
                    let name_val = emitter.string(&param_name);
                    emitter.emit("params", vec![
                        Value::Entity(func_id),
                        Value::Int(index),
                        name_val,
                        type_val,
                    ]);
                    index += 1;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }
}

fn find_params_list<'a>(node: &Node<'a>) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "parameter_list" {
                return Some(child);
            }
            if let Some(found) = find_params_list(&child) {
                return Some(found);
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    None
}

/// Extract the name from a declarator node (e.g., pointer_declarator → identifier).
fn extract_declarator_name(node: &Node<'_>, source: &[u8]) -> String {
    match node.kind() {
        "identifier" | "field_identifier" => node.text(source).to_string(),
        "pointer_declarator" | "reference_declarator" | "array_declarator"
        | "function_declarator" => {
            if let Some(decl) = node.child_by_field("declarator") {
                extract_declarator_name(&decl, source)
            } else {
                String::new()
            }
        }
        "parenthesized_declarator" => {
            // `(* name)` — inner declarator is a positional child (no field name).
            // Walk children and recurse on the first non-punctuation node.
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() != "(" && child.kind() != ")" {
                        return extract_declarator_name(&child, source);
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
            String::new()
        }
        _ => {
            // Try the declarator field first, then text
            if let Some(decl) = node.child_by_field("declarator") {
                extract_declarator_name(&decl, source)
            } else {
                node.text(source).to_string()
            }
        }
    }
}

/// Extract a top-level declaration (may contain variable declarations or type definitions).
fn extract_declaration(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    // Check if this declaration contains a type specifier (struct/class/union/enum)
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "struct_specifier" | "class_specifier" | "union_specifier" | "enum_specifier" => {
                    extract_type_specifier(emitter, file_id, &child, source);
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    // Also check if there are variable declarators
    if let Some(type_node) = node.child_by_field("type") {
        let type_text = type_node.text(source);
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "init_declarator" || child.kind() == "declarator" {
                    let decl_node = if child.kind() == "init_declarator" {
                        child.child_by_field("declarator").unwrap_or(child)
                    } else {
                        child
                    };
                    let var_name = extract_declarator_name(&decl_node, source);
                    if !var_name.is_empty() && !var_name.contains('(') {
                        let var_id = emitter.alloc();
                        let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                        let name_val = emitter.string(&var_name);
                        let type_val = emitter.string(type_text);
                        emitter.emit("variables", vec![
                            Value::Entity(var_id),
                            name_val.clone(),
                            type_val.clone(),
                            Value::Int(VAR_GLOBAL),
                        ]);
                        emitter.emit("globalvariables", vec![
                            Value::Entity(var_id),
                            name_val,
                            type_val,
                        ]);
                        emitter.emit("element_location", vec![
                            Value::Entity(var_id),
                            Value::Entity(loc_id),
                        ]);
                    }
                }
                // Handle plain identifier declarators (e.g., `int x;`)
                if child.kind() == "identifier" && child != type_node {
                    let var_name = child.text(source);
                    if !var_name.is_empty() {
                        let var_id = emitter.alloc();
                        let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                        let name_val = emitter.string(var_name);
                        let type_val = emitter.string(type_text);
                        emitter.emit("variables", vec![
                            Value::Entity(var_id),
                            name_val.clone(),
                            type_val.clone(),
                            Value::Int(VAR_GLOBAL),
                        ]);
                        emitter.emit("globalvariables", vec![
                            Value::Entity(var_id),
                            name_val,
                            type_val,
                        ]);
                        emitter.emit("element_location", vec![
                            Value::Entity(var_id),
                            Value::Entity(loc_id),
                        ]);
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }
}

/// Extract a struct/class/union/enum type specifier.
/// `name_override` is used by `extract_typedef` to supply the typedef name
/// for anonymous structs (e.g., `typedef struct { ... } Point;` → name = "Point").
fn extract_type_specifier(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    extract_type_specifier_impl(emitter, file_id, node, source, None);
}

fn extract_type_specifier_with_name(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    name_override: &str,
) {
    extract_type_specifier_impl(emitter, file_id, node, source, Some(name_override));
}

fn extract_type_specifier_impl(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    name_override: Option<&str>,
) {
    let kind = match node.kind() {
        "struct_specifier" => USERTYPE_STRUCT,
        "class_specifier" => USERTYPE_CLASS,
        "union_specifier" => USERTYPE_UNION,
        "enum_specifier" => USERTYPE_ENUM,
        _ => return,
    };

    // Only extract types that have a body (definition, not forward declaration or type reference).
    // `struct Foo;` and `struct Foo *ptr` have no body — skip them.
    if node.child_by_field("body").is_none() {
        return;
    }

    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();

    // Use the typedef name if the struct itself is anonymous
    let effective_name = if !name.is_empty() {
        name
    } else if let Some(override_name) = name_override {
        override_name.to_string()
    } else {
        String::new()
    };

    let type_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let name_val = emitter.string(if effective_name.is_empty() { "<anonymous>" } else { &effective_name });
    emitter.emit("usertypes", vec![
        Value::Entity(type_id),
        name_val,
        Value::Int(kind),
    ]);
    emitter.emit("element_location", vec![
        Value::Entity(type_id),
        Value::Entity(loc_id),
    ]);

    // Extract base classes
    if let Some(base_clause) = node.child_by_field("base_clause") {
        // In tree-sitter-cpp, base_clause is a "base_class_clause"
        extract_base_classes(emitter, type_id, &base_clause, source);
    }
    // Also check for ":" followed by base specifiers directly
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "base_class_clause" {
                extract_base_classes(emitter, type_id, &child, source);
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    // Extract fields and member functions (or enum constants for enums)
    if let Some(body) = node.child_by_field("body") {
        if kind == USERTYPE_ENUM {
            extract_enum_constants(emitter, file_id, type_id, &body, source);
        } else {
            extract_class_body(emitter, file_id, type_id, &body, source);
        }
    }
}

/// Extract base class references from a base_class_clause.
fn extract_base_classes(
    emitter: &mut FactEmitter<'_>,
    type_id: EntityId,
    base_clause: &Node<'_>,
    source: &[u8],
) {
    let mut index = 0i64;
    let mut cursor = base_clause.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            // Look for type_identifier or qualified_identifier
            if child.kind() == "base_class_specifier" || child.kind() == "type_identifier"
                || child.kind() == "qualified_identifier"
            {
                let base_name = if child.kind() == "base_class_specifier" {
                    // Find the type inside the specifier
                    find_type_in_base_specifier(&child, source)
                } else {
                    child.text(source).to_string()
                };
                if !base_name.is_empty() {
                    let name_val = emitter.string(&base_name);
                    emitter.emit("derivations", vec![
                        Value::Entity(type_id),
                        Value::Int(index),
                        name_val,
                    ]);
                    index += 1;
                }
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

fn find_type_in_base_specifier(node: &Node<'_>, source: &[u8]) -> String {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "type_identifier" || child.kind() == "qualified_identifier" {
                return child.text(source).to_string();
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    node.text(source).to_string()
}

/// Extract members from a class/struct body.
fn extract_class_body(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    type_id: EntityId,
    body: &Node<'_>,
    source: &[u8],
) {
    let mut field_index = 0i64;
    let mut cursor = body.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "field_declaration" => {
                    // Check if this field_declaration contains a nested type definition
                    // (with a body), vs just using a type as a field type reference
                    let mut inner = child.walk();
                    if inner.goto_first_child() {
                        loop {
                            let fc = inner.node();
                            match fc.kind() {
                                "struct_specifier" | "class_specifier" | "union_specifier" | "enum_specifier" => {
                                    // Only extract as nested type if it has a body (definition, not reference)
                                    if fc.child_by_field("body").is_some() {
                                        extract_type_specifier(emitter, file_id, &fc, source);
                                    }
                                }
                                _ => {}
                            }
                            if !inner.goto_next_sibling() { break; }
                        }
                    }
                    extract_field(emitter, type_id, &child, source, &mut field_index);
                }
                "function_definition" => {
                    // Member function with body
                    extract_function(emitter, file_id, &child, source, FUNCTION_NORMAL);
                }
                "declaration" => {
                    // Could be a member function declaration or field
                    // Check if it has a function declarator
                    let has_func = has_function_declarator(&child);
                    if has_func {
                        // It's a member function declaration (no body)
                        extract_member_function_decl(emitter, file_id, &child, source);
                    } else {
                        extract_field_from_declaration(emitter, type_id, &child, source, &mut field_index);
                    }
                }
                "template_declaration" => {
                    // Template member — recurse
                    let mut inner = child.walk();
                    if inner.goto_first_child() {
                        loop {
                            let inner_child = inner.node();
                            if inner_child.kind() == "function_definition" {
                                extract_function(emitter, file_id, &inner_child, source, FUNCTION_NORMAL);
                            }
                            if !inner.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

fn has_function_declarator(node: &Node<'_>) -> bool {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "function_declarator" {
                return true;
            }
            if has_function_declarator(&child) {
                return true;
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    false
}

/// Extract a member function declaration (without body).
fn extract_member_function_decl(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let type_text = node.child_by_field("type")
        .map(|t| t.text(source))
        .unwrap_or("void");

    // Find the function name
    let mut func_name = String::new();
    let mut kind = FUNCTION_NORMAL;
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "function_declarator" {
                if let Some((n, k)) = find_name_in_declarator(&child, source) {
                    func_name = n;
                    kind = k.unwrap_or(FUNCTION_NORMAL);
                }
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    if !func_name.is_empty() {
        let func_id = emitter.alloc();
        let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
        let name_val = emitter.string(&func_name);
        emitter.emit("functions", vec![
            Value::Entity(func_id),
            name_val,
            Value::Int(kind),
        ]);
        emitter.emit("element_location", vec![
            Value::Entity(func_id),
            Value::Entity(loc_id),
        ]);

        let rt_val = emitter.string(type_text);
        emitter.emit("function_return_type", vec![
            Value::Entity(func_id),
            rt_val,
        ]);
    }
}

/// Extract a field from a field_declaration node.
fn extract_field(
    emitter: &mut FactEmitter<'_>,
    type_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    field_index: &mut i64,
) {
    // Build the full type including pointer/reference/array qualifiers
    let full_type = build_full_type(node, source);
    let field_type = if full_type.is_empty() { "unknown".to_string() } else { full_type };

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "field_identifier" => {
                    let field_name = child.text(source);
                    let name_val = emitter.string(field_name);
                    let type_val = emitter.string(&field_type);
                    emitter.emit("fields", vec![
                        Value::Entity(type_id),
                        Value::Int(*field_index),
                        name_val,
                        type_val,
                    ]);
                    *field_index += 1;
                }
                "pointer_declarator" | "reference_declarator" | "array_declarator" => {
                    // field_identifier is nested inside these wrapper declarators
                    if let Some(name) = find_field_identifier(&child, source) {
                        let name_val = emitter.string(&name);
                        let type_val = emitter.string(&field_type);
                        emitter.emit("fields", vec![
                            Value::Entity(type_id),
                            Value::Int(*field_index),
                            name_val,
                            type_val,
                        ]);
                        *field_index += 1;
                    }
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// Extract the type suffix from a declarator chain.
///
/// Walks the declarator nodes outside-in, collecting pointer/reference/array qualifiers.
/// For example:
///   `pointer_declarator(pointer_declarator(field_identifier))` → "**"
///   `pointer_declarator(field_identifier)` → "*"
///   `array_declarator(field_identifier, 64)` → "[64]"
///   `reference_declarator(function_declarator(...))` → "&"
fn declarator_type_suffix(node: &Node<'_>, source: &[u8]) -> String {
    match node.kind() {
        "pointer_declarator" => {
            let mut inner = String::new();
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if matches!(child.kind(),
                        "pointer_declarator" | "reference_declarator" | "array_declarator"
                    ) {
                        inner = declarator_type_suffix(&child, source);
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
            format!("*{}", inner)
        }
        "reference_declarator" => {
            let mut inner = String::new();
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if matches!(child.kind(),
                        "pointer_declarator" | "reference_declarator" | "array_declarator"
                    ) {
                        inner = declarator_type_suffix(&child, source);
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
            format!("&{}", inner)
        }
        "array_declarator" => {
            let mut size_text = String::new();
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "number_literal" {
                        size_text = child.text(source).to_string();
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
            format!("[{}]", size_text)
        }
        _ => String::new(),
    }
}

/// Build the full type string for a field or parameter declaration,
/// including type qualifiers and pointer/reference/array suffixes.
///
/// For `const char *name`, returns "const char *".
/// For `struct Node **next`, returns "struct Node **".
/// For `char name[64]`, returns "char[64]".
fn build_full_type(node: &Node<'_>, source: &[u8]) -> String {
    let mut type_parts = Vec::new();
    let mut suffix = String::new();

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "type_qualifier" => {
                    type_parts.push(child.text(source).to_string());
                }
                "primitive_type" | "type_identifier" | "sized_type_specifier"
                | "struct_specifier" | "union_specifier" | "enum_specifier"
                | "class_specifier" => {
                    type_parts.push(child.text(source).to_string());
                }
                "pointer_declarator" | "reference_declarator" | "array_declarator" => {
                    suffix = declarator_type_suffix(&child, source);
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }

    let base = type_parts.join(" ");
    if suffix.is_empty() {
        base
    } else {
        format!("{} {}", base, suffix)
    }
}

/// Recursively find a field_identifier inside a declarator node.
fn find_field_identifier(node: &Node<'_>, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "field_identifier" {
                return Some(child.text(source).to_string());
            }
            if let Some(name) = find_field_identifier(&child, source) {
                return Some(name);
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    None
}

/// Extract fields from a declaration inside a class body.
fn extract_field_from_declaration(
    emitter: &mut FactEmitter<'_>,
    type_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    field_index: &mut i64,
) {
    let full_type = build_full_type(node, source);
    let field_type = if full_type.is_empty() { "unknown" } else { &full_type };

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "identifier" || child.kind() == "field_identifier" {
                let name = child.text(source);
                // Skip if this is the type itself
                if let Some(type_node) = node.child_by_field("type") {
                    if child.id() == type_node.id() {
                        if !cursor.goto_next_sibling() { break; }
                        continue;
                    }
                }
                let name_val = emitter.string(name);
                let type_val = emitter.string(field_type);
                emitter.emit("fields", vec![
                    Value::Entity(type_id),
                    Value::Int(*field_index),
                    name_val,
                    type_val,
                ]);
                *field_index += 1;
            }
            if child.kind() == "init_declarator" {
                if let Some(decl) = child.child_by_field("declarator") {
                    let name = extract_declarator_name(&decl, source);
                    if !name.is_empty() {
                        let name_val = emitter.string(&name);
                        let type_val = emitter.string(field_type);
                        emitter.emit("fields", vec![
                            Value::Entity(type_id),
                            Value::Int(*field_index),
                            name_val,
                            type_val,
                        ]);
                        *field_index += 1;
                    }
                }
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// Extract a typedef.
///
/// For `typedef struct { ... } Point;`, the tree-sitter AST is:
///   type_definition
///     struct_specifier (anonymous — no name field)
///       field_declaration_list { ... }
///     type_identifier "Point"     ← the typedef name
///
/// We pass the typedef name to `extract_type_specifier_with_name` so
/// that anonymous structs get the typedef name instead of `<anonymous>`.
fn extract_typedef(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    // Find the typedef name (type_identifier child of type_definition)
    let typedef_name = {
        let mut name = None;
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "type_identifier" {
                    name = Some(child.text(source).to_string());
                }
                if !cursor.goto_next_sibling() { break; }
            }
        }
        name.unwrap_or_default()
    };

    // Extract the underlying type specifier
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "struct_specifier" | "class_specifier" | "union_specifier" | "enum_specifier" => {
                    if typedef_name.is_empty() {
                        extract_type_specifier(emitter, file_id, &child, source);
                    } else {
                        extract_type_specifier_with_name(
                            emitter, file_id, &child, source, &typedef_name,
                        );
                    }
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Map a tree-sitter statement node kind to a CodeQL statement kind integer.
/// Returns None for unrecognized kinds.
fn stmt_kind(node_kind: &str) -> Option<i64> {
    match node_kind {
        "compound_statement" => Some(STMT_BLOCK),
        "expression_statement" => Some(STMT_EXPR),
        "if_statement" => Some(STMT_IF),
        "while_statement" => Some(STMT_WHILE),
        "for_statement" => Some(STMT_FOR),
        "return_statement" => Some(STMT_RETURN),
        "do_statement" => Some(STMT_DO_WHILE),
        "switch_statement" => Some(STMT_SWITCH),
        "case_statement" => Some(STMT_SWITCH_CASE),
        "break_statement" => Some(STMT_BREAK),
        "continue_statement" => Some(STMT_CONTINUE),
        "goto_statement" => Some(STMT_GOTO),
        "labeled_statement" => Some(STMT_LABEL),
        "declaration" => Some(STMT_DECL),
        _ => None,
    }
}

/// Map a tree-sitter binary operator to a CodeQL expression kind.
fn binary_op_kind(op: &str) -> i64 {
    match op {
        "+" => EXPR_ADD,
        "-" => EXPR_SUB,
        "*" => EXPR_MUL,
        "/" => EXPR_DIV,
        "%" => EXPR_REM,
        "&" => EXPR_BITAND,
        "|" => EXPR_BITOR,
        "^" => EXPR_BITXOR,
        "<<" => EXPR_LSHIFT,
        ">>" => EXPR_RSHIFT,
        "==" => EXPR_EQ,
        "!=" => EXPR_NE,
        "<" => EXPR_LT,
        ">" => EXPR_GT,
        "<=" => EXPR_LE,
        ">=" => EXPR_GE,
        "&&" => EXPR_LOGAND,
        "||" => EXPR_LOGOR,
        _ => EXPR_ADD, // fallback
    }
}

/// Map a tree-sitter assignment operator to a CodeQL expression kind.
fn assign_op_kind(op: &str) -> i64 {
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
        _ => EXPR_ASSIGN,
    }
}

/// Extract a statement node, emitting to the `stmts` table.
/// Returns the entity ID of the emitted statement, or None if the node isn't a statement.
/// `enclosing_func` is the function this statement belongs to (for `enclosingfunction`).
fn extract_stmt(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    enclosing_func: EntityId,
) -> Option<EntityId> {
    let kind = stmt_kind(node.kind())?;

    let stmt_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    emitter.emit("stmts", vec![
        Value::Entity(stmt_id),
        Value::Int(kind),
        Value::Entity(loc_id),
    ]);

    emitter.emit("enclosingfunction", vec![
        Value::Entity(stmt_id),
        Value::Entity(enclosing_func),
    ]);

    // Process children depending on statement kind
    match node.kind() {
        "compound_statement" => {
            let mut child_index = 0i64;
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        if let Some(child_id) = extract_stmt_or_expr_child(
                            emitter, file_id, &child, source, enclosing_func,
                        ) {
                            emitter.emit("stmtparents", vec![
                                Value::Entity(child_id),
                                Value::Int(child_index),
                                Value::Entity(stmt_id),
                            ]);
                            child_index += 1;
                        }
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "expression_statement" => {
            // The expression child
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, &child, source, stmt_id, 0, enclosing_func);
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "if_statement" => {
            // condition expression
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, stmt_id, 0, enclosing_func);
            }
            // then branch
            if let Some(consequence) = node.child_by_field("consequence") {
                if let Some(then_id) = extract_stmt_or_expr_child(
                    emitter, file_id, &consequence, source, enclosing_func,
                ) {
                    emitter.emit("if_then", vec![
                        Value::Entity(stmt_id),
                        Value::Entity(then_id),
                    ]);
                    emitter.emit("stmtparents", vec![
                        Value::Entity(then_id),
                        Value::Int(0),
                        Value::Entity(stmt_id),
                    ]);
                }
            }
            // else branch — tree-sitter wraps it in an `else_clause` node
            if let Some(alternative) = node.child_by_field("alternative") {
                // Unwrap else_clause to get the actual body (compound_statement or if_statement)
                let else_body = if alternative.kind() == "else_clause" {
                    alternative.named_children_iter().next()
                } else {
                    Some(alternative)
                };
                if let Some(else_body) = else_body {
                    if let Some(else_id) = extract_stmt_or_expr_child(
                        emitter, file_id, &else_body, source, enclosing_func,
                    ) {
                        emitter.emit("if_else", vec![
                            Value::Entity(stmt_id),
                            Value::Entity(else_id),
                        ]);
                        emitter.emit("stmtparents", vec![
                            Value::Entity(else_id),
                            Value::Int(1),
                            Value::Entity(stmt_id),
                        ]);
                    }
                }
            }
        }
        "while_statement" => {
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, stmt_id, 0, enclosing_func);
            }
            if let Some(body) = node.child_by_field("body") {
                if let Some(body_id) = extract_stmt_or_expr_child(
                    emitter, file_id, &body, source, enclosing_func,
                ) {
                    emitter.emit("while_body", vec![
                        Value::Entity(stmt_id),
                        Value::Entity(body_id),
                    ]);
                    emitter.emit("stmtparents", vec![
                        Value::Entity(body_id),
                        Value::Int(0),
                        Value::Entity(stmt_id),
                    ]);
                }
            }
        }
        "do_statement" => {
            if let Some(body) = node.child_by_field("body") {
                if let Some(body_id) = extract_stmt_or_expr_child(
                    emitter, file_id, &body, source, enclosing_func,
                ) {
                    emitter.emit("do_body", vec![
                        Value::Entity(stmt_id),
                        Value::Entity(body_id),
                    ]);
                    emitter.emit("stmtparents", vec![
                        Value::Entity(body_id),
                        Value::Int(0),
                        Value::Entity(stmt_id),
                    ]);
                }
            }
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, stmt_id, 1, enclosing_func);
            }
        }
        "for_statement" => {
            // initializer, condition, update are children
            if let Some(init) = node.child_by_field("initializer") {
                extract_expr(emitter, file_id, &init, source, stmt_id, 0, enclosing_func);
            }
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, stmt_id, 1, enclosing_func);
            }
            if let Some(update) = node.child_by_field("update") {
                extract_expr(emitter, file_id, &update, source, stmt_id, 2, enclosing_func);
            }
            if let Some(body) = node.child_by_field("body") {
                if let Some(body_id) = extract_stmt_or_expr_child(
                    emitter, file_id, &body, source, enclosing_func,
                ) {
                    emitter.emit("for_body", vec![
                        Value::Entity(stmt_id),
                        Value::Entity(body_id),
                    ]);
                    emitter.emit("stmtparents", vec![
                        Value::Entity(body_id),
                        Value::Int(0),
                        Value::Entity(stmt_id),
                    ]);
                }
            }
        }
        "switch_statement" => {
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, stmt_id, 0, enclosing_func);
            }
            if let Some(body) = node.child_by_field("body") {
                if let Some(body_id) = extract_stmt_or_expr_child(
                    emitter, file_id, &body, source, enclosing_func,
                ) {
                    emitter.emit("switch_body", vec![
                        Value::Entity(stmt_id),
                        Value::Entity(body_id),
                    ]);
                    emitter.emit("stmtparents", vec![
                        Value::Entity(body_id),
                        Value::Int(0),
                        Value::Entity(stmt_id),
                    ]);
                }
            }
        }
        "case_statement" => {
            // case value: — extract the value expression
            if let Some(val) = node.child_by_field("value") {
                extract_expr(emitter, file_id, &val, source, stmt_id, 0, enclosing_func);
            }
            // Extract child statements
            let mut child_index = 0i64;
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() && child.kind() != "identifier" && child.kind() != "number_literal" {
                        if let Some(child_id) = extract_stmt_or_expr_child(
                            emitter, file_id, &child, source, enclosing_func,
                        ) {
                            emitter.emit("stmtparents", vec![
                                Value::Entity(child_id),
                                Value::Int(child_index),
                                Value::Entity(stmt_id),
                            ]);
                            child_index += 1;
                        }
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "return_statement" => {
            // return expr;
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, &child, source, stmt_id, 0, enclosing_func);
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "declaration" => {
            // Local variable declaration inside a function body
            extract_local_vars(emitter, file_id, node, source, enclosing_func);
        }
        "labeled_statement" => {
            // label: stmt — extract the child statement
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() && child.kind() != "statement_identifier" {
                        if let Some(child_id) = extract_stmt_or_expr_child(
                            emitter, file_id, &child, source, enclosing_func,
                        ) {
                            emitter.emit("stmtparents", vec![
                                Value::Entity(child_id),
                                Value::Int(0),
                                Value::Entity(stmt_id),
                            ]);
                        }
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        _ => {}
    }

    Some(stmt_id)
}

/// Try to extract a child as either a statement or wrap an expression statement.
/// Returns the entity ID of whatever was emitted.
fn extract_stmt_or_expr_child(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    enclosing_func: EntityId,
) -> Option<EntityId> {
    // If it's a recognized statement kind, extract as statement
    if stmt_kind(node.kind()).is_some() {
        return extract_stmt(emitter, file_id, node, source, enclosing_func);
    }
    // Otherwise it's not a statement we handle — skip it
    None
}

/// Extract an expression node, emitting to the `exprs` and `exprparents` tables.
fn extract_expr(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    parent_id: EntityId,
    child_index: i64,
    enclosing_func: EntityId,
) -> Option<EntityId> {
    let kind = match node.kind() {
        "number_literal" | "string_literal" | "char_literal" | "true" | "false" | "null"
        | "string_content" | "concatenated_string" => Some(EXPR_LITERAL),
        "identifier" => Some(EXPR_VARACCESS),
        "call_expression" => Some(EXPR_CALL),
        "parenthesized_expression" => Some(EXPR_PAREXPR),
        "conditional_expression" => Some(EXPR_CONDITIONAL),
        "subscript_expression" => Some(EXPR_SUBSCRIPT),
        "comma_expression" => Some(EXPR_COMMA),
        "sizeof_expression" => Some(EXPR_SIZEOF),
        "cast_expression" => Some(EXPR_CAST),
        "field_expression" => Some(EXPR_FIELD_ACCESS),
        "binary_expression" => {
            // Determine the operator
            let op = find_operator(node, source);
            Some(binary_op_kind(&op))
        }
        "unary_expression" => {
            let op = find_operator(node, source);
            match op.as_str() {
                "!" => Some(EXPR_LOGNOT),
                "~" => Some(EXPR_BITNOT),
                "-" => Some(EXPR_UNARYMINUS),
                "+" => Some(EXPR_UNARYPLUS),
                _ => Some(EXPR_UNARYMINUS),
            }
        }
        "pointer_expression" => {
            let op = find_operator(node, source);
            match op.as_str() {
                "&" => Some(EXPR_ADDRESS_OF),
                "*" => Some(EXPR_INDIRECT),
                _ => Some(EXPR_INDIRECT),
            }
        }
        "assignment_expression" => {
            let op = find_operator(node, source);
            Some(assign_op_kind(&op))
        }
        "update_expression" => {
            // prefix/postfix ++ or --
            let op = find_operator(node, source);
            let is_prefix = node.child(0)
                .map(|c| !c.is_named())
                .unwrap_or(false);
            match (op.as_str(), is_prefix) {
                ("++", true) => Some(EXPR_PREFIXINC),
                ("++", false) => Some(EXPR_POSTFIXINC),
                ("--", true) => Some(EXPR_PREFIXDEC),
                ("--", false) => Some(EXPR_POSTFIXDEC),
                _ => Some(EXPR_PREFIXINC),
            }
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
        Value::Entity(loc_id),
    ]);

    emitter.emit("exprparents", vec![
        Value::Entity(expr_id),
        Value::Int(child_index),
        Value::Entity(parent_id),
    ]);

    emitter.emit("enclosingfunction", vec![
        Value::Entity(expr_id),
        Value::Entity(enclosing_func),
    ]);

    // Value text: literal values and identifier names
    if expr_kind == EXPR_LITERAL || expr_kind == EXPR_VARACCESS || expr_kind == EXPR_FIELD_ACCESS {
        let text = node.text(source);
        let text_val = emitter.string(text);
        emitter.emit("valuetext", vec![
            Value::Entity(expr_id),
            text_val,
        ]);
    }

    // Recurse into child expressions
    match node.kind() {
        "call_expression" => {
            // Emit iscall(expr_id, 0) — marks this expression as a function call
            emitter.emit("iscall", vec![
                Value::Entity(expr_id),
                Value::Int(0),
            ]);

            // Callee function name (child 0) — extracted as child expression for
            // backward compatibility, also used for funbind resolution post-pass
            if let Some(func) = node.child_by_field("function") {
                extract_expr(emitter, file_id, &func, source, expr_id, 0, enclosing_func);
            }
            // Arguments (children 1+)
            if let Some(args) = node.child_by_field("arguments") {
                let mut idx = 1i64;
                let mut cursor = args.walk();
                if cursor.goto_first_child() {
                    loop {
                        let child = cursor.node();
                        if child.is_named() {
                            extract_expr(emitter, file_id, &child, source, expr_id, idx, enclosing_func);
                            idx += 1;
                        }
                        if !cursor.goto_next_sibling() { break; }
                    }
                }
            }
        }
        "binary_expression" => {
            if let Some(left) = node.child_by_field("left") {
                extract_expr(emitter, file_id, &left, source, expr_id, 0, enclosing_func);
            }
            if let Some(right) = node.child_by_field("right") {
                extract_expr(emitter, file_id, &right, source, expr_id, 1, enclosing_func);
            }
        }
        "unary_expression" | "pointer_expression" => {
            if let Some(arg) = node.child_by_field("argument") {
                extract_expr(emitter, file_id, &arg, source, expr_id, 0, enclosing_func);
            }
        }
        "assignment_expression" => {
            if let Some(left) = node.child_by_field("left") {
                extract_expr(emitter, file_id, &left, source, expr_id, 0, enclosing_func);
            }
            if let Some(right) = node.child_by_field("right") {
                extract_expr(emitter, file_id, &right, source, expr_id, 1, enclosing_func);
            }
        }
        "update_expression" => {
            if let Some(arg) = node.child_by_field("argument") {
                extract_expr(emitter, file_id, &arg, source, expr_id, 0, enclosing_func);
            }
        }
        "parenthesized_expression" => {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        extract_expr(emitter, file_id, &child, source, expr_id, 0, enclosing_func);
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        "conditional_expression" => {
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, expr_id, 0, enclosing_func);
            }
            if let Some(consequence) = node.child_by_field("consequence") {
                extract_expr(emitter, file_id, &consequence, source, expr_id, 1, enclosing_func);
            }
            if let Some(alternative) = node.child_by_field("alternative") {
                extract_expr(emitter, file_id, &alternative, source, expr_id, 2, enclosing_func);
            }
        }
        "subscript_expression" => {
            if let Some(arg) = node.child_by_field("argument") {
                extract_expr(emitter, file_id, &arg, source, expr_id, 0, enclosing_func);
            }
            if let Some(idx) = node.child_by_field("index") {
                extract_expr(emitter, file_id, &idx, source, expr_id, 1, enclosing_func);
            }
        }
        "field_expression" => {
            if let Some(arg) = node.child_by_field("argument") {
                extract_expr(emitter, file_id, &arg, source, expr_id, 0, enclosing_func);
            }
        }
        "cast_expression" => {
            if let Some(val) = node.child_by_field("value") {
                extract_expr(emitter, file_id, &val, source, expr_id, 0, enclosing_func);
            }
        }
        "sizeof_expression" => {
            if let Some(val) = node.child_by_field("value") {
                extract_expr(emitter, file_id, &val, source, expr_id, 0, enclosing_func);
            }
        }
        "comma_expression" => {
            if let Some(left) = node.child_by_field("left") {
                extract_expr(emitter, file_id, &left, source, expr_id, 0, enclosing_func);
            }
            if let Some(right) = node.child_by_field("right") {
                extract_expr(emitter, file_id, &right, source, expr_id, 1, enclosing_func);
            }
        }
        _ => {}
    }

    Some(expr_id)
}

/// Find an operator in a node's unnamed children (e.g., "+", "-", "=", etc.)
fn find_operator(node: &Node<'_>, source: &[u8]) -> String {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if !child.is_named() {
                let text = child.text(source);
                match text {
                    "+" | "-" | "*" | "/" | "%" | "&" | "|" | "^" | "<<" | ">>"
                    | "==" | "!=" | "<" | ">" | "<=" | ">=" | "&&" | "||"
                    | "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^="
                    | "<<=" | ">>=" | "!" | "~" | "++" | "--" => {
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

/// Extract local variable declarations from a declaration statement inside a function body.
fn extract_local_vars(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    enclosing_func: EntityId,
) {
    if let Some(type_node) = node.child_by_field("type") {
        let type_text = type_node.text(source);
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "init_declarator" || child.kind() == "identifier"
                    || child.kind() == "pointer_declarator" || child.kind() == "array_declarator"
                {
                    let (decl_node, init_node) = if child.kind() == "init_declarator" {
                        (child.child_by_field("declarator").unwrap_or(child), child.child_by_field("value"))
                    } else {
                        (child, None)
                    };
                    let var_name = extract_declarator_name(&decl_node, source);
                    if !var_name.is_empty() && !var_name.contains('(') {
                        let var_id = emitter.alloc();
                        let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                        let name_val = emitter.string(&var_name);
                        let type_val = emitter.string(type_text);
                        emitter.emit("localvariables", vec![
                            Value::Entity(var_id),
                            name_val.clone(),
                            type_val,
                        ]);
                        emitter.emit("element_location", vec![
                            Value::Entity(var_id),
                            Value::Entity(loc_id),
                        ]);
                        emitter.emit("enclosingfunction", vec![
                            Value::Entity(var_id),
                            Value::Entity(enclosing_func),
                        ]);

                        // Also emit the initializer expression if present
                        if let Some(init) = init_node {
                            // We don't have a parent stmt id here conveniently,
                            // but the enclosing function is what matters
                            let _ = extract_expr(emitter, file_id, &init, source, var_id, 0, enclosing_func);
                        }
                    }
                }
                if !cursor.goto_next_sibling() { break; }
            }
        }
    }
}

/// Extract enum constants from an enum body.
fn extract_enum_constants(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    type_id: EntityId,
    body: &Node<'_>,
    source: &[u8],
) {
    let mut index = 0i64;
    let mut cursor = body.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "enumerator" {
                if let Some(name_node) = child.child_by_field("name") {
                    let name = name_node.text(source);
                    let const_id = emitter.alloc();
                    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &child);
                    let name_val = emitter.string(name);
                    emitter.emit("enumconstants", vec![
                        Value::Entity(const_id),
                        Value::Entity(type_id),
                        Value::Int(index),
                        name_val,
                        Value::Entity(loc_id),
                    ]);
                    emitter.emit("element_location", vec![
                        Value::Entity(const_id),
                        Value::Entity(loc_id),
                    ]);
                    index += 1;
                }
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Extract a namespace definition, emitting to the `namespaces` and `namespacembrs` tables.
fn extract_namespace(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();

    let ns_id = emitter.alloc();
    let name_val = emitter.string(if name.is_empty() { "<anonymous>" } else { &name });
    emitter.emit("namespaces", vec![
        Value::Entity(ns_id),
        name_val,
    ]);

    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
    emitter.emit("element_location", vec![
        Value::Entity(ns_id),
        Value::Entity(loc_id),
    ]);

    // Extract body and record membership
    if let Some(body) = node.child_by_field("body") {
        let mut cursor = body.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.is_named() {
                    extract_top_level(emitter, file_id, &child, source);
                }
                if !cursor.goto_next_sibling() { break; }
            }
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

/// Extract an #include directive.
fn extract_include(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    if let Some(path_node) = node.child_by_field("path") {
        let include_id = emitter.alloc();
        let path_text = path_node.text(source);
        let path_val = emitter.string(path_text);
        emitter.emit("includes", vec![
            Value::Entity(include_id),
            Value::Entity(file_id),
            path_val,
        ]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocql_database::Database;
    use crate::schema::cpp_schema;

    fn extract_test_file(filename: &str) -> Database {
        let source = std::fs::read(
            format!("tests/fixtures/{}", filename)
        ).unwrap();
        let schema = cpp_schema();
        let mut db = Database::from_schema(schema);
        let extractor = if filename.ends_with(".c") {
            CppExtractor::c()
        } else {
            CppExtractor::cpp()
        };
        let result = extractor.extract_source(&mut db, filename, &source);
        assert!(result.success, "Extraction failed: {:?}", result.error);
        db
    }

    #[test]
    fn test_extract_cpp_functions() {
        let db = extract_test_file("simple.cpp");
        let functions: Vec<_> = db.scan("functions").unwrap().collect();
        let names: Vec<_> = functions.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Functions: {:?}", names);
        assert!(names.contains(&"factorial"), "Should find 'factorial'");
        assert!(names.contains(&"main"), "Should find 'main'");
    }

    #[test]
    fn test_extract_cpp_types() {
        let db = extract_test_file("simple.cpp");
        let types: Vec<_> = db.scan("usertypes").unwrap().collect();
        let names: Vec<_> = types.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Types: {:?}", names);
        assert!(names.contains(&"Point"), "Should find 'Point'");
        assert!(names.contains(&"Animal"), "Should find 'Animal'");
        assert!(names.contains(&"Dog"), "Should find 'Dog'");
    }

    #[test]
    fn test_extract_cpp_fields() {
        let db = extract_test_file("simple.cpp");
        let fields: Vec<_> = db.scan("fields").unwrap().collect();
        let names: Vec<_> = fields.iter().map(|t| {
            db.strings.resolve(t[2].as_string().unwrap())
        }).collect();
        eprintln!("Fields: {:?}", names);
        assert!(names.contains(&"x"), "Should find field 'x'");
        assert!(names.contains(&"y"), "Should find field 'y'");
    }

    #[test]
    fn test_extract_cpp_inheritance() {
        let db = extract_test_file("simple.cpp");
        let derivations: Vec<_> = db.scan("derivations").unwrap().collect();
        let base_names: Vec<_> = derivations.iter().map(|t| {
            db.strings.resolve(t[2].as_string().unwrap())
        }).collect();
        eprintln!("Base classes: {:?}", base_names);
        assert!(base_names.contains(&"Animal"), "Dog should extend Animal");
    }

    #[test]
    fn test_extract_cpp_params() {
        let db = extract_test_file("simple.cpp");
        let params: Vec<_> = db.scan("params").unwrap().collect();
        let names: Vec<_> = params.iter().map(|t| {
            db.strings.resolve(t[2].as_string().unwrap())
        }).collect();
        eprintln!("Params: {:?}", names);
        assert!(names.contains(&"n"), "factorial should have param 'n'");
    }

    #[test]
    fn test_extract_cpp_includes() {
        let db = extract_test_file("simple.cpp");
        let includes: Vec<_> = db.scan("includes").unwrap().collect();
        let paths: Vec<_> = includes.iter().map(|t| {
            db.strings.resolve(t[2].as_string().unwrap())
        }).collect();
        eprintln!("Includes: {:?}", paths);
        assert!(paths.iter().any(|p| p.contains("iostream")), "Should find iostream include");
    }

    #[test]
    fn test_extract_cpp_variables() {
        let db = extract_test_file("simple.cpp");
        let vars: Vec<_> = db.scan("variables").unwrap().collect();
        let names: Vec<_> = vars.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Variables: {:?}", names);
        assert!(names.contains(&"global_var"), "Should find 'global_var'");
    }

    #[test]
    fn test_extract_c_file() {
        let db = extract_test_file("simple.c");
        let functions: Vec<_> = db.scan("functions").unwrap().collect();
        let names: Vec<_> = functions.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("C Functions: {:?}", names);
        assert!(names.contains(&"add"), "Should find 'add'");
        assert!(names.contains(&"print_point"), "Should find 'print_point'");
        assert!(names.contains(&"main"), "Should find 'main'");
    }

    #[test]
    fn test_extract_c_types() {
        let db = extract_test_file("simple.c");
        let types: Vec<_> = db.scan("usertypes").unwrap().collect();
        let names: Vec<_> = types.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("C Types: {:?}", names);
        // The anonymous struct inside typedef should be extracted
        assert!(types.len() >= 1, "Should find at least 1 type");
    }

    #[test]
    fn test_extract_statements() {
        let db = extract_test_file("simple.c");
        let stmts: Vec<_> = db.scan("stmts").unwrap().collect();
        let kinds: Vec<i64> = stmts.iter().map(|t| t[1].as_int().unwrap()).collect();
        eprintln!("Statement kinds: {:?}", kinds);
        // Should have block statements (kind 7)
        assert!(kinds.contains(&STMT_BLOCK), "Should have block statements");
        // Should have return statements (kind 6)
        assert!(kinds.contains(&STMT_RETURN), "Should have return statements");
    }

    #[test]
    fn test_extract_expressions() {
        let db = extract_test_file("simple.c");
        let exprs: Vec<_> = db.scan("exprs").unwrap().collect();
        let kinds: Vec<i64> = exprs.iter().map(|t| t[1].as_int().unwrap()).collect();
        eprintln!("Expression kinds: {:?}", kinds);
        // Should have call expressions (kind 74, @callexpr)
        assert!(kinds.contains(&EXPR_CALL), "Should have call expressions");
        // Should have variable accesses (kind 84)
        assert!(kinds.contains(&EXPR_VARACCESS), "Should have variable accesses");
        // Should have addition (kind 25)
        assert!(kinds.contains(&EXPR_ADD), "Should have addition expressions");
    }

    #[test]
    fn test_extract_local_variables() {
        let db = extract_test_file("simple.c");
        let locals: Vec<_> = db.scan("localvariables").unwrap().collect();
        let names: Vec<_> = locals.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Local variables: {:?}", names);
        assert!(names.contains(&"p"), "Should find local 'p'");
        assert!(names.contains(&"sum"), "Should find local 'sum'");
    }

    #[test]
    fn test_extract_function_entry_points() {
        let db = extract_test_file("simple.c");
        let entries: Vec<_> = db.scan("function_entry_point").unwrap().collect();
        let func_count = db.scan("functions").unwrap().count();
        eprintln!("Entry points: {}, Functions: {}", entries.len(), func_count);
        assert_eq!(entries.len(), func_count, "Each function should have an entry point");
    }

    #[test]
    fn test_extract_if_then() {
        let db = extract_test_file("simple.cpp");
        let if_thens: Vec<_> = db.scan("if_then").unwrap().collect();
        eprintln!("if_then entries: {}", if_thens.len());
        // factorial has an if statement
        assert!(if_thens.len() >= 1, "Should have at least 1 if_then");
    }

    #[test]
    fn test_extract_comments() {
        let db = extract_test_file("simple.c");
        let comments: Vec<_> = db.scan("comments").unwrap().collect();
        let texts: Vec<_> = comments.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Comments: {:?}", texts);
        assert!(texts.iter().any(|t| t.contains("simple C file")), "Should find the file comment");
    }

    #[test]
    fn test_extract_valuetext() {
        let db = extract_test_file("simple.c");
        let values: Vec<_> = db.scan("valuetext").unwrap().collect();
        let texts: Vec<_> = values.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Value texts: {:?}", texts);
        assert!(texts.contains(&"0"), "Should find literal 0");
        // String literals from printf calls
        assert!(texts.iter().any(|t| t.contains("sum")), "Should find format string literal");
    }

    #[test]
    fn test_extract_enclosingfunction() {
        let db = extract_test_file("simple.c");
        let enclosed: Vec<_> = db.scan("enclosingfunction").unwrap().collect();
        eprintln!("Enclosing function entries: {}", enclosed.len());
        assert!(enclosed.len() > 10, "Should have many enclosingfunction entries");
    }

    #[test]
    fn test_extract_enum_constants() {
        let db = extract_test_file("enums_unions.cpp");
        let consts: Vec<_> = db.scan("enumconstants").unwrap().collect();
        let names: Vec<_> = consts.iter().map(|t| {
            db.strings.resolve(t[3].as_string().unwrap())
        }).collect();
        eprintln!("Enum constants: {:?}", names);
        assert!(consts.len() >= 3, "Should have enum constants, got {}", consts.len());
    }

    #[test]
    fn test_extract_namespaces() {
        let db = extract_test_file("namespaces.cpp");
        let namespaces: Vec<_> = db.scan("namespaces").unwrap().collect();
        let names: Vec<_> = namespaces.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Namespaces: {:?}", names);
        assert!(namespaces.len() >= 1, "Should have at least 1 namespace");
    }

    #[test]
    fn test_extract_global_variables_table() {
        let db = extract_test_file("simple.cpp");
        let globals: Vec<_> = db.scan("globalvariables").unwrap().collect();
        let names: Vec<_> = globals.iter().map(|t| {
            db.strings.resolve(t[1].as_string().unwrap())
        }).collect();
        eprintln!("Global variables: {:?}", names);
        assert!(names.contains(&"global_var"), "Should find 'global_var' in globalvariables table");
    }

    #[test]
    fn test_if_else_extraction() {
        let source = b"int f(int x) { if (x > 0) { x = 1; } else { x = 2; } return x; }";
        let schema = cpp_schema();
        let mut db = Database::from_schema(schema);
        let extractor = CppExtractor::c();
        let result = extractor.extract_source(&mut db, "test.c", source);
        assert!(result.success);

        let if_then: Vec<_> = db.scan("if_then").unwrap().collect();
        let if_else: Vec<_> = db.scan("if_else").unwrap().collect();
        eprintln!("if_then rows: {}", if_then.len());
        eprintln!("if_else rows: {}", if_else.len());

        // Debug: dump all stmts to see if the if statement is extracted
        let stmts: Vec<_> = db.scan("stmts").unwrap().collect();
        for s in &stmts {
            eprintln!("  stmt: id={:?} kind={}", s[0], s[1].as_int().unwrap());
        }

        // Also debug: parse the tree and check fields
        let mut parser = tree_sitter::Parser::new();
        let lang: tree_sitter::Language = tree_sitter_c::LANGUAGE.into();
        parser.set_language(&lang).unwrap();
        let tree = parser.parse(source, None).unwrap();
        fn dump(node: &tree_sitter::Node, source: &[u8], depth: usize) {
            let pad = " ".repeat(depth * 2);
            let text: String = node.utf8_text(source).unwrap_or("").chars().take(50).collect();
            let text = text.replace('\n', "\\n");
            eprintln!("{}  {} [named={}] \"{}\"", pad, node.kind(), node.is_named(), text);
            // Show field names
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if let Some(fname) = cursor.field_name() {
                        eprintln!("{}    field={}", pad, fname);
                    }
                    dump(&child, source, depth + 1);
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }
        dump(&tree.root_node(), source, 0);

        assert_eq!(if_then.len(), 1, "Should have 1 if_then entry");
        assert_eq!(if_else.len(), 1, "Should have 1 if_else entry");
    }

    #[test]
    fn test_locations_populated() {
        let db = extract_test_file("simple.cpp");
        let locations: Vec<_> = db.scan("locations_default").unwrap().collect();
        assert!(locations.len() > 5, "Should have multiple locations, got {}", locations.len());
        // All locations should have positive line numbers
        for loc in &locations {
            let line = loc[2].as_int().unwrap();
            assert!(line > 0, "Line numbers should be positive, got {}", line);
        }
    }

    #[test]
    fn test_iscall_and_funbind() {
        let db = extract_test_file("simple.c");
        let iscall_rows: Vec<_> = db.scan("iscall").unwrap().collect();
        let funbind_rows: Vec<_> = db.scan("funbind").unwrap().collect();
        eprintln!("iscall rows: {}", iscall_rows.len());
        eprintln!("funbind rows: {}", funbind_rows.len());
        // simple.c has function calls (e.g., printf, add, etc.)
        assert!(!iscall_rows.is_empty(), "Should have iscall entries for call expressions");
        // funbind should resolve at least some calls to defined functions
        assert!(!funbind_rows.is_empty(), "Should have funbind entries for resolved calls");
    }
}
