use std::collections::HashMap;

use ocql_database::{Database, EntityId, Value};
use ocql_extractor_common::{Extractor, FactEmitter, LocationEmitter, NodeExt};
use ocql_extractor_common::tree_sitter::{Language, Node, Tree};

/// Tracks already-extracted type names → entity IDs to avoid duplicates.
/// When an `extends` or `implements` clause references a type that was already
/// extracted as a declaration in the same file, we reuse the existing entity ID.
type TypeMap = HashMap<String, EntityId>;

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
const STMT_CASE: i64 = 21;
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
const EXPR_TYPEACCESS: i64 = 62;
const EXPR_ARRAYTYPEACCESS: i64 = 63;
const EXPR_DECLANNOTATION: i64 = 66;

// Import kind constants
const IMPORT_SINGLE_TYPE: i64 = 1;
const IMPORT_ON_DEMAND_FROM_TYPE: i64 = 2;
const IMPORT_ON_DEMAND_FROM_PACKAGE: i64 = 3;
const IMPORT_STATIC_ON_DEMAND: i64 = 4;
const IMPORT_STATIC_SINGLE: i64 = 5;

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
        resolve_call_bindings(emitter);
        resolve_variable_bindings(emitter);
    }
}

/// Public entry point: resolve all call and variable bindings on a database.
/// Call this after JDK bytecode extraction to bind source-level calls to JDK methods.
pub fn resolve_bindings(db: &mut Database) {
    let mut emitter = FactEmitter::new(db);
    resolve_call_bindings(&mut emitter);
    resolve_variable_bindings(&mut emitter);
}

/// Post-extraction pass: resolve method/constructor calls to their declarations.
///
/// For each method invocation (EXPR_METHODACCESS=61), finds the method name from
/// namestrings, looks up matching methods by name, and emits callableBinding rows.
/// For each object creation (EXPR_NEW=52), matches against constructors by type name.
fn resolve_call_bindings(emitter: &mut FactEmitter<'_>) {
    use std::collections::{HashMap, HashSet};

    // Step 0: Collect already-bound expression IDs to avoid duplicates
    let mut already_bound: HashSet<EntityId> = HashSet::new();
    if let Some(rows) = emitter.db.scan("callableBinding") {
        for t in rows {
            if let Value::Entity(id) = &t[0] {
                already_bound.insert(*id);
            }
        }
    }

    // Step 1: Find all method call (kind 61) and new (kind 52) expressions
    let mut method_calls: Vec<EntityId> = Vec::new();
    let mut new_exprs: Vec<EntityId> = Vec::new();
    if let Some(rows) = emitter.db.scan("exprs") {
        for t in rows {
            if let (Value::Entity(id), Value::Int(kind)) = (&t[0], &t[1]) {
                if already_bound.contains(id) {
                    continue;
                }
                if *kind == EXPR_METHODACCESS {
                    method_calls.push(*id);
                } else if *kind == EXPR_NEW {
                    new_exprs.push(*id);
                }
            }
        }
    }

    if method_calls.is_empty() && new_exprs.is_empty() {
        return;
    }

    // Step 2: Build expr_id → name map from namestrings
    let mut expr_name_map: HashMap<EntityId, String> = HashMap::new();
    if let Some(rows) = emitter.db.scan("namestrings") {
        for t in rows {
            if let (Value::String(name_sid), Value::Entity(eid)) = (&t[1], &t[2]) {
                let name = emitter.db.strings.resolve(*name_sid).to_string();
                expr_name_map.insert(*eid, name);
            }
        }
    }

    // Step 3: Build method name → method entity ID map (may have multiple with same name)
    let mut method_map: HashMap<String, Vec<EntityId>> = HashMap::new();
    if let Some(rows) = emitter.db.scan("methods") {
        for t in rows {
            if let (Value::Entity(id), Value::String(name_sid)) = (&t[0], &t[1]) {
                let name = emitter.db.strings.resolve(*name_sid).to_string();
                method_map.entry(name).or_default().push(*id);
            }
        }
    }

    // Step 4: Build type name → constructor entity ID map
    let mut constr_map: HashMap<String, Vec<EntityId>> = HashMap::new();
    if let Some(rows) = emitter.db.scan("constrs") {
        for t in rows {
            if let (Value::Entity(id), Value::String(name_sid)) = (&t[0], &t[1]) {
                let name = emitter.db.strings.resolve(*name_sid).to_string();
                constr_map.entry(name).or_default().push(*id);
            }
        }
    }

    // Step 5: Resolve method calls → callableBinding
    let mut bindings: Vec<(EntityId, EntityId)> = Vec::new();
    for call_id in &method_calls {
        if let Some(method_name) = expr_name_map.get(call_id) {
            if let Some(targets) = method_map.get(method_name) {
                // Bind to the first matching method (simple name resolution)
                bindings.push((*call_id, targets[0]));
            }
        }
    }

    // Step 6: Resolve new expressions → callableBinding
    for new_id in &new_exprs {
        if let Some(type_name) = expr_name_map.get(new_id) {
            if let Some(targets) = constr_map.get(type_name) {
                bindings.push((*new_id, targets[0]));
            }
        }
    }

    // Step 7: Emit callableBinding rows
    for (expr_id, callable_id) in bindings {
        emitter.emit("callableBinding", vec![
            Value::Entity(expr_id),
            Value::Entity(callable_id),
        ]);
    }
}

