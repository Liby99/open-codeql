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
#[allow(dead_code)]
const VAR_LOCAL: i64 = 2;

// User type kind constants (matches CodeQL convention)
const USERTYPE_STRUCT: i64 = 1;
const USERTYPE_CLASS: i64 = 2;
const USERTYPE_UNION: i64 = 3;
const USERTYPE_ENUM: i64 = 4;

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
            // Recurse into namespace body
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
        name_val,
        Value::Int(kind),
    ]);

    // Emit location
    emitter.emit("element_location", vec![
        Value::Entity(func_id),
        Value::Entity(loc_id),
    ]);

    // Return type
    if let Some(type_node) = node.child_by_field("type") {
        let return_type = type_node.text(source);
        let rt_val = emitter.string(return_type);
        emitter.emit("function_return_type", vec![
            Value::Entity(func_id),
            rt_val,
        ]);
    }

    // Parameters
    if let Some(declarator) = node.child_by_field("declarator") {
        extract_parameters(emitter, func_id, &declarator, source);
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
                    let param_type = child.child_by_field("type")
                        .map(|t| t.text(source))
                        .unwrap_or("unknown");
                    let param_name = child.child_by_field("declarator")
                        .map(|d| extract_declarator_name(&d, source))
                        .unwrap_or_default();

                    let type_val = emitter.string(param_type);
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
        "pointer_declarator" | "reference_declarator" | "array_declarator" => {
            if let Some(decl) = node.child_by_field("declarator") {
                extract_declarator_name(&decl, source)
            } else {
                String::new()
            }
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
                            name_val,
                            type_val,
                            Value::Int(VAR_GLOBAL),
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
                            name_val,
                            type_val,
                            Value::Int(VAR_GLOBAL),
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
fn extract_type_specifier(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    let kind = match node.kind() {
        "struct_specifier" => USERTYPE_STRUCT,
        "class_specifier" => USERTYPE_CLASS,
        "union_specifier" => USERTYPE_UNION,
        "enum_specifier" => USERTYPE_ENUM,
        _ => return,
    };

    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();

    // Skip anonymous types with no body (forward declarations with no name)
    if name.is_empty() {
        // Could be an anonymous struct inside a typedef — still extract if it has a body
        if node.child_by_field("body").is_none() {
            return;
        }
    }

    let type_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    let name_val = emitter.string(if name.is_empty() { "<anonymous>" } else { &name });
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

    // Extract fields and member functions
    if let Some(body) = node.child_by_field("body") {
        extract_class_body(emitter, file_id, type_id, &body, source);
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
    let field_type = node.child_by_field("type")
        .map(|t| t.text(source))
        .unwrap_or("unknown");

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "field_identifier" => {
                    let field_name = child.text(source);
                    let name_val = emitter.string(field_name);
                    let type_val = emitter.string(field_type);
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
                _ => {}
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
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
    let field_type = node.child_by_field("type")
        .map(|t| t.text(source))
        .unwrap_or("unknown");

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
fn extract_typedef(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
) {
    // Check if the typedef wraps a struct/class/union/enum
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
}