/// Post-extraction pass: resolve variable accesses to their declarations.
///
/// For each variable access (EXPR_VARACCESS=60), finds the variable name from
/// namestrings, looks up matching variables (local vars, params, fields) by name,
/// and emits variableBinding rows.
fn resolve_variable_bindings(emitter: &mut FactEmitter<'_>) {
    use std::collections::{HashMap, HashSet};

    // Step 0: Collect already-bound expression IDs to avoid duplicates
    let mut already_bound: HashSet<EntityId> = HashSet::new();
    if let Some(rows) = emitter.db.scan("variableBinding") {
        for t in rows {
            if let Value::Entity(id) = &t[0] {
                already_bound.insert(*id);
            }
        }
    }

    // Step 1: Find all variable access (kind 60) expressions
    let mut var_accesses: Vec<EntityId> = Vec::new();
    if let Some(rows) = emitter.db.scan("exprs") {
        for t in rows {
            if let (Value::Entity(id), Value::Int(kind)) = (&t[0], &t[1]) {
                if *kind == EXPR_VARACCESS && !already_bound.contains(id) {
                    var_accesses.push(*id);
                }
            }
        }
    }

    if var_accesses.is_empty() {
        return;
    }

    // Step 2: Build expr_id → name map from namestrings
    let mut expr_name_map: HashMap<EntityId, String> = HashMap::new();
    if let Some(rows) = emitter.db.scan("namestrings") {
        for t in rows {
            if let (Value::String(name_sid), Value::Entity(eid)) = (&t[1], &t[2]) {
                let name = emitter.db.strings.resolve(*name_sid).to_string();
                expr_name_map.insert(*eid, name);
            }
        }
    }

    // Step 3: Build variable name → entity ID maps
    // Priority: local vars > params > fields (innermost scope wins)
    let mut var_map: HashMap<String, EntityId> = HashMap::new();

    // Fields (lowest priority — inserted first, overwritten by locals/params)
    if let Some(rows) = emitter.db.scan("fields") {
        for t in rows {
            if let (Value::Entity(id), Value::String(name_sid)) = (&t[0], &t[1]) {
                let name = emitter.db.strings.resolve(*name_sid).to_string();
                var_map.insert(name, *id);
            }
        }
    }

    // Parameters
    if let Some(rows) = emitter.db.scan("params") {
        for t in rows {
            if let Value::Entity(id) = &t[0] {
                // paramName has the actual name
                let param_id = *id;
                // Look up name from paramName table
                if let Some(pname_rows) = emitter.db.scan("paramName") {
                    for pn in pname_rows {
                        if let (Value::Entity(pid), Value::String(name_sid)) = (&pn[0], &pn[1]) {
                            if *pid == param_id {
                                let name = emitter.db.strings.resolve(*name_sid).to_string();
                                var_map.insert(name, param_id);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    // Local variables (highest priority)
    if let Some(rows) = emitter.db.scan("localvars") {
        for t in rows {
            if let (Value::Entity(id), Value::String(name_sid)) = (&t[0], &t[1]) {
                let name = emitter.db.strings.resolve(*name_sid).to_string();
                var_map.insert(name, *id);
            }
        }
    }

    // Step 4: Resolve variable accesses → variableBinding
    let mut bindings: Vec<(EntityId, EntityId)> = Vec::new();
    for var_id in &var_accesses {
        if let Some(var_name) = expr_name_map.get(var_id) {
            if let Some(&target_id) = var_map.get(var_name) {
                bindings.push((*var_id, target_id));
            }
        }
    }

    // Step 5: Emit variableBinding rows
    for (expr_id, variable_id) in bindings {
        emitter.emit("variableBinding", vec![
            Value::Entity(expr_id),
            Value::Entity(variable_id),
        ]);
    }
}

/// Extract the top-level program node.
fn extract_program(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    root: &Node<'_>,
    source: &[u8],
) {
    let mut type_map = TypeMap::new();
    let mut cursor = root.walk();
    if cursor.goto_first_child() {
        loop {
            let node = cursor.node();
            if node.is_named() {
                extract_top_level(emitter, file_id, &node, source, None, &mut type_map);
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
    type_map: &mut TypeMap,
) {
    match node.kind() {
        "package_declaration" => {
            extract_package(emitter, file_id, node, source);
        }
        "import_declaration" => {
            extract_import(emitter, file_id, node, source);
        }
        "class_declaration" => {
            extract_class(emitter, file_id, node, source, false, enclosing_class, type_map);
        }
        "interface_declaration" => {
            extract_class(emitter, file_id, node, source, true, enclosing_class, type_map);
        }
        "enum_declaration" => {
            extract_enum(emitter, file_id, node, source, enclosing_class, type_map);
        }
        "record_declaration" => {
            extract_class(emitter, file_id, node, source, false, enclosing_class, type_map);
        }
        "annotation_type_declaration" => {
            extract_annotation_type(emitter, file_id, node, source, enclosing_class, type_map);
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

    // Determine if the import target is a type (uppercase) or package (lowercase)
    // e.g., "import java.io.*" → package on-demand (kind 3)
    //        "import java.util.Map.*" → type on-demand (kind 2)
    let target_is_type = {
        // Extract the path before ".*" or the last segment
        let path = import_text
            .trim_end_matches(';')
            .trim_end_matches(".*")
            .trim();
        // Check if last segment starts with uppercase
        path.rsplit('.').next()
            .and_then(|s| s.chars().next())
            .map(|c| c.is_uppercase())
            .unwrap_or(false)
    };

    let kind = match (is_static, is_star, target_is_type) {
        (false, false, _) => IMPORT_SINGLE_TYPE,
        (false, true, true) => IMPORT_ON_DEMAND_FROM_TYPE,
        (false, true, false) => IMPORT_ON_DEMAND_FROM_PACKAGE,
        (true, true, _) => IMPORT_STATIC_ON_DEMAND,
        (true, false, _) => IMPORT_STATIC_SINGLE,
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
    type_map: &mut TypeMap,
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    if name.is_empty() { return; }

    let class_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);

    // Register in type_map for deduplication
    type_map.insert(name.clone(), class_id);

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
        extract_extends(emitter, file_id, class_id, &superclass, source, type_map);
    } else {
        // Implicit extends java.lang.Object for classes/interfaces without explicit superclass.
        // In the JVM, even interfaces have super_class = java.lang.Object.
        let object_id = get_or_create_type(emitter, type_map, "Object");
        emitter.emit("extendsReftype", vec![
            Value::Entity(class_id),
            Value::Entity(object_id),
        ]);
    }

    // Extract interfaces
    if let Some(interfaces) = node.child_by_field("interfaces") {
        extract_type_refs(emitter, file_id, class_id, &interfaces, source, type_map, "implInterface");
    }

    // Interface-extends-interface uses extendsReftype (not implInterface)
    if let Some(super_interfaces) = node.child_by_field("super_interfaces") {
        extract_type_refs(emitter, file_id, class_id, &super_interfaces, source, type_map, "extendsReftype");
    }

    // Extract type parameters
    if let Some(type_params) = node.child_by_field("type_parameters") {
        extract_type_parameters(emitter, file_id, class_id, &type_params, source);
    }

    // Extract body
    if let Some(body) = node.child_by_field("body") {
        let (has_constructor, has_instance_field_init) =
            extract_class_body(emitter, file_id, class_id, &body, source, type_map);

        // Generate implicit default constructor for classes without explicit ones
        // (interfaces don't get default constructors)
        if !is_interface && !has_constructor {
            emit_default_constructor(emitter, file_id, class_id, &name, node);
        }

        // Emit synthetic <obinit> method for classes with instance field initializers
        if !is_interface && has_instance_field_init {
            emit_obinit(emitter, file_id, class_id, node);
        }
    }
}

/// Extract superclass from a `superclass` field node.
fn extract_extends(
    emitter: &mut FactEmitter<'_>,
    _file_id: EntityId,
    class_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    type_map: &mut TypeMap,
) {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "type_identifier" || child.kind() == "scoped_type_identifier"
                || child.kind() == "generic_type"
            {
                let type_name = extract_type_name(&child, source);
                // Reuse existing entity if the type was already declared in this file
                let super_id = if let Some(&existing) = type_map.get(&type_name) {
                    existing
                } else {
                    let id = emitter.alloc();
                    let dummy_pkg = emitter.alloc();
                    let name_val = emitter.string(&type_name);
                    emitter.emit("classes_or_interfaces", vec![
                        Value::Entity(id),
                        name_val,
                        Value::Entity(dummy_pkg),
                        Value::Entity(id),
                    ]);
                    type_map.insert(type_name, id);
                    id
                };
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

/// Extract type references from a type_list node, emitting to the given table.
/// Used for both `implInterface` (class implements interface) and `extendsReftype`
/// (interface extends interface).
fn extract_type_refs(
    emitter: &mut FactEmitter<'_>,
    _file_id: EntityId,
    class_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    type_map: &mut TypeMap,
    table: &str,
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
                // Reuse existing entity if the type was already declared in this file
                let iface_id = if let Some(&existing) = type_map.get(&type_name) {
                    existing
                } else {
                    let id = emitter.alloc();
                    let dummy_pkg = emitter.alloc();
                    let name_val = emitter.string(&type_name);
                    emitter.emit("classes_or_interfaces", vec![
                        Value::Entity(id),
                        name_val,
                        Value::Entity(dummy_pkg),
                        Value::Entity(id),
                    ]);
                    emitter.emit("isInterface", vec![
                        Value::Entity(id),
                    ]);
                    type_map.insert(type_name, id);
                    id
                };
                emitter.emit(table, vec![
                    Value::Entity(class_id),
                    Value::Entity(iface_id),
                ]);
            }
            // Also check type_list children
            if child.kind() == "type_list" {
                extract_type_refs(emitter, _file_id, class_id, &child, source, type_map, table);
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

/// Get or create a `classes_or_interfaces` entry for a type by simple name.
/// Reuses existing entity from `type_map` if already seen.
fn get_or_create_type(
    emitter: &mut FactEmitter<'_>,
    type_map: &mut TypeMap,
    name: &str,
) -> EntityId {
    if let Some(&id) = type_map.get(name) {
        return id;
    }
    let id = emitter.alloc();
    let dummy_pkg = emitter.alloc();
    let name_val = emitter.string(name);
    emitter.emit("classes_or_interfaces", vec![
        Value::Entity(id),
        name_val,
        Value::Entity(dummy_pkg),
        Value::Entity(id),
    ]);
    type_map.insert(name.to_string(), id);
    id
}

/// Extract an enum declaration.
fn extract_enum(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    node: &Node<'_>,
    source: &[u8],
    enclosing_class: Option<EntityId>,
    type_map: &mut TypeMap,
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    if name.is_empty() { return; }

    let class_id = emitter.alloc();
    type_map.insert(name.clone(), class_id);
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
        extract_type_refs(emitter, file_id, class_id, &interfaces, source, type_map, "implInterface");
    }

    // Implicit extends Enum (like CodeQL)
    let enum_id = get_or_create_type(emitter, type_map, "Enum");
    emitter.emit("extendsReftype", vec![
        Value::Entity(class_id),
        Value::Entity(enum_id),
    ]);

    // Extract enum body
    if let Some(body) = node.child_by_field("body") {
        extract_enum_body(emitter, file_id, class_id, &body, source, type_map);
    }

    // Emit compiler-generated valueOf(String) and values() methods
    emit_enum_synthetic_methods(emitter, file_id, class_id, &name, node);

    // Emit implicit enum constructor (every enum has a private constructor)
    {
        let constr_id = emitter.alloc();
        let loc_id = LocationEmitter::emit_for_node(emitter, file_id, node);
        let sig = format!("{}(String, int)", name);
        let name_val = emitter.string(&name);
        let sig_val = emitter.string(&sig);
        let type_val = emitter.string(&name);
        emitter.emit("constrs", vec![
            Value::Entity(constr_id),
            name_val,
            sig_val,
            type_val,
            Value::Entity(class_id),
            Value::Entity(constr_id),
        ]);
        emitter.emit("declaresMember", vec![
            Value::Entity(class_id),
            Value::Entity(constr_id),
        ]);
        emitter.emit("hasLocation", vec![
            Value::Entity(constr_id),
            Value::Entity(loc_id),
        ]);
        let mod_id = emitter.alloc();
        let mod_name = emitter.string("private");
        emitter.emit("modifiers", vec![Value::Entity(mod_id), mod_name]);
        emitter.emit("hasModifier", vec![Value::Entity(constr_id), Value::Entity(mod_id)]);
        emitter.emit("compiler_generated", vec![Value::Entity(constr_id), Value::Int(1)]);
    }
}

/// Extract enum body (constants and members).
fn extract_enum_body(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    class_id: EntityId,
    body: &Node<'_>,
    source: &[u8],
    type_map: &mut TypeMap,
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
                    let _ = extract_class_body_children(emitter, file_id, class_id, &child, source, type_map);
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
    type_map: &mut TypeMap,
) {
    let name = node.child_by_field("name")
        .map(|n| n.text(source).to_string())
        .unwrap_or_default();
    if name.is_empty() { return; }

    let class_id = emitter.alloc();
    type_map.insert(name.clone(), class_id);
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
/// Returns true if an explicit constructor was found.
/// Returns (has_constructor, has_instance_field_init).
fn extract_class_body(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    class_id: EntityId,
    body: &Node<'_>,
    source: &[u8],
    type_map: &mut TypeMap,
) -> (bool, bool) {
    extract_class_body_children(emitter, file_id, class_id, body, source, type_map)
}

/// Returns (has_constructor, has_instance_field_init).
fn extract_class_body_children(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    class_id: EntityId,
    body: &Node<'_>,
    source: &[u8],
    type_map: &mut TypeMap,
) -> (bool, bool) {
    let mut has_constructor = false;
    let mut has_instance_field_init = false;
    let mut cursor = body.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "field_declaration" => {
                    extract_field(emitter, file_id, class_id, &child, source);
                    // Check if this is a non-static field with an initializer
                    if !has_instance_field_init && !has_modifier(&child, source, "static") {
                        let mut fc = child.walk();
                        if fc.goto_first_child() {
                            loop {
                                if fc.node().kind() == "variable_declarator"
                                    && fc.node().child_by_field("value").is_some()
                                {
                                    has_instance_field_init = true;
                                    break;
                                }
                                if !fc.goto_next_sibling() { break; }
                            }
                        }
                    }
                }
                "method_declaration" => {
                    extract_method(emitter, file_id, class_id, &child, source);
                }
                "constructor_declaration" => {
                    has_constructor = true;
                    extract_constructor(emitter, file_id, class_id, &child, source);
                }
                "class_declaration" => {
                    extract_class(emitter, file_id, &child, source, false, Some(class_id), type_map);
                }
                "interface_declaration" => {
                    extract_class(emitter, file_id, &child, source, true, Some(class_id), type_map);
                }
                "enum_declaration" => {
                    extract_enum(emitter, file_id, &child, source, Some(class_id), type_map);
                }
                "annotation_type_declaration" => {
                    extract_annotation_type(emitter, file_id, &child, source, Some(class_id), type_map);
                }
                "record_declaration" => {
                    extract_class(emitter, file_id, &child, source, false, Some(class_id), type_map);
                }
                "line_comment" | "block_comment" => {
                    extract_comment(emitter, file_id, &child, source);
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
    (has_constructor, has_instance_field_init)
}

/// Emit a synthetic default constructor for a class that has no explicit one.
/// In Java, every class without an explicit constructor gets a default no-arg constructor.
fn emit_default_constructor(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    class_id: EntityId,
    class_name: &str,
    class_node: &Node<'_>,
) {
    let constr_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, class_node);

    let sig = format!("{}()", class_name);
    let name_val = emitter.string(class_name);
    let sig_val = emitter.string(&sig);
    let type_val = emitter.string(class_name);

    emitter.emit("constrs", vec![
        Value::Entity(constr_id),
        name_val,
        sig_val,
        type_val,
        Value::Entity(class_id),
        Value::Entity(constr_id), // sourceid = self
    ]);

    emitter.emit("declaresMember", vec![
        Value::Entity(class_id),
        Value::Entity(constr_id),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(constr_id),
        Value::Entity(loc_id),
    ]);

    // Mark as default constructor
    emitter.emit("isDefConstr", vec![
        Value::Entity(constr_id),
    ]);

    // Default constructors are public
    let mod_id = emitter.alloc();
    let mod_name = emitter.string("public");
    emitter.emit("modifiers", vec![
        Value::Entity(mod_id),
        mod_name,
    ]);
    emitter.emit("hasModifier", vec![
        Value::Entity(constr_id),
        Value::Entity(mod_id),
    ]);

    // Mark as compiler-generated (kind 1 = default constructor)
    emitter.emit("compiler_generated", vec![
        Value::Entity(constr_id),
        Value::Int(1),
    ]);

    // Emit synthetic body: block with implicit super() call
    let block_id = emitter.alloc();
    emitter.emit("stmts", vec![
        Value::Entity(block_id),
        Value::Int(STMT_BLOCK),
        Value::Entity(constr_id),
        Value::Int(0),
        Value::Entity(constr_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(block_id),
        Value::Entity(loc_id),
    ]);

    let super_id = emitter.alloc();
    emitter.emit("stmts", vec![
        Value::Entity(super_id),
        Value::Int(20), // @superconstructorinvocationstmt
        Value::Entity(block_id),
        Value::Int(0),
        Value::Entity(constr_id),
    ]);
    emitter.emit("hasLocation", vec![
        Value::Entity(super_id),
        Value::Entity(loc_id),
    ]);
}

/// Emit a synthetic `<obinit>` method for classes with instance field initializers.
///
/// In CodeQL, `<obinit>` (object initializer) is a synthetic method that collects
/// all instance field initializers and instance initializer blocks. Each constructor
/// implicitly calls `<obinit>`. We emit the method and a callableBinding from each
/// constructor to it.
fn emit_obinit(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    class_id: EntityId,
    class_node: &Node<'_>,
) {
    let obinit_id = emitter.alloc();
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, class_node);

    let name_val = emitter.string("<obinit>");
    let sig_val = emitter.string("<obinit>()");
    let ret_val = emitter.string("void");
    emitter.emit("methods", vec![
        Value::Entity(obinit_id),
        name_val,
        sig_val,
        ret_val,
        Value::Entity(class_id),
        Value::Entity(obinit_id), // sourceid = self
    ]);

    emitter.emit("declaresMember", vec![
        Value::Entity(class_id),
        Value::Entity(obinit_id),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(obinit_id),
        Value::Entity(loc_id),
    ]);

    // Mark as compiler-generated
    emitter.emit("compiler_generated", vec![
        Value::Entity(obinit_id),
        Value::Int(2), // kind 2 = instance initializer
    ]);

    // Find all constructors of this class and emit a synthetic call to <obinit>
    // We scan the constrs table for entries belonging to this class.
    let constr_ids: Vec<EntityId> = {
        let mut ids = Vec::new();
        if let Some(rows) = emitter.db.scan("constrs") {
            for t in rows {
                if let (Value::Entity(cid), Value::Entity(parent)) = (&t[0], &t[4]) {
                    if *parent == class_id {
                        ids.push(*cid);
                    }
                }
            }
        }
        ids
    };

    for constr_id in constr_ids {
        // Emit a synthetic method access expression (kind 61) calling <obinit>
        let call_id = emitter.alloc();
        let call_name = emitter.string("<obinit>");
        let call_type = emitter.string("void");
        emitter.emit("exprs", vec![
            Value::Entity(call_id),
            Value::Int(EXPR_METHODACCESS),
            call_type,
            Value::Entity(constr_id),
            Value::Int(-3), // synthetic child index
        ]);
        emitter.emit("namestrings", vec![
            call_name.clone(),
            call_name,
            Value::Entity(call_id),
        ]);
        emitter.emit("callableBinding", vec![
            Value::Entity(call_id),
            Value::Entity(obinit_id),
        ]);
        emitter.emit("hasLocation", vec![
            Value::Entity(call_id),
            Value::Entity(loc_id),
        ]);
    }
}

/// Emit compiler-generated `valueOf(String)` and `values()` methods for an enum.
fn emit_enum_synthetic_methods(
    emitter: &mut FactEmitter<'_>,
    file_id: EntityId,
    class_id: EntityId,
    enum_name: &str,
    enum_node: &Node<'_>,
) {
    let loc_id = LocationEmitter::emit_for_node(emitter, file_id, enum_node);

    // static values() method
    {
        let method_id = emitter.alloc();
        let name_val = emitter.string("values");
        let sig_val = emitter.string("values()");
        let type_val = emitter.string(&format!("{}[]", enum_name));
        emitter.emit("methods", vec![
            Value::Entity(method_id),
            name_val,
            sig_val,
            type_val,
            Value::Entity(class_id),
            Value::Entity(method_id),
        ]);
        emitter.emit("declaresMember", vec![
            Value::Entity(class_id),
            Value::Entity(method_id),
        ]);
        emitter.emit("hasLocation", vec![
            Value::Entity(method_id),
            Value::Entity(loc_id),
        ]);
        // public static
        let pub_id = emitter.alloc();
        let pub_name = emitter.string("public");
        emitter.emit("modifiers", vec![Value::Entity(pub_id), pub_name]);
        emitter.emit("hasModifier", vec![Value::Entity(method_id), Value::Entity(pub_id)]);
        let static_id = emitter.alloc();
        let static_name = emitter.string("static");
        emitter.emit("modifiers", vec![Value::Entity(static_id), static_name]);
        emitter.emit("hasModifier", vec![Value::Entity(method_id), Value::Entity(static_id)]);
        emitter.emit("compiler_generated", vec![Value::Entity(method_id), Value::Int(1)]);
    }

    // static valueOf(String) method
    {
        let method_id = emitter.alloc();
        let name_val = emitter.string("valueOf");
        let sig_val = emitter.string("valueOf(String)");
        let type_val = emitter.string(enum_name);
        emitter.emit("methods", vec![
            Value::Entity(method_id),
            name_val,
            sig_val,
            type_val,
            Value::Entity(class_id),
            Value::Entity(method_id),
        ]);
        emitter.emit("declaresMember", vec![
            Value::Entity(class_id),
            Value::Entity(method_id),
        ]);
        emitter.emit("hasLocation", vec![
            Value::Entity(method_id),
            Value::Entity(loc_id),
        ]);
        // public static
        let pub_id = emitter.alloc();
        let pub_name = emitter.string("public");
        emitter.emit("modifiers", vec![Value::Entity(pub_id), pub_name]);
        emitter.emit("hasModifier", vec![Value::Entity(method_id), Value::Entity(pub_id)]);
        let static_id = emitter.alloc();
        let static_name = emitter.string("static");
        emitter.emit("modifiers", vec![Value::Entity(static_id), static_name]);
        emitter.emit("hasModifier", vec![Value::Entity(method_id), Value::Entity(static_id)]);
        emitter.emit("compiler_generated", vec![Value::Entity(method_id), Value::Int(1)]);

        // Parameter: String name
        let param_id = emitter.alloc();
        let param_type = emitter.string("String");
        emitter.emit("params", vec![
            Value::Entity(param_id),
            param_type,
            Value::Int(0),
            Value::Entity(method_id),
            Value::Entity(param_id),
        ]);
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
                    emitter.emit("declaresMember", vec![
                        Value::Entity(class_id),
                        Value::Entity(field_id),
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

    emitter.emit("declaresMember", vec![
        Value::Entity(class_id),
        Value::Entity(method_id),
    ]);

    emitter.emit("hasLocation", vec![
        Value::Entity(method_id),
        Value::Entity(loc_id),
    ]);

    extract_modifiers(emitter, file_id, node, source, method_id);

    // Interface methods are implicitly public and abstract (unless default/static).
    // Check if the enclosing declaration is an interface.
    let in_interface = node.parent()
        .and_then(|p| p.parent())
        .map_or(false, |gp| gp.kind() == "interface_declaration");
    if in_interface {
        // Add implicit "public" if not explicitly present
        let has_public = has_modifier(node, source, "public");
        if !has_public {
            let mod_id = emitter.alloc();
            let mod_name = emitter.string("public");
            emitter.emit("modifiers", vec![Value::Entity(mod_id), mod_name]);
            emitter.emit("hasModifier", vec![Value::Entity(method_id), Value::Entity(mod_id)]);
        }
        // Add implicit "abstract" if not default and not static
        let has_abstract = has_modifier(node, source, "abstract");
        let has_default = has_modifier(node, source, "default");
        let has_static = has_modifier(node, source, "static");
        if !has_abstract && !has_default && !has_static {
            let mod_id = emitter.alloc();
            let mod_name = emitter.string("abstract");
            emitter.emit("modifiers", vec![Value::Entity(mod_id), mod_name]);
            emitter.emit("hasModifier", vec![Value::Entity(method_id), Value::Entity(mod_id)]);
        }
    }

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

    emitter.emit("declaresMember", vec![
        Value::Entity(class_id),
        Value::Entity(constr_id),
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
        // Check if body starts with explicit_constructor_invocation (super()/this())
        let has_explicit_ctor_call = {
            let mut cursor = body.walk();
            let mut found = false;
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "explicit_constructor_invocation" {
                        found = true;
                        break;
                    }
                    // Skip non-statement nodes (braces, whitespace)
                    if child.is_named() {
                        break; // First named child is not explicit_constructor_invocation
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
            found
        };

        extract_method_body(emitter, file_id, constr_id, &body, source);

        // Emit implicit super() call if no explicit this()/super() was found
        if !has_explicit_ctor_call {
            // Find the block statement ID emitted as child of this constructor
            let block_id = {
                let mut found = None;
                if let Some(rows) = emitter.db.scan("stmts") {
                    for t in rows {
                        if let (Value::Entity(sid), Value::Int(0), Value::Entity(pid), Value::Int(0)) =
                            (&t[0], &t[1], &t[2], &t[3])
                        {
                            if *pid == constr_id {
                                found = Some(*sid);
                                break;
                            }
                        }
                    }
                }
                found
            };
            if let Some(block_id) = block_id {
                let super_id = emitter.alloc();
                let loc_id = LocationEmitter::emit_for_node(emitter, file_id, &body);
                emitter.emit("stmts", vec![
                    Value::Entity(super_id),
                    Value::Int(20), // @superconstructorinvocationstmt
                    Value::Entity(block_id),
                    Value::Int(-1), // synthetic, before first real statement
                    Value::Entity(constr_id),
                ]);
                emitter.emit("hasLocation", vec![
                    Value::Entity(super_id),
                    Value::Entity(loc_id),
                ]);
            }
        }
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

/// Check if a declaration node has a specific modifier keyword in its source.
fn has_modifier(node: &Node<'_>, source: &[u8], modifier: &str) -> bool {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "modifiers" {
                let mut inner = child.walk();
                if inner.goto_first_child() {
                    loop {
                        if inner.node().text(source) == modifier {
                            return true;
                        }
                        if !inner.goto_next_sibling() { break; }
                    }
                }
                return false;
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
    false
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
                            // Also emit as declannotation expression (kind 66)
                            let ann_type = emitter.string(&ann_name);
                            emitter.emit("exprs", vec![
                                Value::Entity(ann_id),
                                Value::Int(EXPR_DECLANNOTATION),
                                ann_type,
                                Value::Entity(parent_id),
                                Value::Int(-2), // annotation child
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
        "explicit_constructor_invocation" => {
            // super(...) or this(...) in constructor bodies
            let text = node.text(source);
            if text.starts_with("super") {
                Some(20) // superconstructorinvocation
            } else {
                Some(19) // constructorinvocation (this)
            }
        }
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
        "explicit_constructor_invocation" => {
            // Extract arguments of super(...) or this(...) calls
            if let Some(args) = node.child_by_field("arguments") {
                let mut idx = 0i64;
                let mut cursor = args.walk();
                if cursor.goto_first_child() {
                    loop {
                        let child = cursor.node();
                        if child.is_named() {
                            extract_expr(emitter, file_id, &child, source, callable_id, stmt_id, idx);
                            idx += 1;
                        }
                        if !cursor.goto_next_sibling() { break; }
                    }
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
            // Extract init (may be local_variable_declaration or expression_statement)
            if let Some(init) = node.child_by_field("init") {
                extract_stmt(emitter, file_id, &init, source, callable_id, stmt_id, 0);
            }
            // Extract condition expression
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, callable_id, stmt_id, 1);
            }
            // Extract update expression(s)
            if let Some(update) = node.child_by_field("update") {
                extract_expr(emitter, file_id, &update, source, callable_id, stmt_id, 2);
            }
            // Extract body
            if let Some(body) = node.child_by_field("body") {
                extract_stmt(emitter, file_id, &body, source, callable_id, stmt_id, 3);
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
        "switch_expression" => {
            // Extract condition
            if let Some(cond) = node.child_by_field("condition") {
                extract_expr(emitter, file_id, &cond, source, callable_id, stmt_id, 0);
            }
            // Extract switch body — case labels and their statements
            if let Some(body) = node.child_by_field("body") {
                let mut case_idx = 0i64;
                let mut cursor = body.walk();
                if cursor.goto_first_child() {
                    loop {
                        let child = cursor.node();
                        match child.kind() {
                            "switch_block_statement_group" => {
                                // Each group has switch_label(s) and statement(s)
                                let mut inner = child.walk();
                                if inner.goto_first_child() {
                                    loop {
                                        let item = inner.node();
                                        if item.kind() == "switch_label" {
                                            // Emit case label as STMT_CASE
                                            let case_id = emitter.alloc();
                                            let case_loc = LocationEmitter::emit_for_node(emitter, file_id, &item);
                                            emitter.emit("stmts", vec![
                                                Value::Entity(case_id),
                                                Value::Int(STMT_CASE),
                                                Value::Entity(stmt_id),
                                                Value::Int(case_idx),
                                                Value::Entity(callable_id),
                                            ]);
                                            emitter.emit("hasLocation", vec![
                                                Value::Entity(case_id),
                                                Value::Entity(case_loc),
                                            ]);
                                            // Extract case value expression(s)
                                            let mut expr_idx = 0i64;
                                            let mut label_cursor = item.walk();
                                            if label_cursor.goto_first_child() {
                                                loop {
                                                    let label_child = label_cursor.node();
                                                    if label_child.is_named() {
                                                        extract_expr(emitter, file_id, &label_child, source, callable_id, case_id, expr_idx);
                                                        expr_idx += 1;
                                                    }
                                                    if !label_cursor.goto_next_sibling() { break; }
                                                }
                                            }
                                            case_idx += 1;
                                        } else if item.is_named() {
                                            // Statement inside the case group
                                            extract_stmt(emitter, file_id, &item, source, callable_id, stmt_id, case_idx);
                                            case_idx += 1;
                                        }
                                        if !inner.goto_next_sibling() { break; }
                                    }
                                }
                            }
                            "switch_rule" => {
                                // Arrow-style case: switch_label -> expression/block/throw
                                let mut inner = child.walk();
                                if inner.goto_first_child() {
                                    loop {
                                        let item = inner.node();
                                        if item.kind() == "switch_label" {
                                            let case_id = emitter.alloc();
                                            let case_loc = LocationEmitter::emit_for_node(emitter, file_id, &item);
                                            emitter.emit("stmts", vec![
                                                Value::Entity(case_id),
                                                Value::Int(STMT_CASE),
                                                Value::Entity(stmt_id),
                                                Value::Int(case_idx),
                                                Value::Entity(callable_id),
                                            ]);
                                            emitter.emit("hasLocation", vec![
                                                Value::Entity(case_id),
                                                Value::Entity(case_loc),
                                            ]);
                                            case_idx += 1;
                                        } else if item.is_named() {
                                            extract_stmt(emitter, file_id, &item, source, callable_id, stmt_id, case_idx);
                                            case_idx += 1;
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
            let mut decl_idx = 0i64;
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

                            // Emit localvariabledeclexpr (kind 56)
                            let decl_expr_id = emitter.alloc();
                            let type_val2 = emitter.string(&type_text);
                            emitter.emit("exprs", vec![
                                Value::Entity(decl_expr_id),
                                Value::Int(EXPR_LOCALVARDECL),
                                type_val2,
                                Value::Entity(stmt_id),
                                Value::Int(decl_idx),
                            ]);
                            emitter.emit("hasLocation", vec![
                                Value::Entity(decl_expr_id),
                                Value::Entity(loc_id),
                            ]);
                            emitter.emit("variableBinding", vec![
                                Value::Entity(decl_expr_id),
                                Value::Entity(var_id),
                            ]);

                            // Extract initializer expression
                            if let Some(value) = child.child_by_field("value") {
                                extract_expr(emitter, file_id, &value, source, callable_id, decl_expr_id, 0);
                            }
                            decl_idx += 1;
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

    // Emit namestrings for literals
    if expr_kind >= EXPR_BOOLEAN_LIT && expr_kind <= EXPR_NULL_LIT {
        let text = node.text(source);
        // For string literals, strip surrounding quotes
        let value = if expr_kind == EXPR_STRING_LIT {
            unescape_java_string(text.trim_matches('"'))
        } else if expr_kind == EXPR_CHAR_LIT {
            unescape_java_string(text.trim_matches('\''))
        } else {
            text.to_string()
        };
        let name_val = emitter.string(&text);
        let value_val = emitter.string(&value);
        emitter.emit("namestrings", vec![
            name_val,
            value_val,
            Value::Entity(expr_id),
        ]);
    }

    // Emit namestrings for method invocations (method name)
    if expr_kind == EXPR_METHODACCESS {
        if let Some(name_node) = node.child_by_field("name") {
            let method_name = name_node.text(source);
            let name_val = emitter.string(method_name);
            let value_val = emitter.string(method_name);
            emitter.emit("namestrings", vec![
                name_val,
                value_val,
                Value::Entity(expr_id),
            ]);
        }
    }

    // Emit namestrings for object creation expressions (type name)
    if expr_kind == EXPR_NEW {
        if let Some(type_node) = node.child_by_field("type") {
            let type_name = type_node.text(source);
            let name_val = emitter.string(type_name);
            let value_val = emitter.string(type_name);
            emitter.emit("namestrings", vec![
                name_val,
                value_val,
                Value::Entity(expr_id),
            ]);
            // Emit type access expression (kind 62) as child of new expr
            let ta_id = emitter.alloc();
            let ta_loc = LocationEmitter::emit_for_node(emitter, file_id, &type_node);
            let ta_type = emitter.string(type_name);
            emitter.emit("exprs", vec![
                Value::Entity(ta_id),
                Value::Int(EXPR_TYPEACCESS),
                ta_type,
                Value::Entity(expr_id),
                Value::Int(-1), // type child
            ]);
            emitter.emit("hasLocation", vec![
                Value::Entity(ta_id),
                Value::Entity(ta_loc),
            ]);
        }
    }

    // Emit namestrings for variable accesses (identifier name)
    if expr_kind == EXPR_VARACCESS {
        let var_name = if node.kind() == "field_access" {
            // For field access, use the field name
            node.child_by_field("field")
                .map(|n| n.text(source).to_string())
                .unwrap_or_default()
        } else {
            // For plain identifier
            node.text(source).to_string()
        };
        if !var_name.is_empty() {
            let name_val = emitter.string(&var_name);
            let value_val = emitter.string(&var_name);
            emitter.emit("namestrings", vec![
                name_val,
                value_val,
                Value::Entity(expr_id),
            ]);
        }
    }

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
/// Unescape Java string escape sequences to their actual character values.
fn unescape_java_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some('r') => result.push('\r'),
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('\'') => result.push('\''),
                Some('0') => result.push('\0'),
                Some('u') => {
                    // Unicode escape: \uXXXX
                    let hex: String = chars.by_ref().take(4).collect();
                    if let Ok(code) = u32::from_str_radix(&hex, 16) {
                        if let Some(ch) = char::from_u32(code) {
                            result.push(ch);
                        }
                    }
                }
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    result
}

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
